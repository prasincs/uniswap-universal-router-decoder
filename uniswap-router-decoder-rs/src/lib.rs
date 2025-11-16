/*!
# Uniswap Universal Router Decoder (Rust)

A high-quality, security-focused port of the Uniswap Universal Router decoder to Rust using alloy-rs.

This library provides functionality to decode and encode transactions for the Uniswap Universal Router,
supporting V2, V3, and V4 protocols.

## Features

- Decode Universal Router transactions from mainnet
- Support for V2, V3, and V4 swaps
- V3 path encoding/decoding
- V4 actions and position management
- Type-safe ABI handling with alloy-sol-types
- Comprehensive error handling
- Full test coverage with mainnet transaction validation

## Example

```rust,no_run
use uniswap_router_decoder_rs::{Decoder, Result};
use alloy_primitives::Bytes;

fn main() -> Result<()> {
    // Create offline decoder (no RPC provider needed)
    let decoder = Decoder::<()>::new_offline();

    // Decode a transaction input
    let input_hex = "0x3593564c000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000065f7e64700000000000000000000000000000000000000000000000000000000000000020b000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000f424000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000";

    let input_bytes = Bytes::from(hex::decode(&input_hex[2..]).unwrap());
    let decoded = decoder.decode_function_input(&input_bytes)?;

    println!("Function: {}", decoded.function_name);
    println!("Commands: {} commands", decoded.inputs.len());

    Ok(())
}
```
*/

#![warn(missing_docs)]

/// Constants and ABI definitions for Uniswap Universal Router
pub mod constants;
/// Main decoder implementation for Universal Router transactions
pub mod decoder;
/// Router function and action enums
pub mod enums;
/// Error types and result definitions
pub mod error;
/// Type definitions for decoded data structures
pub mod types;
/// V3 path encoding/decoding utilities
pub mod v3_path;

// Re-export main types
pub use constants::Addresses;
pub use decoder::Decoder;
pub use enums::{
    FunctionRecipient, RouterConstants, RouterFunction, TransactionSpeed, V4Actions, V4Constants,
};
pub use error::{Result, RouterError};
pub use types::*; // This includes CommandInput
pub use v3_path::{decode_v3_path, encode_v3_path, extract_fees, extract_tokens};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
