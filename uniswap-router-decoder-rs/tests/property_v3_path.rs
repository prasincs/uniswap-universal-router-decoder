/// Property-Based Tests for V3 Path Encoding/Decoding
///
/// Uses proptest to generate random V3 paths and verify encode/decode roundtrips.
/// This ensures the V3 path codec is correct for all valid inputs.
///
/// Run with: cargo test --test property_v3_path

use alloy_primitives::Address;
use proptest::prelude::*;
use uniswap_router_decoder_rs::types::V3PathElement;
use uniswap_router_decoder_rs::v3_path::{decode_v3_path, encode_v3_path};

/// Generate a valid V3 fee tier (100, 500, 3000, or 10000)
fn fee_strategy() -> impl Strategy<Value = u32> {
    prop_oneof![
        Just(100),    // 0.01%
        Just(500),    // 0.05%
        Just(3000),   // 0.3%
        Just(10000),  // 1%
    ]
}

/// Generate a random address
fn address_strategy() -> impl Strategy<Value = Address> {
    any::<[u8; 20]>().prop_map(Address::from)
}

/// Generate a V3 path element (either token or fee)
fn path_element_strategy() -> impl Strategy<Value = V3PathElement> {
    prop_oneof![
        address_strategy().prop_map(V3PathElement::Token),
        fee_strategy().prop_map(V3PathElement::Fee),
    ]
}

/// Generate a valid V3 path
/// Format: Token -> Fee -> Token -> Fee -> ... -> Token
fn v3_path_strategy() -> impl Strategy<Value = Vec<V3PathElement>> {
    // Generate 1 to 4 hops (2 to 5 tokens)
    prop::collection::vec(
        (address_strategy(), fee_strategy()),
        1..=4,
    ).prop_map(|pairs| {
        let mut path = Vec::new();

        // Add first token
        if let Some((first_token, _)) = pairs.first() {
            path.push(V3PathElement::Token(*first_token));
        }

        // Add fee -> token pairs
        for (token, fee) in &pairs {
            path.push(V3PathElement::Fee(*fee));
            path.push(V3PathElement::Token(*token));
        }

        path
    })
}

proptest! {
    /// Property: Encoding then decoding a V3 path (exact in) should return original path
    #[test]
    fn prop_v3_path_encode_decode_exact_in(path in v3_path_strategy()) {
        let encoded = encode_v3_path(&path, false).expect("Encoding should succeed");
        let decoded = decode_v3_path(&encoded, false).expect("Decoding should succeed");

        prop_assert_eq!(path, decoded, "Roundtrip should preserve path");
    }

    /// Property: Encoding for exact out then decoding should return original path
    #[test]
    fn prop_v3_path_encode_decode_exact_out(path in v3_path_strategy()) {
        // For exact out, we encode the path reversed
        let mut reversed = path.clone();
        reversed.reverse();
        let encoded = encode_v3_path(&reversed, false).expect("Encoding should succeed");

        // Decoding with is_exact_out=true reverses it back
        let decoded = decode_v3_path(&encoded, true).expect("Decoding should succeed");

        prop_assert_eq!(path, decoded, "Exact out decode should reverse encoded path");
    }

    /// Property: Encoded path length should be correct
    /// Formula: 20 + (num_hops * 23) where num_hops = (num_tokens - 1)
    #[test]
    fn prop_v3_path_encoded_length(path in v3_path_strategy()) {
        let encoded = encode_v3_path(&path, false).expect("Encoding should succeed");

        let num_tokens = path.iter().filter(|e| matches!(e, V3PathElement::Token(_))).count();
        let num_fees = path.iter().filter(|e| matches!(e, V3PathElement::Fee(_))).count();

        // Should have num_tokens = num_fees + 1
        prop_assert_eq!(num_tokens, num_fees + 1, "Should have one more token than fees");

        // Length should be: first_token (20 bytes) + num_fees * (fee (3 bytes) + token (20 bytes))
        let expected_length = 20 + (num_fees * 23);
        prop_assert_eq!(encoded.len(), expected_length, "Encoded length should match formula");
    }

    /// Property: Decoding an encoded path should preserve token order (exact in)
    #[test]
    fn prop_preserves_token_order_exact_in(tokens in prop::collection::vec(address_strategy(), 2..=5)) {
        // Build path: token0 -> fee -> token1 -> fee -> token2 -> ...
        let mut path = Vec::new();
        for (i, token) in tokens.iter().enumerate() {
            path.push(V3PathElement::Token(*token));
            if i < tokens.len() - 1 {
                path.push(V3PathElement::Fee(3000)); // Use 0.3% fee
            }
        }

        let encoded = encode_v3_path(&path, false).expect("Encoding should succeed");
        let decoded = decode_v3_path(&encoded, false).expect("Decoding should succeed");

        let decoded_tokens: Vec<Address> = decoded
            .iter()
            .filter_map(|e| match e {
                V3PathElement::Token(addr) => Some(*addr),
                _ => None,
            })
            .collect();

        prop_assert_eq!(tokens, decoded_tokens, "Token order should be preserved");
    }

    /// Property: Fees should be preserved during encode/decode
    #[test]
    fn prop_preserves_fees(
        tokens in prop::collection::vec(address_strategy(), 2..=4),
        fees in prop::collection::vec(fee_strategy(), 1..=3)
    ) {
        // Ensure we have right number of fees (one less than tokens)
        let num_fees = tokens.len().min(fees.len() + 1) - 1;
        let tokens = &tokens[..num_fees + 1];
        let fees = &fees[..num_fees];

        // Build path
        let mut path = Vec::new();
        path.push(V3PathElement::Token(tokens[0]));
        for i in 0..num_fees {
            path.push(V3PathElement::Fee(fees[i]));
            path.push(V3PathElement::Token(tokens[i + 1]));
        }

        let encoded = encode_v3_path(&path, false).expect("Encoding should succeed");
        let decoded = decode_v3_path(&encoded, false).expect("Decoding should succeed");

        let decoded_fees: Vec<u32> = decoded
            .iter()
            .filter_map(|e| match e {
                V3PathElement::Fee(fee) => Some(*fee),
                _ => None,
            })
            .collect();

        prop_assert_eq!(fees.to_vec(), decoded_fees, "Fees should be preserved");
    }

    /// Property: Single token path should encode to exactly 20 bytes
    #[test]
    fn prop_single_token_path(token in address_strategy()) {
        let path = vec![V3PathElement::Token(token)];
        let encoded = encode_v3_path(&path, false).expect("Encoding should succeed");

        prop_assert_eq!(encoded.len(), 20, "Single token should be 20 bytes");

        let decoded = decode_v3_path(&encoded, false).expect("Decoding should succeed");
        prop_assert_eq!(path, decoded, "Single token roundtrip should work");
    }

    /// Property: Double encoding should equal single encoding
    #[test]
    fn prop_encoding_idempotent(path in v3_path_strategy()) {
        let encoded1 = encode_v3_path(&path, false).expect("First encoding should succeed");
        let decoded = decode_v3_path(&encoded1, false).expect("Decoding should succeed");
        let encoded2 = encode_v3_path(&decoded, false).expect("Second encoding should succeed");

        prop_assert_eq!(encoded1, encoded2, "Encoding should be idempotent");
    }
}

/// Edge case tests
#[cfg(test)]
mod edge_cases {
    use super::*;
    use alloy_primitives::address;

    #[test]
    fn test_minimum_path_length() {
        // Minimum valid path is 20 bytes (single token)
        let single_token = vec![V3PathElement::Token(address!("0000000000000000000000000000000000000001"))];
        let encoded = encode_v3_path(&single_token, false).unwrap();
        assert_eq!(encoded.len(), 20);
    }

    #[test]
    fn test_two_hop_path() {
        // Two tokens, one fee = 43 bytes
        let path = vec![
            V3PathElement::Token(address!("0000000000000000000000000000000000000001")),
            V3PathElement::Fee(3000),
            V3PathElement::Token(address!("0000000000000000000000000000000000000002")),
        ];
        let encoded = encode_v3_path(&path, false).unwrap();
        assert_eq!(encoded.len(), 43);
    }

    #[test]
    fn test_all_fee_tiers() {
        let fees = vec![100, 500, 3000, 10000];

        for fee in fees {
            let path = vec![
                V3PathElement::Token(address!("0000000000000000000000000000000000000001")),
                V3PathElement::Fee(fee),
                V3PathElement::Token(address!("0000000000000000000000000000000000000002")),
            ];

            let encoded = encode_v3_path(&path, false).unwrap();
            let decoded = decode_v3_path(&encoded, false).unwrap();

            assert_eq!(path, decoded, "Fee tier {} should roundtrip correctly", fee);
        }
    }

    #[test]
    fn test_maximum_reasonable_path() {
        // Test a path with 10 hops (11 tokens, 10 fees)
        let mut path = Vec::new();
        for i in 0..11 {
            let addr_bytes = [i as u8; 20];
            path.push(V3PathElement::Token(Address::from(addr_bytes)));
            if i < 10 {
                path.push(V3PathElement::Fee(3000));
            }
        }

        let encoded = encode_v3_path(&path, false).unwrap();
        assert_eq!(encoded.len(), 20 + (10 * 23)); // 250 bytes

        let decoded = decode_v3_path(&encoded, false).unwrap();
        assert_eq!(path, decoded);
    }

    #[test]
    #[should_panic(expected = "Path is empty")]
    fn test_empty_path_fails() {
        let _ = encode_v3_path(&vec![], false).unwrap();
    }

    #[test]
    #[should_panic(expected = "Path must start with a token")]
    fn test_fee_without_token_fails() {
        // Can't have fee as first element
        let _ = encode_v3_path(&vec![V3PathElement::Fee(3000)], false).unwrap();
    }

    #[test]
    #[should_panic(expected = "tokens and fees must alternate")]
    fn test_consecutive_tokens_fails() {
        // Can't have two tokens without fee between
        let _ = encode_v3_path(&vec![
            V3PathElement::Token(address!("0000000000000000000000000000000000000001")),
            V3PathElement::Token(address!("0000000000000000000000000000000000000002")),
        ], false).unwrap();
    }

    #[test]
    #[should_panic(expected = "tokens and fees must alternate")]
    fn test_consecutive_fees_fails() {
        // Can't have two fees without token between
        let _ = encode_v3_path(&vec![
            V3PathElement::Token(address!("0000000000000000000000000000000000000001")),
            V3PathElement::Fee(3000),
            V3PathElement::Fee(500),
        ], false).unwrap();
    }
}
