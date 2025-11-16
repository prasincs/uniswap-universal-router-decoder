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
use uniswap_router_decoder::{Decoder, Result};
use alloy_primitives::TxHash;
use alloy_provider::{ProviderBuilder, Provider};

#[tokio::main]
async fn main() -> Result<()> {
    // Create provider
    let provider = ProviderBuilder::new()
        .on_http("https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY".parse().unwrap());

    // Create decoder
    let decoder = Decoder::new(provider);

    // Decode a transaction
    let tx_hash: TxHash = "0x52e63b75f41a352ad9182f9e0f923c8557064c3b1047d1778c1ea5b11b979dd9"
        .parse()
        .unwrap();

    let decoded = decoder.decode_transaction(tx_hash).await?;
    println!("{:#?}", decoded);

    Ok(())
}
```
*/

#![warn(missing_docs)]

pub mod constants;
pub mod decoder;
pub mod enums;
pub mod error;
pub mod types;
pub mod v3_path;

// Re-export main types
pub use decoder::Decoder;
pub use enums::{RouterFunction, V4Actions, RouterConstants, V4Constants, FunctionRecipient, TransactionSpeed};
pub use error::{Result, RouterError};
pub use types::*;
pub use constants::Addresses;
pub use v3_path::{decode_v3_path, encode_v3_path, extract_tokens, extract_fees};

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
