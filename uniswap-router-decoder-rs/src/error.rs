/// Error types for the Uniswap Universal Router decoder/encoder
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RouterError {
    #[error("ABI decoding error: {0}")]
    AbiDecoding(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Invalid command type: {0:#x}")]
    InvalidCommandType(u8),

    #[error("Invalid function name: {0}")]
    InvalidFunctionName(String),

    #[error("Action/param length mismatch: {actions} actions, {params} params")]
    LengthMismatch { actions: usize, params: usize },

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Contract error: {0}")]
    Contract(String),

    #[error("Hex decode error: {0}")]
    HexDecode(#[from] hex::FromHexError),

    #[error("Serde JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Unknown error")]
    Unknown,
}

pub type Result<T> = std::result::Result<T, RouterError>;
