/// Decoder for Uniswap Universal Router transactions
use alloy_primitives::{Bytes, TxHash, U256};
use alloy_consensus::transaction::Transaction;
use alloy_provider::Provider;
use alloy_sol_types::SolCall;
use serde_json::json;

use crate::constants::RouterCommands;
use crate::enums::{RouterConstants, RouterFunction, V4Actions};
use crate::error::{Result, RouterError};
use crate::types::*;
use crate::v3_path;

/// Main decoder for Universal Router transactions
pub struct Decoder<P> {
    provider: Option<P>,
}

impl<P> Decoder<P>
where
    P: Provider + Clone,
{
    /// Create a new decoder with a provider
    pub fn new(provider: P) -> Self {
        Self {
            provider: Some(provider),
        }
    }

    /// Decode a transaction by hash
    ///
    /// Fetches the transaction from the blockchain and decodes its input
    pub async fn decode_transaction(&self, tx_hash: TxHash) -> Result<DecodedTransaction> {
        let provider = self
            .provider
            .as_ref()
            .ok_or_else(|| RouterError::Provider("No provider configured".to_string()))?;

        let tx = provider
            .get_transaction_by_hash(tx_hash)
            .await
            .map_err(|e| RouterError::Provider(e.to_string()))?
            .ok_or_else(|| RouterError::Provider("Transaction not found".to_string()))?;

        // Get the input data - field name depends on alloy version
        // Transaction input - alloy type compatibility
        let input_data = tx.inner.input().clone();
        let decoded_input = self.decode_function_input(&input_data)?;

        Ok(DecodedTransaction {
            transaction: serde_json::to_value(&tx)?,
            decoded_input,
        })
    }
}

impl<P> Decoder<P> {
    /// Create a decoder without a provider (for decoding raw inputs only)
    pub fn new_offline() -> Self {
        Self { provider: None }
    }

    /// Decode raw transaction input data
    pub fn decode_function_input(&self, input: &Bytes) -> Result<DecodedInput> {
        if input.len() < 4 {
            return Err(RouterError::AbiDecoding(
                "Input too short for function selector".to_string(),
            ));
        }

        // Function selectors for execute functions
        const EXECUTE_WITH_DEADLINE: [u8; 4] = [0x24, 0x85, 0x6b, 0xc3];
        const EXECUTE_NO_DEADLINE: [u8; 4] = [0x24, 0x85, 0x6b, 0xc5];

        let selector = &input[0..4];

        let (commands, inputs_raw, deadline) = if selector == EXECUTE_WITH_DEADLINE {
            // Manually decode: execute(bytes commands, bytes[] inputs, uint256 deadline)
            self.decode_execute_with_deadline(input)?
        } else if selector == EXECUTE_NO_DEADLINE {
            // Manually decode: execute(bytes commands, bytes[] inputs)
            self.decode_execute_no_deadline(input)?
        } else {
            return Err(RouterError::AbiDecoding(format!(
                "Unknown function selector: {}",
                hex::encode(selector)
            )));
        };

        // Decode each command
        let decoded_commands = self.decode_commands(&commands, &inputs_raw)?;

        Ok(DecodedInput {
            function_name: "execute".to_string(),
            commands,
            inputs: decoded_commands,
            deadline,
        })
    }

    /// Manually decode execute(bytes, bytes[], uint256)
    fn decode_execute_with_deadline(&self, input: &Bytes) -> Result<(Bytes, Vec<Bytes>, Option<U256>)> {
        use alloy_sol_types::SolType;

        // Skip 4-byte selector
        let data = &input[4..];

        // Decode as (bytes, bytes[], uint256)
        type ExecuteParams = (
            alloy_sol_types::sol_data::Bytes,
            alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Bytes>,
            alloy_sol_types::sol_data::Uint<256>,
        );

        let (commands, inputs, deadline) = ExecuteParams::abi_decode(data, true)
            .map_err(|e| RouterError::AbiDecoding(format!("Failed to decode execute: {}", e)))?;

        Ok((commands, inputs, Some(deadline)))
    }

    /// Manually decode execute(bytes, bytes[])
    fn decode_execute_no_deadline(&self, input: &Bytes) -> Result<(Bytes, Vec<Bytes>, Option<U256>)> {
        use alloy_sol_types::SolType;

        // Skip 4-byte selector
        let data = &input[4..];

        // Decode as (bytes, bytes[])
        type ExecuteParams = (
            alloy_sol_types::sol_data::Bytes,
            alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Bytes>,
        );

        let (commands, inputs) = ExecuteParams::abi_decode(data, true)
            .map_err(|e| RouterError::AbiDecoding(format!("Failed to decode execute: {}", e)))?;

        Ok((commands, inputs, None))
    }

    /// Decode commands and their inputs
    fn decode_commands(&self, commands: &Bytes, inputs: &[Bytes]) -> Result<Vec<CommandInput>> {
        if commands.len() != inputs.len() {
            return Err(RouterError::LengthMismatch {
                actions: commands.len(),
                params: inputs.len(),
            });
        }

        let mut decoded = Vec::new();

        for (i, &command_byte) in commands.iter().enumerate() {
            let command_type = command_byte & RouterConstants::COMMAND_TYPE_MASK;
            let revert_on_fail = (command_byte & RouterConstants::FLAG_ALLOW_REVERT) == 0;

            match RouterFunction::from_u8(command_type) {
                Some(function) => {
                    let decoded_function =
                        self.decode_command_input(function, &inputs[i])?;
                    decoded.push(CommandInput::Decoded {
                        function: decoded_function,
                        revert_on_fail,
                    });
                }
                None => {
                    // Unknown command - return as hex
                    decoded.push(CommandInput::Raw(hex::encode(&inputs[i])));
                }
            }
        }

        Ok(decoded)
    }

    /// Decode a single command input based on the function type
    fn decode_command_input(
        &self,
        function: RouterFunction,
        input: &Bytes,
    ) -> Result<DecodedFunction> {
        let name = function.name().to_string();

        // Decode based on function type
        let params = match function {
            RouterFunction::V2SwapExactIn => self.decode_v2_swap_exact_in(input)?,
            RouterFunction::V2SwapExactOut => self.decode_v2_swap_exact_out(input)?,
            RouterFunction::V3SwapExactIn => self.decode_v3_swap_exact_in(input)?,
            RouterFunction::V3SwapExactOut => self.decode_v3_swap_exact_out(input)?,
            RouterFunction::V4Swap => self.decode_v4_swap(input)?,
            RouterFunction::V4InitializePool => self.decode_v4_initialize_pool(input)?,
            RouterFunction::V4PositionManagerCall => {
                self.decode_v4_position_manager_call(input)?
            }
            RouterFunction::WrapEth => self.decode_wrap_eth(input)?,
            RouterFunction::UnwrapWeth => self.decode_unwrap_weth(input)?,
            RouterFunction::Sweep => self.decode_sweep(input)?,
            RouterFunction::Transfer => self.decode_transfer(input)?,
            RouterFunction::PayPortion => self.decode_pay_portion(input)?,
            RouterFunction::Permit2Permit => self.decode_permit2_permit(input)?,
            RouterFunction::Permit2TransferFrom => {
                self.decode_permit2_transfer_from(input)?
            }
        };

        Ok(DecodedFunction { name, params })
    }

    // Decoding functions for each command type
    fn decode_v2_swap_exact_in(&self, data: &Bytes) -> Result<serde_json::Value> {
        use crate::constants::RouterCommands::*;
        let call = V2_SWAP_EXACT_INCall::abi_decode(data, true)
            .map_err(|e| RouterError::AbiDecoding(e.to_string()))?;
        Ok(json!({
            "recipient": call.recipient,
            "amountIn": call.amountIn,
            "amountOutMin": call.amountOutMin,
            "path": call.path,
            "payerIsUser": call.payerIsUser,
        }))
    }

    fn decode_v2_swap_exact_out(&self, data: &Bytes) -> Result<serde_json::Value> {
        use crate::constants::RouterCommands::*;
        let call = V2_SWAP_EXACT_OUTCall::abi_decode(data, true)
            .map_err(|e| RouterError::AbiDecoding(e.to_string()))?;
        Ok(json!({
            "recipient": call.recipient,
            "amountOut": call.amountOut,
            "amountInMax": call.amountInMax,
            "path": call.path,
            "payerIsUser": call.payerIsUser,
        }))
    }

    fn decode_v3_swap_exact_in(&self, data: &Bytes) -> Result<serde_json::Value> {
        use crate::constants::RouterCommands::*;
        let call = V3_SWAP_EXACT_INCall::abi_decode(data, true)
            .map_err(|e| RouterError::AbiDecoding(e.to_string()))?;
        Ok(json!({
            "recipient": call.recipient,
            "amountIn": call.amountIn,
            "amountOutMin": call.amountOutMin,
            "path": call.path,
            "payerIsUser": call.payerIsUser,
        }))
    }

    fn decode_v3_swap_exact_out(&self, data: &Bytes) -> Result<serde_json::Value> {
        use crate::constants::RouterCommands::*;
        let call = V3_SWAP_EXACT_OUTCall::abi_decode(data, true)
            .map_err(|e| RouterError::AbiDecoding(e.to_string()))?;
        Ok(json!({
            "recipient": call.recipient,
            "amountOut": call.amountOut,
            "amountInMax": call.amountInMax,
            "path": call.path,
            "payerIsUser": call.payerIsUser,
        }))
    }

    fn decode_v4_swap(&self, data: &Bytes) -> Result<serde_json::Value> {
        use crate::constants::RouterCommands::*;
        let call = V4_SWAPCall::abi_decode(data, true)
            .map_err(|e| RouterError::AbiDecoding(e.to_string()))?;

        // Decode V4 actions
        let decoded_params = self.decode_v4_actions(&call.actions, &call.params)?;

        Ok(json!({
            "actions": call.actions,
            "params": decoded_params,
        }))
    }

    fn decode_v4_initialize_pool(&self, data: &Bytes) -> Result<serde_json::Value> {
        use crate::constants::RouterCommands::*;
        let call = V4_INITIALIZE_POOLCall::abi_decode(data, true)
            .map_err(|e| RouterError::AbiDecoding(e.to_string()))?;
        Ok(json!({
            "currency0": call.currency0,
            "currency1": call.currency1,
            "fee": call.fee,
            "tickSpacing": call.tickSpacing,
            "hooks": call.hooks,
            "sqrtPriceX96": call.sqrtPriceX96,
        }))
    }

    fn decode_v4_position_manager_call(&self, data: &Bytes) -> Result<serde_json::Value> {
        use crate::constants::RouterCommands::*;
        let call = V4_POSITION_MANAGER_CALLCall::abi_decode(data, true)
            .map_err(|e| RouterError::AbiDecoding(e.to_string()))?;

        // The unlockData needs to be further decoded
        let unlock_data = self.decode_v4_unlock_data(&call.unlockData)?;

        Ok(json!({
            "unlockData": unlock_data,
            "deadline": call.deadline,
        }))
    }

    fn decode_wrap_eth(&self, data: &Bytes) -> Result<serde_json::Value> {
        use crate::constants::RouterCommands::*;
        let call = WRAP_ETHCall::abi_decode(data, true)
            .map_err(|e| RouterError::AbiDecoding(e.to_string()))?;
        Ok(json!({
            "recipient": call.recipient,
            "amountMin": call.amountMin,
        }))
    }

    fn decode_unwrap_weth(&self, data: &Bytes) -> Result<serde_json::Value> {
        use crate::constants::RouterCommands::*;
        let call = UNWRAP_WETHCall::abi_decode(data, true)
            .map_err(|e| RouterError::AbiDecoding(e.to_string()))?;
        Ok(json!({
            "recipient": call.recipient,
            "amountMin": call.amountMin,
        }))
    }

    fn decode_sweep(&self, data: &Bytes) -> Result<serde_json::Value> {
        use crate::constants::RouterCommands::*;
        let call = SWEEPCall::abi_decode(data, true)
            .map_err(|e| RouterError::AbiDecoding(e.to_string()))?;
        Ok(json!({
            "token": call.token,
            "recipient": call.recipient,
            "amountMin": call.amountMin,
        }))
    }

    fn decode_transfer(&self, data: &Bytes) -> Result<serde_json::Value> {
        use crate::constants::RouterCommands::*;
        let call = TRANSFERCall::abi_decode(data, true)
            .map_err(|e| RouterError::AbiDecoding(e.to_string()))?;
        Ok(json!({
            "token": call.token,
            "recipient": call.recipient,
            "value": call.value,
        }))
    }

    fn decode_pay_portion(&self, data: &Bytes) -> Result<serde_json::Value> {
        use crate::constants::RouterCommands::*;
        let call = PAY_PORTIONCall::abi_decode(data, true)
            .map_err(|e| RouterError::AbiDecoding(e.to_string()))?;
        Ok(json!({
            "token": call.token,
            "recipient": call.recipient,
            "bips": call.bips,
        }))
    }

    fn decode_permit2_permit(&self, data: &Bytes) -> Result<serde_json::Value> {
        use crate::constants::RouterCommands::*;
        let call = PERMIT2_PERMITCall::abi_decode(data, true)
            .map_err(|e| RouterError::AbiDecoding(e.to_string()))?;
        Ok(json!({
            "token": call.token,
            "amount": call.amount,
            "expiration": call.expiration,
            "nonce": call.nonce,
            "spender": call.spender,
            "sigDeadline": call.sigDeadline,
            "signature": call.signature,
        }))
    }

    fn decode_permit2_transfer_from(&self, data: &Bytes) -> Result<serde_json::Value> {
        use crate::constants::RouterCommands::*;
        let call = PERMIT2_TRANSFER_FROMCall::abi_decode(data, true)
            .map_err(|e| RouterError::AbiDecoding(e.to_string()))?;
        Ok(json!({
            "token": call.token,
            "recipient": call.recipient,
            "amount": call.amount,
        }))
    }

    /// Decode V4 actions and their parameters
    fn decode_v4_actions(
        &self,
        actions: &Bytes,
        params: &[Bytes],
    ) -> Result<Vec<serde_json::Value>> {
        if actions.len() != params.len() {
            return Err(RouterError::LengthMismatch {
                actions: actions.len(),
                params: params.len(),
            });
        }

        let mut decoded = Vec::new();

        for (i, &action_byte) in actions.iter().enumerate() {
            match V4Actions::from_u8(action_byte) {
                Some(action) => {
                    let decoded_param = self.decode_v4_action_param(action, &params[i])?;
                    decoded.push(decoded_param);
                }
                None => {
                    // Unknown action - return as hex
                    decoded.push(json!({"unknown": hex::encode(&params[i])}));
                }
            }
        }

        Ok(decoded)
    }

    /// Decode a single V4 action parameter
    fn decode_v4_action_param(
        &self,
        action: V4Actions,
        param: &Bytes,
    ) -> Result<serde_json::Value> {
        // Placeholder - full implementation would decode each action type
        Ok(json!({
            "action": action.name(),
            "raw_param": hex::encode(param),
        }))
    }

    /// Decode V4 unlock data (actions + params)
    fn decode_v4_unlock_data(&self, data: &Bytes) -> Result<serde_json::Value> {
        // Placeholder - full implementation would decode properly
        Ok(json!({
            "raw": hex::encode(data),
        }))
    }

    /// Decode a V3 path
    pub fn decode_v3_path(&self, function_name: &str, path: &Bytes) -> Result<V3Path> {
        let is_exact_out = function_name.to_uppercase().contains("EXACT_OUT");
        v3_path::decode_v3_path(path, is_exact_out)
    }

    /// Decode contract errors
    pub fn decode_error(&self, error_data: &Bytes) -> Result<(String, serde_json::Value)> {
        if error_data.len() < 4 {
            return Ok(("Unknown error".to_string(), json!({})));
        }

        // Placeholder - full implementation would decode error types
        Ok(("Unknown error".to_string(), json!({})))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_creation() {
        let decoder: Decoder<()> = Decoder::new_offline();
        // Test that decoder can be created without provider
    }

    #[test]
    fn test_command_type_mask() {
        let command_with_revert = 0x8b; // WRAP_ETH with revert flag
        let masked = command_with_revert & RouterConstants::COMMAND_TYPE_MASK;
        assert_eq!(masked, 0x0b);
        assert_eq!(
            RouterFunction::from_u8(masked),
            Some(RouterFunction::WrapEth)
        );
    }

    #[test]
    fn test_revert_flag() {
        let command_with_revert = 0x8b;
        let command_without_revert = 0x0b;

        assert!((command_with_revert & RouterConstants::FLAG_ALLOW_REVERT) != 0);
        assert!((command_without_revert & RouterConstants::FLAG_ALLOW_REVERT) == 0);
    }
}
