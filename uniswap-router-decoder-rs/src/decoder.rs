/// Decoder for Uniswap Universal Router transactions
use alloy_primitives::{Bytes, TxHash, U256};
use alloy_consensus::transaction::Transaction;
use alloy_provider::Provider;
use alloy_sol_types::{SolCall, sol};
use serde_json::json;

use crate::constants::RouterCommands;
use crate::enums::{RouterConstants, RouterFunction, V4Actions};
use crate::error::{Result, RouterError};
use crate::types::*;
use crate::v3_path;

// Define execute functions for proper ABI decoding
sol! {
    function execute(bytes commands, bytes[] inputs);
    function execute(bytes commands, bytes[] inputs, uint256 deadline);
}

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
        // execute(bytes,bytes[]) = 0x24856bc3
        const EXECUTE_NO_DEADLINE: [u8; 4] = [0x24, 0x85, 0x6b, 0xc3];
        // execute(bytes,bytes[],uint256) = 0x3593564c
        const EXECUTE_WITH_DEADLINE: [u8; 4] = [0x35, 0x93, 0x56, 0x4c];

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
        // Use the generated sol! type
        let call = execute_1Call::abi_decode(input, false)
            .map_err(|e| RouterError::AbiDecoding(format!("Failed to decode execute(bytes,bytes[],uint256): {}", e)))?;

        Ok((call.commands, call.inputs, Some(call.deadline)))
    }

    /// Manually decode execute(bytes, bytes[])
    fn decode_execute_no_deadline(&self, input: &Bytes) -> Result<(Bytes, Vec<Bytes>, Option<U256>)> {
        // Use the generated sol! type
        let call = execute_0Call::abi_decode(input, false)
            .map_err(|e| RouterError::AbiDecoding(format!("Failed to decode execute(bytes,bytes[]): {}", e)))?;

        Ok((call.commands, call.inputs, None))
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
        use alloy_sol_types::SolType;

        // Decode parameters: (address recipient, uint256 amountIn, uint256 amountOutMin, address[] path, bool payerIsUser)
        type Params = (
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
            alloy_sol_types::sol_data::Bool,
        );

        let (recipient, amount_in, amount_out_min, path, payer_is_user) =
            <Params as SolType>::abi_decode_params(data, false)
                .map_err(|e| RouterError::AbiDecoding(format!("V2_SWAP_EXACT_IN decode failed: {}", e)))?;

        Ok(json!({
            "recipient": format!("{:?}", recipient),
            "amountIn": amount_in.to_string(),
            "amountOutMin": amount_out_min.to_string(),
            "path": path.iter().map(|a| format!("{:?}", a)).collect::<Vec<_>>(),
            "payerIsUser": payer_is_user,
        }))
    }

    fn decode_v2_swap_exact_out(&self, data: &Bytes) -> Result<serde_json::Value> {
        use alloy_sol_types::SolType;

        // Decode parameters: (address recipient, uint256 amountOut, uint256 amountInMax, address[] path, bool payerIsUser)
        type Params = (
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Address>,
            alloy_sol_types::sol_data::Bool,
        );

        let (recipient, amount_out, amount_in_max, path, payer_is_user) =
            <Params as SolType>::abi_decode_params(data, false)
                .map_err(|e| RouterError::AbiDecoding(format!("V2_SWAP_EXACT_OUT decode failed: {}", e)))?;

        Ok(json!({
            "recipient": format!("{:?}", recipient),
            "amountOut": amount_out.to_string(),
            "amountInMax": amount_in_max.to_string(),
            "path": path.iter().map(|a| format!("{:?}", a)).collect::<Vec<_>>(),
            "payerIsUser": payer_is_user,
        }))
    }

    fn decode_v3_swap_exact_in(&self, data: &Bytes) -> Result<serde_json::Value> {
        use alloy_sol_types::SolType;

        // Decode parameters: (address recipient, uint256 amountIn, uint256 amountOutMin, bytes path, bool payerIsUser)
        type Params = (
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Bytes,
            alloy_sol_types::sol_data::Bool,
        );

        let (recipient, amount_in, amount_out_min, path, payer_is_user) =
            <Params as SolType>::abi_decode_params(data, false)
                .map_err(|e| RouterError::AbiDecoding(format!("V3_SWAP_EXACT_IN decode failed: {}", e)))?;

        Ok(json!({
            "recipient": format!("{:?}", recipient),
            "amountIn": amount_in.to_string(),
            "amountOutMin": amount_out_min.to_string(),
            "path": format!("0x{}", hex::encode(&path)),
            "payerIsUser": payer_is_user,
        }))
    }

    fn decode_v3_swap_exact_out(&self, data: &Bytes) -> Result<serde_json::Value> {
        use alloy_sol_types::SolType;

        // Decode parameters: (address recipient, uint256 amountOut, uint256 amountInMax, bytes path, bool payerIsUser)
        type Params = (
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Uint<256>,
            alloy_sol_types::sol_data::Bytes,
            alloy_sol_types::sol_data::Bool,
        );

        let (recipient, amount_out, amount_in_max, path, payer_is_user) =
            <Params as SolType>::abi_decode_params(data, false)
                .map_err(|e| RouterError::AbiDecoding(format!("V3_SWAP_EXACT_OUT decode failed: {}", e)))?;

        Ok(json!({
            "recipient": format!("{:?}", recipient),
            "amountOut": amount_out.to_string(),
            "amountInMax": amount_in_max.to_string(),
            "path": format!("0x{}", hex::encode(&path)),
            "payerIsUser": payer_is_user,
        }))
    }

    fn decode_v4_swap(&self, data: &Bytes) -> Result<serde_json::Value> {
        use alloy_sol_types::SolType;

        // Decode parameters: (bytes actions, bytes[] params)
        type Params = (
            alloy_sol_types::sol_data::Bytes,
            alloy_sol_types::sol_data::Array<alloy_sol_types::sol_data::Bytes>,
        );

        let (actions, params) = <Params as SolType>::abi_decode_params(data, false)
            .map_err(|e| RouterError::AbiDecoding(format!("V4_SWAP decode failed: {}", e)))?;

        // Decode V4 actions
        let decoded_params = self.decode_v4_actions(&actions, &params)?;

        Ok(json!({
            "actions": format!("0x{}", hex::encode(&actions)),
            "params": decoded_params,
        }))
    }

    fn decode_v4_initialize_pool(&self, data: &Bytes) -> Result<serde_json::Value> {
        use alloy_sol_types::SolType;

        // Decode parameters: (address currency0, address currency1, uint24 fee, int24 tickSpacing, address hooks, uint160 sqrtPriceX96)
        type Params = (
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<24>,
            alloy_sol_types::sol_data::Int<24>,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<160>,
        );

        let (currency0, currency1, fee, tick_spacing, hooks, sqrt_price_x96) =
            <Params as SolType>::abi_decode_params(data, false)
                .map_err(|e| RouterError::AbiDecoding(format!("V4_INITIALIZE_POOL decode failed: {}", e)))?;

        Ok(json!({
            "currency0": format!("{:?}", currency0),
            "currency1": format!("{:?}", currency1),
            "fee": fee.to_string(),
            "tickSpacing": tick_spacing.to_string(),
            "hooks": format!("{:?}", hooks),
            "sqrtPriceX96": sqrt_price_x96.to_string(),
        }))
    }

    fn decode_v4_position_manager_call(&self, data: &Bytes) -> Result<serde_json::Value> {
        use alloy_sol_types::SolType;

        // Decode parameters: (bytes unlockData, uint256 deadline)
        type Params = (
            alloy_sol_types::sol_data::Bytes,
            alloy_sol_types::sol_data::Uint<256>,
        );

        let (unlock_data_bytes, deadline) = <Params as SolType>::abi_decode_params(data, false)
            .map_err(|e| RouterError::AbiDecoding(format!("V4_POSITION_MANAGER_CALL decode failed: {}", e)))?;

        // The unlockData needs to be further decoded
        let unlock_data = self.decode_v4_unlock_data(&unlock_data_bytes)?;

        Ok(json!({
            "unlockData": unlock_data,
            "deadline": deadline.to_string(),
        }))
    }

    fn decode_wrap_eth(&self, data: &Bytes) -> Result<serde_json::Value> {
        use alloy_sol_types::SolType;

        // Decode parameters: (address recipient, uint256 amountMin)
        type Params = (
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
        );

        let (recipient, amount_min) = <Params as SolType>::abi_decode_params(data, false)
            .map_err(|e| RouterError::AbiDecoding(format!("WRAP_ETH decode failed: {}", e)))?;

        Ok(json!({
            "recipient": format!("{:?}", recipient),
            "amountMin": amount_min.to_string(),
        }))
    }

    fn decode_unwrap_weth(&self, data: &Bytes) -> Result<serde_json::Value> {
        use alloy_sol_types::SolType;

        // Decode parameters: (address recipient, uint256 amountMin)
        type Params = (
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
        );

        let (recipient, amount_min) = <Params as SolType>::abi_decode_params(data, false)
            .map_err(|e| RouterError::AbiDecoding(format!("UNWRAP_WETH decode failed: {}", e)))?;

        Ok(json!({
            "recipient": format!("{:?}", recipient),
            "amountMin": amount_min.to_string(),
        }))
    }

    fn decode_sweep(&self, data: &Bytes) -> Result<serde_json::Value> {
        use alloy_sol_types::SolType;

        // Decode parameters: (address token, address recipient, uint256 amountMin)
        type Params = (
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
        );

        let (token, recipient, amount_min) = <Params as SolType>::abi_decode_params(data, false)
            .map_err(|e| RouterError::AbiDecoding(format!("SWEEP decode failed: {}", e)))?;

        Ok(json!({
            "token": format!("{:?}", token),
            "recipient": format!("{:?}", recipient),
            "amountMin": amount_min.to_string(),
        }))
    }

    fn decode_transfer(&self, data: &Bytes) -> Result<serde_json::Value> {
        use alloy_sol_types::SolType;

        // Decode parameters: (address token, address recipient, uint256 value)
        type Params = (
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
        );

        let (token, recipient, value) = <Params as SolType>::abi_decode_params(data, false)
            .map_err(|e| RouterError::AbiDecoding(format!("TRANSFER decode failed: {}", e)))?;

        Ok(json!({
            "token": format!("{:?}", token),
            "recipient": format!("{:?}", recipient),
            "value": value.to_string(),
        }))
    }

    fn decode_pay_portion(&self, data: &Bytes) -> Result<serde_json::Value> {
        use alloy_sol_types::SolType;

        // Decode parameters: (address token, address recipient, uint256 bips)
        type Params = (
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
        );

        let (token, recipient, bips) = <Params as SolType>::abi_decode_params(data, false)
            .map_err(|e| RouterError::AbiDecoding(format!("PAY_PORTION decode failed: {}", e)))?;

        Ok(json!({
            "token": format!("{:?}", token),
            "recipient": format!("{:?}", recipient),
            "bips": bips.to_string(),
        }))
    }

    fn decode_permit2_permit(&self, data: &Bytes) -> Result<serde_json::Value> {
        use alloy_sol_types::SolType;

        // Decode parameters: (address token, uint160 amount, uint48 expiration, uint48 nonce, address spender, uint256 sigDeadline, bytes signature)
        type Params = (
            alloy_sol_types::sol_data::Address,      // token
            alloy_sol_types::sol_data::Uint<160>,    // amount
            alloy_sol_types::sol_data::Uint<48>,     // expiration
            alloy_sol_types::sol_data::Uint<48>,     // nonce
            alloy_sol_types::sol_data::Address,      // spender
            alloy_sol_types::sol_data::Uint<256>,    // sigDeadline
            alloy_sol_types::sol_data::Bytes,        // signature
        );

        let (token, amount, expiration, nonce, spender, sig_deadline, signature) =
            <Params as SolType>::abi_decode_params(data, false)
                .map_err(|e| RouterError::AbiDecoding(format!("PERMIT2_PERMIT decode failed: {}", e)))?;

        Ok(json!({
            "token": format!("{:?}", token),
            "amount": amount.to_string(),
            "expiration": expiration.to_string(),
            "nonce": nonce.to_string(),
            "spender": format!("{:?}", spender),
            "sigDeadline": sig_deadline.to_string(),
            "signature": format!("0x{}", hex::encode(&signature)),
        }))
    }

    fn decode_permit2_transfer_from(&self, data: &Bytes) -> Result<serde_json::Value> {
        use alloy_sol_types::SolType;

        // Decode parameters: (address token, address recipient, uint256 amount)
        type Params = (
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Address,
            alloy_sol_types::sol_data::Uint<256>,
        );

        let (token, recipient, amount) = <Params as SolType>::abi_decode_params(data, false)
            .map_err(|e| RouterError::AbiDecoding(format!("PERMIT2_TRANSFER_FROM decode failed: {}", e)))?;

        Ok(json!({
            "token": format!("{:?}", token),
            "recipient": format!("{:?}", recipient),
            "amount": amount.to_string(),
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
