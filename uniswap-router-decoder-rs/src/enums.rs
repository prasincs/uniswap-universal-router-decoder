/// Enums used by the Uniswap Universal Router Codec
/// Port of _enums.py
use alloy_primitives::{Address, U256};
use serde::{Deserialize, Serialize};

/// Universal Router command IDs
/// https://docs.uniswap.org/contracts/universal-router/technical-reference#command
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum RouterFunction {
    V3SwapExactIn = 0x00,
    V3SwapExactOut = 0x01,
    Permit2TransferFrom = 0x02,
    Sweep = 0x04,
    Transfer = 0x05,
    PayPortion = 0x06,
    V2SwapExactIn = 0x08,
    V2SwapExactOut = 0x09,
    Permit2Permit = 0x0a,
    WrapEth = 0x0b,
    UnwrapWeth = 0x0c,
    V4Swap = 0x10,
    V4InitializePool = 0x13,
    V4PositionManagerCall = 0x14,
}

impl RouterFunction {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::V3SwapExactIn),
            0x01 => Some(Self::V3SwapExactOut),
            0x02 => Some(Self::Permit2TransferFrom),
            0x04 => Some(Self::Sweep),
            0x05 => Some(Self::Transfer),
            0x06 => Some(Self::PayPortion),
            0x08 => Some(Self::V2SwapExactIn),
            0x09 => Some(Self::V2SwapExactOut),
            0x0a => Some(Self::Permit2Permit),
            0x0b => Some(Self::WrapEth),
            0x0c => Some(Self::UnwrapWeth),
            0x10 => Some(Self::V4Swap),
            0x13 => Some(Self::V4InitializePool),
            0x14 => Some(Self::V4PositionManagerCall),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::V3SwapExactIn => "V3_SWAP_EXACT_IN",
            Self::V3SwapExactOut => "V3_SWAP_EXACT_OUT",
            Self::Permit2TransferFrom => "PERMIT2_TRANSFER_FROM",
            Self::Sweep => "SWEEP",
            Self::Transfer => "TRANSFER",
            Self::PayPortion => "PAY_PORTION",
            Self::V2SwapExactIn => "V2_SWAP_EXACT_IN",
            Self::V2SwapExactOut => "V2_SWAP_EXACT_OUT",
            Self::Permit2Permit => "PERMIT2_PERMIT",
            Self::WrapEth => "WRAP_ETH",
            Self::UnwrapWeth => "UNWRAP_WETH",
            Self::V4Swap => "V4_SWAP",
            Self::V4InitializePool => "V4_INITIALIZE_POOL",
            Self::V4PositionManagerCall => "V4_POSITION_MANAGER_CALL",
        }
    }
}

/// V4-specific action IDs
/// https://github.com/Uniswap/v4-periphery/blob/main/src/libraries/Actions.sol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum V4Actions {
    // Positions
    MintPosition = 0x02,
    MintPositionFromDeltas = 0x05,
    SettlePair = 0x0d,
    TakePair = 0x11,
    CloseCurrency = 0x12,
    ClearOrTake = 0x13,
    Sweep = 0x14,
    Wrap = 0x15,
    Unwrap = 0x16,

    // Swaps
    SwapExactInSingle = 0x06,
    SwapExactIn = 0x07,
    SwapExactOutSingle = 0x08,
    SwapExactOut = 0x09,
    SettleAll = 0x0c,
    TakeAll = 0x0f,
    TakePortion = 0x10,

    // Common
    Settle = 0x0b,
    Take = 0x0e,
}

impl V4Actions {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x02 => Some(Self::MintPosition),
            0x05 => Some(Self::MintPositionFromDeltas),
            0x06 => Some(Self::SwapExactInSingle),
            0x07 => Some(Self::SwapExactIn),
            0x08 => Some(Self::SwapExactOutSingle),
            0x09 => Some(Self::SwapExactOut),
            0x0b => Some(Self::Settle),
            0x0c => Some(Self::SettleAll),
            0x0d => Some(Self::SettlePair),
            0x0e => Some(Self::Take),
            0x0f => Some(Self::TakeAll),
            0x10 => Some(Self::TakePortion),
            0x11 => Some(Self::TakePair),
            0x12 => Some(Self::CloseCurrency),
            0x13 => Some(Self::ClearOrTake),
            0x14 => Some(Self::Sweep),
            0x15 => Some(Self::Wrap),
            0x16 => Some(Self::Unwrap),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::MintPosition => "MINT_POSITION",
            Self::MintPositionFromDeltas => "MINT_POSITION_FROM_DELTAS",
            Self::SettlePair => "SETTLE_PAIR",
            Self::TakePair => "TAKE_PAIR",
            Self::CloseCurrency => "CLOSE_CURRENCY",
            Self::ClearOrTake => "CLEAR_OR_TAKE",
            Self::Sweep => "SWEEP",
            Self::Wrap => "WRAP",
            Self::Unwrap => "UNWRAP",
            Self::SwapExactInSingle => "SWAP_EXACT_IN_SINGLE",
            Self::SwapExactIn => "SWAP_EXACT_IN",
            Self::SwapExactOutSingle => "SWAP_EXACT_OUT_SINGLE",
            Self::SwapExactOut => "SWAP_EXACT_OUT",
            Self::SettleAll => "SETTLE_ALL",
            Self::TakeAll => "TAKE_ALL",
            Self::TakePortion => "TAKE_PORTION",
            Self::Settle => "SETTLE",
            Self::Take => "TAKE",
        }
    }
}

/// Recipient types for router functions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FunctionRecipient {
    /// Transaction sender
    Sender,
    /// Universal Router contract
    Router,
    /// Custom address
    Custom,
}

/// Router constants from Uniswap Universal Router contract
/// https://github.com/Uniswap/universal-router/blob/main/contracts/libraries/Constants.sol
pub struct RouterConstants;

impl RouterConstants {
    /// MSG_SENDER constant
    pub const MSG_SENDER: Address = Address::new([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
    ]);

    /// ADDRESS_THIS constant
    pub const ADDRESS_THIS: Address = Address::new([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x02,
    ]);

    /// ROUTER_BALANCE constant (2^255)
    pub const ROUTER_BALANCE: U256 = U256::from_limbs([0, 0, 0, 0x8000000000000000]);

    /// FLAG_ALLOW_REVERT mask
    pub const FLAG_ALLOW_REVERT: u8 = 0x80;

    /// COMMAND_TYPE_MASK mask
    pub const COMMAND_TYPE_MASK: u8 = 0x3f;
}

/// V4 specific constants
pub struct V4Constants;

impl V4Constants {
    /// OPEN_DELTA constant
    pub const OPEN_DELTA: u64 = 0;

    /// CONTRACT_BALANCE constant
    pub const CONTRACT_BALANCE: U256 = U256::from_limbs([
        0x0000000000000000,
        0x0000000000000000,
        0x0000000000000000,
        0x8000000000000000,
    ]);
}

/// Transaction speed settings for gas estimation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionSpeed {
    Slow = 0,
    Average = 1,
    Fast = 2,
    Faster = 3,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_function_from_u8() {
        assert_eq!(
            RouterFunction::from_u8(0x00),
            Some(RouterFunction::V3SwapExactIn)
        );
        assert_eq!(
            RouterFunction::from_u8(0x10),
            Some(RouterFunction::V4Swap)
        );
        assert_eq!(RouterFunction::from_u8(0xff), None);
    }

    #[test]
    fn test_router_function_name() {
        assert_eq!(RouterFunction::V3SwapExactIn.name(), "V3_SWAP_EXACT_IN");
        assert_eq!(RouterFunction::WrapEth.name(), "WRAP_ETH");
    }

    #[test]
    fn test_v4_actions_from_u8() {
        assert_eq!(V4Actions::from_u8(0x06), Some(V4Actions::SwapExactInSingle));
        assert_eq!(V4Actions::from_u8(0x0b), Some(V4Actions::Settle));
        assert_eq!(V4Actions::from_u8(0xff), None);
    }

    #[test]
    fn test_constants() {
        assert_eq!(RouterConstants::FLAG_ALLOW_REVERT, 0x80);
        assert_eq!(RouterConstants::COMMAND_TYPE_MASK, 0x3f);
    }
}
