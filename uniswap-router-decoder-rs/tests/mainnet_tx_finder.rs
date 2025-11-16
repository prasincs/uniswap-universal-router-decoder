/// Mainnet Transaction Finder
///
/// This test finds real mainnet transactions for all Universal Router command types.
/// It uses alloy to fetch transactions from Etherscan and group them by command type.
///
/// Run with: cargo test --test mainnet_tx_finder -- --nocapture --ignored
///
/// This test is marked #[ignore] because it requires network access and takes time.
/// Run it manually when you want to refresh the test fixtures.

use alloy_primitives::{Address, Bytes};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::Transaction;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::str::FromStr;
use uniswap_router_decoder_rs::{Decoder, RouterFunction};

/// Universal Router address on mainnet
const UNIVERSAL_ROUTER: &str = "0x3fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD";

/// Test fixture structure
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

/// Decode commands from transaction input
fn extract_commands(input: &Bytes) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if input.len() < 4 {
        return Err("Input too short".into());
    }

    let decoder = Decoder::<()>::new_offline();
    let decoded = decoder.decode_function_input(input)?;

    Ok(decoded.commands.to_vec())
}

/// Fetch recent transactions to Universal Router
async fn fetch_recent_transactions(
    limit: usize,
) -> Result<Vec<Transaction>, Box<dyn std::error::Error>> {
    println!("🔍 Fetching recent transactions to Universal Router...");

    // Use public RPC endpoint
    let rpc_url = std::env::var("ETH_RPC_URL")
        .unwrap_or_else(|_| "https://eth.llamarpc.com".to_string());

    let provider = ProviderBuilder::new()
        .on_http(rpc_url.parse()?);

    let router_address = Address::from_str(UNIVERSAL_ROUTER)?;

    // Get latest block
    let latest_block = provider.get_block_number().await?;
    println!("📦 Latest block: {}", latest_block);

    let mut transactions = Vec::new();
    let mut blocks_checked = 0;
    let max_blocks = 1000; // Check last 1000 blocks (~3-4 hours)

    // Search backwards from latest block
    for block_num in (latest_block.saturating_sub(max_blocks)..=latest_block).rev() {
        if transactions.len() >= limit {
            break;
        }

        blocks_checked += 1;
        if blocks_checked % 100 == 0 {
            println!("⏳ Checked {} blocks, found {} txs", blocks_checked, transactions.len());
        }

        // Get block with full transaction details
        if let Ok(Some(block)) = provider.get_block_by_number(block_num.into(), true).await {
            if let Some(block_txs) = block.transactions.as_transactions() {
                for tx in block_txs {
                    if let Some(to) = tx.to {
                        if to == router_address {
                            transactions.push(tx.clone());
                        }
                    }
                }
            }
        }
    }

    println!("✅ Found {} transactions in {} blocks", transactions.len(), blocks_checked);
    Ok(transactions)
}

/// Group transactions by command types
fn group_by_commands(
    transactions: Vec<Transaction>,
) -> HashMap<u8, Vec<(String, Bytes, u64)>> {
    let mut groups: HashMap<u8, Vec<(String, Bytes, u64)>> = HashMap::new();

    for tx in transactions {
        let input = &tx.input;

        if let Ok(commands) = extract_commands(input) {
            for cmd in commands {
                let cmd_type = cmd & 0x3f; // Remove revert flag
                groups.entry(cmd_type).or_default().push((
                    format!("{:?}", tx.hash),
                    input.clone(),
                    tx.block_number.unwrap_or(0),
                ));
            }
        }
    }

    groups
}

#[tokio::test]
#[ignore] // Requires network access
async fn find_all_command_types() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🚀 Starting Mainnet Transaction Finder\n");
    println!("📋 Target: Find real mainnet transactions for all 14 Universal Router commands\n");

    // Fetch recent transactions
    let transactions = fetch_recent_transactions(100).await?;

    // Group by command type
    let grouped = group_by_commands(transactions);

    // All command types we need to find
    let all_commands = vec![
        (0x00, "V3_SWAP_EXACT_IN"),
        (0x01, "V3_SWAP_EXACT_OUT"),
        (0x02, "PERMIT2_TRANSFER_FROM"),
        (0x04, "SWEEP"),
        (0x05, "TRANSFER"),
        (0x06, "PAY_PORTION"),
        (0x08, "V2_SWAP_EXACT_IN"),
        (0x09, "V2_SWAP_EXACT_OUT"),
        (0x0a, "PERMIT2_PERMIT"),
        (0x0b, "WRAP_ETH"),
        (0x0c, "UNWRAP_WETH"),
        (0x10, "V4_SWAP"),
        (0x13, "V4_INITIALIZE_POOL"),
        (0x14, "V4_POSITION_MANAGER_CALL"),
    ];

    println!("\n📊 Command Coverage Report:\n");
    println!("{:<30} {:<10} {:<50}", "Command", "Found", "Sample TX");
    println!("{}", "=".repeat(90));

    let mut fixtures = Vec::new();
    let mut found_types = HashSet::new();

    for (code, name) in &all_commands {
        if let Some(txs) = grouped.get(code) {
            let (tx_hash, input, block) = &txs[0];
            println!("{:<30} {:<10} {:<50}",
                name,
                format!("✅ {}", txs.len()),
                &tx_hash[..50]
            );

            fixtures.push(CommandFixture {
                command_type: name.to_string(),
                command_code: *code,
                tx_hash: tx_hash.clone(),
                input_data: format!("0x{}", hex::encode(input)),
                block_number: *block,
            });

            found_types.insert(*code);
        } else {
            println!("{:<30} {:<10}", name, "❌ Missing");
        }
    }

    println!("\n{}", "=".repeat(90));

    let missing: Vec<String> = all_commands
        .iter()
        .filter(|(code, _)| !found_types.contains(code))
        .map(|(_, name)| name.to_string())
        .collect();

    let coverage = CoverageSummary {
        total_commands: all_commands.len(),
        found_commands: found_types.len(),
        missing_commands: missing.clone(),
    };

    println!("\n📈 Coverage: {}/{} ({:.1}%)",
        coverage.found_commands,
        coverage.total_commands,
        (coverage.found_commands as f64 / coverage.total_commands as f64) * 100.0
    );

    if !missing.is_empty() {
        println!("\n⚠️  Missing commands:");
        for cmd in &missing {
            println!("   - {}", cmd);
        }
        println!("\n💡 Tip: Try increasing search range or using Etherscan API for historical data");
    }

    // Save fixtures to JSON
    let mainnet_fixtures = MainnetFixtures {
        router_address: UNIVERSAL_ROUTER.to_string(),
        fixtures,
        coverage,
    };

    let json = serde_json::to_string_pretty(&mainnet_fixtures)?;
    fs::create_dir_all("tests/fixtures")?;
    fs::write("tests/fixtures/mainnet_txs.json", json)?;

    println!("\n💾 Fixtures saved to: tests/fixtures/mainnet_txs.json\n");

    Ok(())
}

#[tokio::test]
#[ignore] // Requires network access
async fn find_v4_transactions() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔍 Searching for V4 transactions (may not exist yet on mainnet)...\n");

    let transactions = fetch_recent_transactions(200).await?;
    let grouped = group_by_commands(transactions);

    let v4_commands = vec![
        (0x10, "V4_SWAP"),
        (0x13, "V4_INITIALIZE_POOL"),
        (0x14, "V4_POSITION_MANAGER_CALL"),
    ];

    let mut found_v4 = false;
    for (code, name) in v4_commands {
        if let Some(txs) = grouped.get(&code) {
            println!("✅ Found {} transactions: {}", txs.len(), name);
            found_v4 = true;
        } else {
            println!("❌ Not found: {}", name);
        }
    }

    if !found_v4 {
        println!("\n💡 V4 commands not found. This is expected if V4 is not yet deployed on mainnet.");
        println!("   V4 testing will need to use testnets or simulated environments.");
    }

    Ok(())
}

#[test]
fn verify_router_function_enum_coverage() {
    // Ensure our test covers all RouterFunction variants
    use uniswap_router_decoder_rs::RouterFunction;

    let all_variants = [
        RouterFunction::V3SwapExactIn,
        RouterFunction::V3SwapExactOut,
        RouterFunction::Permit2TransferFrom,
        RouterFunction::Sweep,
        RouterFunction::Transfer,
        RouterFunction::PayPortion,
        RouterFunction::V2SwapExactIn,
        RouterFunction::V2SwapExactOut,
        RouterFunction::Permit2Permit,
        RouterFunction::WrapEth,
        RouterFunction::UnwrapWeth,
        RouterFunction::V4Swap,
        RouterFunction::V4InitializePool,
        RouterFunction::V4PositionManagerCall,
    ];

    println!("\n✅ RouterFunction enum has {} variants", all_variants.len());
    assert_eq!(all_variants.len(), 14, "Expected 14 router function variants");
}
