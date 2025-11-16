#!/usr/bin/env rust
//! CLI tool for decoding Uniswap Universal Router transactions
//!
//! Usage:
//!   cargo run --bin decode -- <input_hex>
//!
//! Output: JSON formatted decoded transaction matching Python decoder output

use alloy_primitives::Bytes;
use serde_json::json;
use std::env;
use uniswap_router_decoder_rs::{CommandInput, Decoder, Result, RouterError};

fn main() {
    let exit_code = match run() {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    };
    std::process::exit(exit_code);
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <input_hex>", args[0]);
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} 0x24856bc3000000...", args[0]);
        return Err(RouterError::AbiDecoding(
            "No input hex provided".to_string(),
        ));
    }

    let input_hex = &args[1];

    // Parse hex input
    let input_bytes = parse_hex_input(input_hex)?;

    // Debug: print input info
    eprintln!("Input length: {} bytes", input_bytes.len());
    eprintln!(
        "Selector: 0x{}",
        hex::encode(&input_bytes[0..4.min(input_bytes.len())])
    );

    // Create offline decoder (no RPC needed for input-only decoding)
    let decoder = Decoder::<()>::new_offline();

    // Decode the input
    let decoded = decoder.decode_function_input(&input_bytes)?;

    // Format output to match Python decoder
    let commands_detail: Vec<serde_json::Value> = decoded
        .inputs
        .iter()
        .enumerate()
        .map(|(i, cmd_input)| match cmd_input {
            CommandInput::Decoded {
                function,
                revert_on_fail,
            } => {
                json!({
                    "index": i,
                    "function": function.name,
                    "params": serialize_params(&function.params),
                    "revert_on_fail": revert_on_fail
                })
            }
            CommandInput::Raw(hex) => {
                json!({
                    "index": i,
                    "type": "unknown",
                    "raw": hex
                })
            }
        })
        .collect();

    let output = json!({
        "decoder": "rust",
        "success": true,
        "function": decoded.function_name,
        "commands": format!("0x{}", hex::encode(&decoded.commands)),
        "num_commands": decoded.inputs.len(),
        "deadline": decoded.deadline.map(|d| d.to_string()).unwrap_or_else(|| "None".to_string()),
        "commands_detail": commands_detail
    });

    // Output JSON
    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}

fn parse_hex_input(input: &str) -> Result<Bytes> {
    let hex_str = input.trim().trim_start_matches("0x");

    hex::decode(hex_str)
        .map(Bytes::from)
        .map_err(|e| RouterError::AbiDecoding(format!("Invalid hex: {}", e)))
}

fn serialize_params(params: &serde_json::Value) -> serde_json::Value {
    // The params are already in the correct format from the decoder
    // Just ensure addresses are checksummed and bytes are hex-encoded
    match params {
        serde_json::Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (k, v) in map {
                result.insert(k.clone(), serialize_value(v));
            }
            serde_json::Value::Object(result)
        }
        _ => params.clone(),
    }
}

fn serialize_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(serialize_value).collect())
        }
        serde_json::Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (k, v) in map {
                result.insert(k.clone(), serialize_value(v));
            }
            serde_json::Value::Object(result)
        }
        // Keep primitives as-is
        _ => value.clone(),
    }
}
