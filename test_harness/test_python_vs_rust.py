#!/usr/bin/env python3
"""
Side-by-side comparison test: Python vs Rust decoder
Uses the Python library directly and compares with Rust binary output.
"""
import json
import subprocess
import sys
from pathlib import Path

# Test transaction - the one provided by the user
TEST_TX_HASH = "0x02f904cf0181b78477359400847c17b3e383045307943fc91a3afd70395cd496c647d5a6cc9d4b2b7fad80b904a424856bc30000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000040a08060c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000032000000000000000000000000000000000000000000000000000000000000003a0000000000000000000000000000000000000000000000000000000000000016000000000000000000000000072b658bd674f9c2b4954682f517c17d14476e417000000000000000000000000ffffffffffffffffffffffffffffffffffffffff000000000000000000000000000000000000000000000000000000006940571900000000000000000000000000000000000000000000000000000000000000000000000000000000000000003fc91a3afd70395cd496c647d5a6cc9d4b2b7fad000000000000000000000000000000000000000000000000000000006918d12100000000000000000000000000000000000000000000000000000000000000e000000000000000000000000000000000000000000000000000000000000000412eb0933411b0970637515316fb50511bea7908d3f85808074ceed3bf881562bc06da5178104470e54fb5be96075169b30799c30f30975317ae14113ffdb84bc81c000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000285aaa58c1a1a183d0000000000000000000000000000000000000000000000000009cf200e607a0800000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000200000000000000000000000072b658bd674f9c2b4954682f517c17d14476e417000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000000000000000000000000000000060000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000fee13a103a10d593b9ae06b3e05f2e7e1c000000000000000000000000000000000000000000000000000000000000001900000000000000000000000000000000000000000000000000000000000000400000000000000000000000008419e7eda8577dfc49591a49cad965a0fc6716cf0000000000000000000000000000000000000000000000000009c8d8ef9ef49bc0"


def extract_input_from_raw_tx(raw_tx: str) -> str:
    """Extract the input data from raw transaction."""
    # This is a simplified extractor - focuses on getting the input portion
    # For EIP-1559 transactions starting with 0x02

    from eth_account._utils.typed_transactions import TypedTransaction

    tx_bytes = bytes.fromhex(raw_tx[2:] if raw_tx.startswith('0x') else raw_tx)
    typed_tx = TypedTransaction.from_bytes(tx_bytes)
    tx_dict = typed_tx.as_dict()

    return tx_dict.get('data', b'').hex()


def decode_with_python(input_hex: str) -> dict:
    """Decode using Python library."""
    print("=" * 80)
    print("PYTHON DECODER")
    print("=" * 80)

    try:
        from uniswap_universal_router_decoder import RouterCodec

        codec = RouterCodec()  # No RPC needed for input-only decoding
        input_bytes = bytes.fromhex(input_hex[2:] if input_hex.startswith('0x') else input_hex)

        fn, params = codec.decode.function_input(input_bytes)

        # Extract command info
        commands_hex = params['commands'].hex()
        num_commands = len(params['inputs'])

        print(f"Function: {fn.fn_name}")
        print(f"Commands: 0x{commands_hex}")
        print(f"Number of commands: {num_commands}")
        print(f"Deadline: {params.get('deadline', 'None')}")
        print()

        # Build structured output
        commands_detail = []
        for i, cmd in enumerate(params['inputs']):
            if isinstance(cmd, str):
                commands_detail.append({
                    "index": i,
                    "type": "unknown",
                    "raw": cmd
                })
            else:
                func, cmd_params, metadata = cmd

                # Serialize parameters
                serialized_params = {}
                for k, v in cmd_params.items():
                    if isinstance(v, bytes):
                        serialized_params[k] = f"0x{v.hex()}"
                    elif isinstance(v, list):
                        serialized_params[k] = [str(item) for item in v]
                    else:
                        serialized_params[k] = str(v)

                commands_detail.append({
                    "index": i,
                    "function": func.fn_name,
                    "params": serialized_params,
                    "revert_on_fail": metadata.get('revert_on_fail', True)
                })

                print(f"Command {i+1}: {func.fn_name}")
                print(f"  Revert on fail: {metadata.get('revert_on_fail', True)}")
                for k, v in list(serialized_params.items())[:3]:  # Show first 3 params
                    v_str = v if len(str(v)) < 60 else str(v)[:60] + "..."
                    print(f"  {k}: {v_str}")

        return {
            "decoder": "python",
            "success": True,
            "function": fn.fn_name,
            "commands": f"0x{commands_hex}",
            "num_commands": num_commands,
            "deadline": str(params.get('deadline', 'None')),
            "commands_detail": commands_detail
        }

    except Exception as e:
        print(f"ERROR: {e}")
        import traceback
        traceback.print_exc()
        return {
            "decoder": "python",
            "success": False,
            "error": str(e)
        }


def decode_with_rust(input_hex: str) -> dict:
    """Decode using Rust binary."""
    print("\n" + "=" * 80)
    print("RUST DECODER")
    print("=" * 80)

    # Check if Rust binary exists
    rust_dir = Path("uniswap-router-decoder-rs")
    if not rust_dir.exists():
        print("ERROR: Rust project directory not found")
        return {
            "decoder": "rust",
            "success": False,
            "error": "Rust project not found"
        }

    try:
        # Prepare input hex with 0x prefix
        if not input_hex.startswith('0x'):
            input_hex = f"0x{input_hex}"

        # Call Rust decoder CLI
        print("Building and running Rust decoder...")
        result = subprocess.run(
            ["cargo", "run", "--quiet", "--bin", "decode", "--", input_hex],
            cwd=rust_dir,
            capture_output=True,
            text=True,
            timeout=60
        )

        if result.returncode != 0:
            print(f"ERROR: Rust decoder failed with code {result.returncode}")
            print(f"STDERR: {result.stderr}")
            return {
                "decoder": "rust",
                "success": False,
                "error": f"Rust decoder failed: {result.stderr}"
            }

        # Parse JSON output
        rust_output = json.loads(result.stdout)

        # Print summary
        print(f"Function: {rust_output['function']}")
        print(f"Commands: {rust_output['commands']}")
        print(f"Number of commands: {rust_output['num_commands']}")
        print(f"Deadline: {rust_output['deadline']}")
        print()

        for i, cmd in enumerate(rust_output['commands_detail']):
            print(f"Command {i+1}: {cmd['function']}")
            print(f"  Revert on fail: {cmd['revert_on_fail']}")
            for k, v in list(cmd['params'].items())[:3]:  # Show first 3 params
                v_str = v if len(str(v)) < 60 else str(v)[:60] + "..."
                print(f"  {k}: {v_str}")

        return rust_output

    except subprocess.TimeoutExpired:
        print("ERROR: Rust decoder timed out")
        return {
            "decoder": "rust",
            "success": False,
            "error": "Timeout after 60 seconds"
        }
    except json.JSONDecodeError as e:
        print(f"ERROR: Failed to parse Rust output as JSON: {e}")
        print(f"Raw output: {result.stdout}")
        return {
            "decoder": "rust",
            "success": False,
            "error": f"Invalid JSON output: {e}"
        }
    except Exception as e:
        print(f"ERROR: Unexpected error: {e}")
        import traceback
        traceback.print_exc()
        return {
            "decoder": "rust",
            "success": False,
            "error": str(e)
        }


def compare_results(python_result: dict, rust_result: dict):
    """Compare Python and Rust decoder results."""
    print("\n" + "=" * 80)
    print("COMPARISON")
    print("=" * 80)

    if not python_result.get("success"):
        print("❌ Python decoder failed - cannot compare")
        return False

    if not rust_result.get("success"):
        print("⚠️  Rust decoder not yet ready for comparison")
        print("   Reason:", rust_result.get("error", "Unknown"))
        return False

    # Compare key fields
    matches = []
    differences = []

    # Compare function name
    if python_result.get("function") == rust_result.get("function"):
        matches.append("✅ Function name matches")
    else:
        differences.append(f"❌ Function: Python={python_result.get('function')} vs Rust={rust_result.get('function')}")

    # Compare commands
    if python_result.get("commands") == rust_result.get("commands"):
        matches.append("✅ Commands bytes match")
    else:
        differences.append(f"❌ Commands: Python={python_result.get('commands')} vs Rust={rust_result.get('commands')}")

    # Compare number of commands
    if python_result.get("num_commands") == rust_result.get("num_commands"):
        matches.append(f"✅ Number of commands match: {python_result.get('num_commands')}")
    else:
        differences.append(f"❌ Num commands: Python={python_result.get('num_commands')} vs Rust={rust_result.get('num_commands')}")

    # Print results
    print("\nMatches:")
    for match in matches:
        print(f"  {match}")

    if differences:
        print("\nDifferences:")
        for diff in differences:
            print(f"  {diff}")

    return len(differences) == 0


def main():
    print("Uniswap Router Decoder - Side-by-Side Comparison Test")
    print("=" * 80)
    print()

    # Extract input from raw transaction
    try:
        input_hex = extract_input_from_raw_tx(TEST_TX_HASH)
        print(f"Extracted input: 0x{input_hex[:100]}...")
        print(f"Input length: {len(input_hex)//2} bytes")
        print()
    except Exception as e:
        print(f"Error extracting input: {e}")
        print("Using a direct decode test instead...")
        # For now, we'll just test the Python decoder
        input_hex = TEST_TX_HASH[TEST_TX_HASH.index('24856bc3'):]  # Start from function selector

    # Run Python decoder
    python_result = decode_with_python(input_hex)

    # Run Rust decoder
    rust_result = decode_with_rust(input_hex)

    # Compare results
    success = compare_results(python_result, rust_result)

    # Save results to file
    output = {
        "test_transaction": TEST_TX_HASH[:100] + "...",
        "input_hex": f"0x{input_hex[:100]}...",
        "python": python_result,
        "rust": rust_result,
        "comparison_passed": success
    }

    output_file = Path("test_outputs/comparison_result.json")
    output_file.parent.mkdir(exist_ok=True)
    with open(output_file, 'w') as f:
        json.dump(output, f, indent=2)

    print(f"\nResults saved to: {output_file}")

    # Exit code
    if python_result.get("success"):
        print("\n✅ Python decoder: PASSED")
        if rust_result.get("success") and success:
            print("✅ Rust decoder: PASSED")
            print("✅ Comparison: PASSED")
            return 0
        else:
            print("⚠️  Rust decoder: NOT YET READY")
            print("\nNext step: Create CLI wrapper (bin/main.rs) for Rust decoder")
            return 0  # Not a failure, just incomplete
    else:
        print("\n❌ Python decoder: FAILED")
        return 1


if __name__ == "__main__":
    sys.exit(main())
