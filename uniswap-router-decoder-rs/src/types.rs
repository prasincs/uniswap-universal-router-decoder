/// Type definitions for Uniswap Universal Router
use alloy_primitives::{Address, Bytes, U256};
use serde::{Deserialize, Serialize};

/// Pool key for V4 pools
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolKey {
    pub currency_0: Address,
    pub currency_1: Address,
    pub fee: u32,
    pub tick_spacing: i32,
    pub hooks: Address,
}

/// Path key for V4 multi-hop swaps
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathKey {
    pub intermediate_currency: Address,
    pub fee: u32,
    pub tick_spacing: i32,
    pub hooks: Address,
    pub hook_data: Bytes,
}

/// Decoded router command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodedCommand {
    pub function_name: String,
    pub params: serde_json::Value,
    pub revert_on_fail: bool,
}

/// Decoded transaction input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodedInput {
    pub function_name: String,
    pub commands: Bytes,
    pub inputs: Vec<CommandInput>,
    pub deadline: Option<U256>,
}

/// Command input - either decoded or raw hex
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CommandInput {
    Decoded {
        function: DecodedFunction,
        revert_on_fail: bool,
    },
    Raw(String),
}

/// Decoded function details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodedFunction {
    pub name: String,
    pub params: serde_json::Value,
}

/// V3 swap path element
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum V3PathElement {
    Token(Address),
    Fee(u32),
}

/// Decoded V3 path (alternating tokens and fees)
pub type V3Path = Vec<V3PathElement>;

/// V4 swap actions and params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V4SwapData {
    pub actions: Bytes,
    pub params: Vec<V4ActionParam>,
}

/// V4 action parameter - either decoded or raw
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum V4ActionParam {
    Decoded {
        action_name: String,
        params: serde_json::Value,
    },
    Raw(String),
}

/// V4 position manager call data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V4PositionManagerData {
    pub unlock_data: V4UnlockData,
    pub deadline: U256,
}

/// V4 unlock data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V4UnlockData {
    pub actions: Bytes,
    pub params: Vec<V4ActionParam>,
}

/// Transaction result with decoded input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodedTransaction {
    #[serde(flatten)]
    pub transaction: serde_json::Value,
    pub decoded_input: DecodedInput,
}

/// Permit2 permit details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermitDetails {
    pub token: Address,
    pub amount: U256,
    pub expiration: u64,
    pub nonce: u64,
}

/// Permit2 single permit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermitSingle {
    pub details: PermitDetails,
    pub spender: Address,
    pub sig_deadline: U256,
}

/// Function recipient information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecipientType {
    Sender,
    Router,
    Custom(Address),
}

impl RecipientType {
    pub fn from_address(addr: Address, sender: Address, router: Address) -> Self {
        if addr == sender || addr == crate::enums::RouterConstants::MSG_SENDER {
            Self::Sender
        } else if addr == router || addr == crate::enums::RouterConstants::ADDRESS_THIS {
            Self::Router
        } else {
            Self::Custom(addr)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;

    #[test]
    fn test_recipient_type() {
        let sender = address!("0000000000000000000000000000000000000001");
        let router = address!("0000000000000000000000000000000000000002");
        let custom = address!("1111111111111111111111111111111111111111");

        assert_eq!(
            RecipientType::from_address(sender, sender, router),
            RecipientType::Sender
        );
        assert_eq!(
            RecipientType::from_address(router, sender, router),
            RecipientType::Router
        );
        assert_eq!(
            RecipientType::from_address(custom, sender, router),
            RecipientType::Custom(custom)
        );
    }

    #[test]
    fn test_pool_key_serialization() {
        let pool_key = PoolKey {
            currency_0: address!("0000000000000000000000000000000000000001"),
            currency_1: address!("0000000000000000000000000000000000000002"),
            fee: 3000,
            tick_spacing: 60,
            hooks: address!("0000000000000000000000000000000000000000"),
        };

        let json = serde_json::to_string(&pool_key).unwrap();
        let deserialized: PoolKey = serde_json::from_str(&json).unwrap();
        assert_eq!(pool_key, deserialized);
    }
}
