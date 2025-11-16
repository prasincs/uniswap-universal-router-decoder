/// Decoder for Uniswap Universal Router transactions
use alloy_primitives::{Address, Bytes, FixedBytes, TxHash, U256};
use alloy_provider::Provider;
use alloy_rpc_types::Transaction;
use alloy_sol_types::{SolCall, SolInterface};
use serde_json::json;

use crate::constants::*;
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
    P: Provider,
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

        let decoded_input = self.decode_function_input(&tx.input)?;

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

        let selector = &input[0..4];

        // Check if it's execute(bytes,bytes[]) or execute(bytes,bytes[],uint256)
        let has_deadline = selector == UniversalRouter::executeCall::SELECTOR;
        let no_deadline = selector == &[0x24, 0x85, 0x6b, 0xc5]; // execute(bytes,bytes[])

        if !has_deadline && !no_deadline {
            return Err(RouterError::AbiDecoding(format!(
                "Unknown function selector: {}",
                hex::encode(selector)
            )));
        }

        // Decode the function call
        let (commands, inputs_raw, deadline) = if has_deadline {
            let call = UniversalRouter::executeCall::abi_decode(input, true)
                .map_err(|e| RouterError::AbiDecoding(e.to_string()))?;
            (call.commands, call.inputs, Some(call.deadline))
        } else {
            // For the overload without deadline, we need to manually decode
            let data = &input[4..];
            let decoded = alloy_sol_types::sol_data::Bytes::abi_decode(data, true)
                .map_err(|e| RouterError::AbiDecoding(e.to_string()))?;
            // This is simplified - in practice we'd need to properly decode the bytes[] too
            (Bytes::new(), vec![], None)
        };

        let function_name = if deadline.is_some() {
            "execute"
        } else {
            "execute"
        };

        // Decode each command
        let decoded_commands = self.decode_commands(&commands, &inputs_raw)?;

        Ok(DecodedInput {
            function_name: function_name.to_string(),
            commands,
            inputs: decoded_commands,
            deadline,
        })
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
                        self.decode_command_input(function, &inputs[i], command_byte)?;
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
        command_byte: u8,
    ) -> Result<DecodedFunction> {
        let name = function.name().to_string();

        // For most commands, prepend the function selector
        // For V4_POSITION_MANAGER_CALL, the input is already complete
        let data_to_decode = if function == RouterFunction::V4PositionManagerCall {
            input.clone()
        } else {
            // Prepend selector - we need to compute it based on the function
            let selector = self.get_function_selector(function);
            let mut full_data = Vec::with_capacity(4 + input.len());
            full_data.extend_from_slice(&selector);
            full_data.extend_from_slice(input);
            Bytes::from(full_data)
        };

        // Decode based on function type
        let params = match function {
            RouterFunction::V2SwapExactIn => self.decode_v2_swap_exact_in(&data_to_decode)?,
            RouterFunction::V2SwapExactOut => self.decode_v2_swap_exact_out(&data_to_decode)?,
            RouterFunction::V3SwapExactIn => self.decode_v3_swap_exact_in(&data_to_decode)?,
            RouterFunction::V3SwapExactOut => self.decode_v3_swap_exact_out(&data_to_decode)?,
            RouterFunction::V4Swap => self.decode_v4_swap(&data_to_decode)?,
            RouterFunction::V4InitializePool => self.decode_v4_initialize_pool(&data_to_decode)?,
            RouterFunction::V4PositionManagerCall => {
                self.decode_v4_position_manager_call(&data_to_decode)?
            }
            RouterFunction::WrapEth => self.decode_wrap_eth(&data_to_decode)?,
            RouterFunction::UnwrapWeth => self.decode_unwrap_weth(&data_to_decode)?,
            RouterFunction::Sweep => self.decode_sweep(&data_to_decode)?,
            RouterFunction::Transfer => self.decode_transfer(&data_to_decode)?,
            RouterFunction::PayPortion => self.decode_pay_portion(&data_to_decode)?,
            RouterFunction::Permit2Permit => self.decode_permit2_permit(&data_to_decode)?,
            RouterFunction::Permit2TransferFrom => {
                self.decode_permit2_transfer_from(&data_to_decode)?
            }
        };

        Ok(DecodedFunction { name, params })
    }

    /// Get function selector for a given RouterFunction
    fn get_function_selector(&self, function: RouterFunction) -> [u8; 4] {
        use crate::constants::RouterCommands::*;
        match function {
            RouterFunction::V2SwapExactIn => V2_SWAP_EXACT_INCall::SELECTOR,
            RouterFunction::V2SwapExactOut => V2_SWAP_EXACT_OUTCall::SELECTOR,
            RouterFunction::V3SwapExactIn => V3_SWAP_EXACT_INCall::SELECTOR,
            RouterFunction::V3SwapExactOut => V3_SWAP_EXACT_OUTCall::SELECTOR,
            RouterFunction::WrapEth => WRAP_ETHCall::SELECTOR,
            RouterFunction::UnwrapWeth => UNWRAP_WETHCall::SELECTOR,
            RouterFunction::Sweep => SWEEPCall::SELECTOR,
            RouterFunction::Transfer => TRANSFERCall::SELECTOR,
            RouterFunction::PayPortion => PAY_PORTIONCall::SELECTOR,
            RouterFunction::V4Swap => V4_SWAPCall::SELECTOR,
            RouterFunction::V4InitializePool => V4_INITIALIZE_POOLCall::SELECTOR,
            RouterFunction::Permit2Permit => PERMIT2_PERMITCall::SELECTOR,
            RouterFunction::Permit2TransferFrom => PERMIT2_TRANSFER_FROMCall::SELECTOR,
            RouterFunction::V4PositionManagerCall => V4_POSITION_MANAGER_CALLCall::SELECTOR,
        }
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
            "poolKey": {
                "currency0": call.currency0,
                "currency1": call.currency1,
                "fee": call.fee,
                "tickSpacing": call.tickSpacing,
                "hooks": call.hooks,
            },
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
                    decoded.push(json!(hex::encode(&params[i])));
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
        // Each action has its own parameter structure
        // For now, return a placeholder - full implementation would decode each type
        Ok(json!({
            "action": action.name(),
            "raw_param": hex::encode(param),
        }))
    }

    /// Decode V4 unlock data (actions + params)
    fn decode_v4_unlock_data(&self, data: &Bytes) -> Result<serde_json::Value> {
        // Decode as (bytes actions, bytes[] params)
        // This is simplified - full implementation would use proper ABI decoding
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

        let selector = &error_data[0..4];

        // Try to decode against known errors
        // This is a simplified version - full implementation would try all error types
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
