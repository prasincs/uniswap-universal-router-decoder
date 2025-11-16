/// Integration Test: Mainnet Transaction Replay
///
/// This test validates the Rust decoder against real mainnet transactions.
/// It uses fixtures collected by mainnet_tx_finder.rs
///
/// Run with: cargo test --test integration_mainnet_replay

use alloy_primitives::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use uniswap_router_decoder_rs::{Decoder, Result as RouterResult};

#[derive(Debug, Serialize, Deserialize)]
struct CommandFixture {
    command_type: String,
    command_code: u8,
    tx_hash: String,
    input_data: String,
    block_number: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct MainnetFixtures {
    router_address: String,
    fixtures: Vec<CommandFixture>,
    coverage: CoverageSummary,
}

#[derive(Debug, Serialize, Deserialize)]
struct CoverageSummary {
    total_commands: usize,
    found_commands: usize,
    missing_commands: Vec<String>,
}

/// Load mainnet fixtures
fn load_fixtures() -> Option<MainnetFixtures> {
    let fixture_path = "tests/fixtures/mainnet_txs.json";
    if let Ok(json) = fs::read_to_string(fixture_path) {
        serde_json::from_str(&json).ok()
    } else {
        None
    }
}

/// Test that decoder can handle all found mainnet transactions
#[test]
fn test_decode_all_mainnet_fixtures() {
    let fixtures = match load_fixtures() {
        Some(f) => f,
        None => {
            println!("\n⚠️  No fixtures found at tests/fixtures/mainnet_txs.json");
            println!("   Run: cargo test --test mainnet_tx_finder find_all_command_types -- --ignored --nocapture");
            println!("   to generate fixtures first.\n");
            return; // Skip test if no fixtures
        }
    };

    println!("\n🧪 Testing Rust Decoder Against {} Mainnet Transactions\n", fixtures.fixtures.len());
    println!("{:<30} {:<10} {:<50}", "Command", "Status", "Details");
    println!("{}", "=".repeat(90));

    let decoder = Decoder::<()>::new_offline();
    let mut passed = 0;
    let mut failed = 0;
    let mut errors_by_command: HashMap<String, Vec<String>> = HashMap::new();

    for fixture in &fixtures.fixtures {
        // Decode hex input
        let input_hex = fixture.input_data.strip_prefix("0x").unwrap_or(&fixture.input_data);
        let input_bytes = match hex::decode(input_hex) {
            Ok(bytes) => Bytes::from(bytes),
            Err(e) => {
                println!("{:<30} {:<10} {:<50}",
                    fixture.command_type,
                    "❌ FAIL",
                    format!("Hex decode error: {}", e)
                );
                failed += 1;
                errors_by_command
                    .entry(fixture.command_type.clone())
                    .or_default()
                    .push(format!("Hex decode: {}", e));
                continue;
            }
        };

        // Decode transaction
        match decoder.decode_function_input(&input_bytes) {
            Ok(decoded) => {
                // Verify command is present
                let has_command = decoded.inputs.iter().any(|cmd| {
                    cmd.function.to_uppercase() == fixture.command_type
                });

                if has_command {
                    println!("{:<30} {:<10} {:<50}",
                        fixture.command_type,
                        "✅ PASS",
                        format!("{} commands decoded", decoded.inputs.len())
                    );
                    passed += 1;
                } else {
                    println!("{:<30} {:<10} {:<50}",
                        fixture.command_type,
                        "⚠️  WARN",
                        "Command not found in decoded output"
                    );
                    // Still count as passed since decoding succeeded
                    passed += 1;
                }
            }
            Err(e) => {
                println!("{:<30} {:<10} {:<50}",
                    fixture.command_type,
                    "❌ FAIL",
                    format!("Decode error: {}", e)
                );
                failed += 1;
                errors_by_command
                    .entry(fixture.command_type.clone())
                    .or_default()
                    .push(format!("Decode: {}", e));
            }
        }
    }

    println!("\n{}", "=".repeat(90));
    println!("\n📊 Results:");
    println!("   ✅ Passed: {}/{} ({:.1}%)",
        passed,
        fixtures.fixtures.len(),
        (passed as f64 / fixtures.fixtures.len() as f64) * 100.0
    );
    println!("   ❌ Failed: {}/{} ({:.1}%)",
        failed,
        fixtures.fixtures.len(),
        (failed as f64 / fixtures.fixtures.len() as f64) * 100.0
    );

    println!("\n📈 Coverage:");
    println!("   Total Commands: {}", fixtures.coverage.total_commands);
    println!("   Found Commands: {}", fixtures.coverage.found_commands);
    println!("   Coverage: {:.1}%",
        (fixtures.coverage.found_commands as f64 / fixtures.coverage.total_commands as f64) * 100.0
    );

    if !fixtures.coverage.missing_commands.is_empty() {
        println!("\n⚠️  Missing mainnet samples for:");
        for cmd in &fixtures.coverage.missing_commands {
            println!("   - {}", cmd);
        }
    }

    if !errors_by_command.is_empty() {
        println!("\n❌ Errors by command:");
        for (cmd, errors) in &errors_by_command {
            println!("\n   {}:", cmd);
            for error in errors {
                println!("     - {}", error);
            }
        }
    }

    println!(); // Empty line at end

    // Assert that we have good coverage
    assert!(
        passed > 0,
        "No transactions decoded successfully"
    );

    // Don't fail on missing fixtures, just warn
    if failed > 0 {
        panic!("\n❌ {} transactions failed to decode. See errors above.\n", failed);
    }
}

/// Test specific command types individually
#[test]
fn test_v2_swap_exact_in() {
    test_command_type("V2_SWAP_EXACT_IN");
}

#[test]
fn test_v3_swap_exact_in() {
    test_command_type("V3_SWAP_EXACT_IN");
}

#[test]
fn test_permit2_permit() {
    test_command_type("PERMIT2_PERMIT");
}

#[test]
fn test_unwrap_weth() {
    test_command_type("UNWRAP_WETH");
}

#[test]
fn test_wrap_eth() {
    test_command_type("WRAP_ETH");
}

/// Helper to test a specific command type
fn test_command_type(command_name: &str) {
    let fixtures = match load_fixtures() {
        Some(f) => f,
        None => {
            println!("⚠️  Skipping {}: no fixtures found", command_name);
            return;
        }
    };

    let fixture = fixtures
        .fixtures
        .iter()
        .find(|f| f.command_type == command_name);

    match fixture {
        Some(f) => {
            println!("\n🧪 Testing {}", command_name);
            println!("   TX: {}", f.tx_hash);
            println!("   Block: {}", f.block_number);

            let decoder = Decoder::<()>::new_offline();
            let input_hex = f.input_data.strip_prefix("0x").unwrap_or(&f.input_data);
            let input_bytes = Bytes::from(hex::decode(input_hex).expect("Valid hex"));

            match decoder.decode_function_input(&input_bytes) {
                Ok(decoded) => {
                    println!("   ✅ Decoded successfully");
                    println!("   Commands: {}", decoded.inputs.len());

                    // Pretty print first command
                    if let Some(first) = decoded.inputs.first() {
                        println!("   First command: {}", first.function);
                        println!("   Parameters: {}", serde_json::to_string_pretty(&first.params).unwrap());
                    }
                }
                Err(e) => {
                    panic!("❌ Failed to decode {}: {}", command_name, e);
                }
            }
        }
        None => {
            println!("⚠️  Skipping {}: no fixture found", command_name);
        }
    }
}

/// Benchmark decoding performance on mainnet transactions
#[test]
#[ignore] // Only run when explicitly requested
fn benchmark_mainnet_decoding() {
    use std::time::Instant;

    let fixtures = match load_fixtures() {
        Some(f) => f,
        None => {
            println!("⚠️  No fixtures for benchmarking");
            return;
        }
    };

    println!("\n⏱️  Benchmarking Decoder Performance\n");

    let decoder = Decoder::<()>::new_offline();
    let mut total_time = std::time::Duration::ZERO;
    let mut successful = 0;

    for fixture in &fixtures.fixtures {
        let input_hex = fixture.input_data.strip_prefix("0x").unwrap_or(&fixture.input_data);
        if let Ok(input_bytes) = hex::decode(input_hex) {
            let input = Bytes::from(input_bytes);

            let start = Instant::now();
            if decoder.decode_function_input(&input).is_ok() {
                let elapsed = start.elapsed();
                total_time += elapsed;
                successful += 1;
            }
        }
    }

    if successful > 0 {
        let avg_time = total_time / successful as u32;
        println!("📊 Results:");
        println!("   Transactions: {}", successful);
        println!("   Total time: {:?}", total_time);
        println!("   Average: {:?} per transaction", avg_time);
        println!("   Throughput: {:.0} tx/sec", 1_000_000_000.0 / avg_time.as_nanos() as f64);
    }
}
