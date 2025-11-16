/// V3 path encoding and decoding utilities
///
/// V3 paths are encoded as: token0 (20 bytes) + fee (3 bytes) + token1 (20 bytes) + ...
/// For exact-out swaps, the path is reversed after decoding.
use alloy_primitives::{Address, Bytes};

use crate::error::{Result, RouterError};
use crate::types::{V3Path, V3PathElement};

/// Decode a V3 path from bytes
///
/// # Arguments
/// * `path` - The encoded path bytes
/// * `is_exact_out` - Whether this is an exact-out swap (path will be reversed)
///
/// # Returns
/// A vector alternating between tokens (Address) and fees (u32)
pub fn decode_v3_path(path: &[u8], is_exact_out: bool) -> Result<V3Path> {
    if path.len() < 20 {
        return Err(RouterError::InvalidPath(
            "Path too short, must be at least 20 bytes".to_string(),
        ));
    }

    // V3 path format: token (20 bytes) + [fee (3 bytes) + token (20 bytes)]*
    // Minimum length is 20 bytes (single token)
    // Each additional hop adds 23 bytes (3 byte fee + 20 byte token)
    if (path.len() - 20) % 23 != 0 {
        return Err(RouterError::InvalidPath(format!(
            "Invalid path length: {}. Must be 20 + n*23 bytes",
            path.len()
        )));
    }

    let mut result = Vec::new();

    // First token (20 bytes)
    let first_token = Address::from_slice(&path[0..20]);
    result.push(V3PathElement::Token(first_token));

    // Process remaining path in 23-byte chunks (3 byte fee + 20 byte token)
    let mut offset = 20;
    while offset < path.len() {
        if offset + 23 > path.len() {
            break;
        }

        // Fee (3 bytes, big-endian uint24)
        let fee_bytes = &path[offset..offset + 3];
        let fee = u32::from_be_bytes([0, fee_bytes[0], fee_bytes[1], fee_bytes[2]]);
        result.push(V3PathElement::Fee(fee));

        // Token (20 bytes)
        let token = Address::from_slice(&path[offset + 3..offset + 23]);
        result.push(V3PathElement::Token(token));

        offset += 23;
    }

    // For exact-out swaps, reverse the path
    if is_exact_out {
        result.reverse();
    }

    Ok(result)
}

/// Encode a V3 path to bytes
///
/// # Arguments
/// * `path` - Vector of alternating tokens and fees
/// * `is_exact_out` - Whether this is an exact-out swap (path will be reversed before encoding)
///
/// # Returns
/// Encoded path bytes
pub fn encode_v3_path(path: &V3Path, is_exact_out: bool) -> Result<Bytes> {
    if path.is_empty() {
        return Err(RouterError::InvalidPath("Path is empty".to_string()));
    }

    // Validate path structure: must start with token, alternate with fees
    if !matches!(path[0], V3PathElement::Token(_)) {
        return Err(RouterError::InvalidPath(
            "Path must start with a token".to_string(),
        ));
    }

    // Check alternating pattern
    for (i, element) in path.iter().enumerate() {
        let expected_token = i % 2 == 0;
        match element {
            V3PathElement::Token(_) if !expected_token => {
                return Err(RouterError::InvalidPath(
                    "Invalid path structure: tokens and fees must alternate".to_string(),
                ));
            }
            V3PathElement::Fee(_) if expected_token => {
                return Err(RouterError::InvalidPath(
                    "Invalid path structure: tokens and fees must alternate".to_string(),
                ));
            }
            _ => {}
        }
    }

    // Path must end with a token
    if !matches!(path[path.len() - 1], V3PathElement::Token(_)) {
        return Err(RouterError::InvalidPath(
            "Path must end with a token".to_string(),
        ));
    }

    let mut working_path = path.clone();
    if is_exact_out {
        working_path.reverse();
    }

    let mut encoded = Vec::new();

    for element in working_path {
        match element {
            V3PathElement::Token(addr) => {
                encoded.extend_from_slice(addr.as_slice());
            }
            V3PathElement::Fee(fee) => {
                if fee > 0xFFFFFF {
                    return Err(RouterError::InvalidPath(format!(
                        "Fee {} exceeds maximum uint24 value",
                        fee
                    )));
                }
                // Encode as 3 bytes big-endian
                let fee_bytes = fee.to_be_bytes();
                encoded.extend_from_slice(&fee_bytes[1..4]);
            }
        }
    }

    Ok(Bytes::from(encoded))
}

/// Extract token addresses from a V3 path
pub fn extract_tokens(path: &V3Path) -> Vec<Address> {
    path.iter()
        .filter_map(|element| match element {
            V3PathElement::Token(addr) => Some(*addr),
            V3PathElement::Fee(_) => None,
        })
        .collect()
}

/// Extract fees from a V3 path
pub fn extract_fees(path: &V3Path) -> Vec<u32> {
    path.iter()
        .filter_map(|element| match element {
            V3PathElement::Token(_) => None,
            V3PathElement::Fee(fee) => Some(*fee),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;
    use hex;

    #[test]
    fn test_decode_simple_path() {
        // USDC -> WETH with 3000 fee
        // Path: USDC (20 bytes) + 0x000bb8 (3 bytes) + WETH (20 bytes)
        let usdc = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
        let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");

        let mut path_bytes = Vec::new();
        path_bytes.extend_from_slice(usdc.as_slice());
        path_bytes.extend_from_slice(&[0x00, 0x0b, 0xb8]); // 3000 fee
        path_bytes.extend_from_slice(weth.as_slice());

        let decoded = decode_v3_path(&path_bytes, false).unwrap();

        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0], V3PathElement::Token(usdc));
        assert_eq!(decoded[1], V3PathElement::Fee(3000));
        assert_eq!(decoded[2], V3PathElement::Token(weth));
    }

    #[test]
    fn test_decode_multihop_path() {
        let token0 = address!("0000000000000000000000000000000000000001");
        let token1 = address!("0000000000000000000000000000000000000002");
        let token2 = address!("0000000000000000000000000000000000000003");

        let mut path_bytes = Vec::new();
        path_bytes.extend_from_slice(token0.as_slice());
        path_bytes.extend_from_slice(&[0x00, 0x01, 0xf4]); // 500 fee
        path_bytes.extend_from_slice(token1.as_slice());
        path_bytes.extend_from_slice(&[0x00, 0x0b, 0xb8]); // 3000 fee
        path_bytes.extend_from_slice(token2.as_slice());

        let decoded = decode_v3_path(&path_bytes, false).unwrap();

        assert_eq!(decoded.len(), 5);
        assert_eq!(decoded[0], V3PathElement::Token(token0));
        assert_eq!(decoded[1], V3PathElement::Fee(500));
        assert_eq!(decoded[2], V3PathElement::Token(token1));
        assert_eq!(decoded[3], V3PathElement::Fee(3000));
        assert_eq!(decoded[4], V3PathElement::Token(token2));
    }

    #[test]
    fn test_decode_exact_out_reverses_path() {
        let token0 = address!("0000000000000000000000000000000000000001");
        let token1 = address!("0000000000000000000000000000000000000002");

        let mut path_bytes = Vec::new();
        path_bytes.extend_from_slice(token0.as_slice());
        path_bytes.extend_from_slice(&[0x00, 0x0b, 0xb8]); // 3000
        path_bytes.extend_from_slice(token1.as_slice());

        let decoded = decode_v3_path(&path_bytes, true).unwrap();

        // Path should be reversed
        assert_eq!(decoded[0], V3PathElement::Token(token1));
        assert_eq!(decoded[1], V3PathElement::Fee(3000));
        assert_eq!(decoded[2], V3PathElement::Token(token0));
    }

    #[test]
    fn test_encode_simple_path() {
        let usdc = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
        let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");

        let path = vec![
            V3PathElement::Token(usdc),
            V3PathElement::Fee(3000),
            V3PathElement::Token(weth),
        ];

        let encoded = encode_v3_path(&path, false).unwrap();

        assert_eq!(encoded.len(), 43); // 20 + 3 + 20

        // Verify structure
        assert_eq!(&encoded[0..20], usdc.as_slice());
        assert_eq!(&encoded[20..23], &[0x00, 0x0b, 0xb8]);
        assert_eq!(&encoded[23..43], weth.as_slice());
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let token0 = address!("0000000000000000000000000000000000000001");
        let token1 = address!("0000000000000000000000000000000000000002");
        let token2 = address!("0000000000000000000000000000000000000003");

        let original_path = vec![
            V3PathElement::Token(token0),
            V3PathElement::Fee(500),
            V3PathElement::Token(token1),
            V3PathElement::Fee(3000),
            V3PathElement::Token(token2),
        ];

        let encoded = encode_v3_path(&original_path, false).unwrap();
        let decoded = decode_v3_path(&encoded, false).unwrap();

        assert_eq!(original_path, decoded);
    }

    #[test]
    fn test_extract_tokens() {
        let token0 = address!("0000000000000000000000000000000000000001");
        let token1 = address!("0000000000000000000000000000000000000002");

        let path = vec![
            V3PathElement::Token(token0),
            V3PathElement::Fee(3000),
            V3PathElement::Token(token1),
        ];

        let tokens = extract_tokens(&path);
        assert_eq!(tokens, vec![token0, token1]);
    }

    #[test]
    fn test_extract_fees() {
        let token0 = address!("0000000000000000000000000000000000000001");
        let token1 = address!("0000000000000000000000000000000000000002");

        let path = vec![
            V3PathElement::Token(token0),
            V3PathElement::Fee(500),
            V3PathElement::Token(token1),
        ];

        let fees = extract_fees(&path);
        assert_eq!(fees, vec![500]);
    }

    #[test]
    fn test_invalid_path_length() {
        let invalid_path = vec![0u8; 25]; // Invalid length
        let result = decode_v3_path(&invalid_path, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_path_structure() {
        // Start with fee instead of token
        let path = vec![V3PathElement::Fee(3000), V3PathElement::Token(Address::ZERO)];
        let result = encode_v3_path(&path, false);
        assert!(result.is_err());
    }
}
