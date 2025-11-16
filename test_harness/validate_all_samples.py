#!/usr/bin/env python3
"""
Validate all collected mainnet samples with both Python and Rust decoders.

This script:
1. Loads mainnet transaction samples
2. Decodes each with Python decoder
3. Decodes each with Rust decoder
4. Compares results and reports any discrepancies
5. Generates a comprehensive test report
"""

import json
import subprocess
import sys
from pathlib import Path
from typing import Dict, List, Tuple
from collections import defaultdict
import traceback


class SampleValidator:
    def __init__(self, samples_file: str = "test_outputs/mainnet_samples.json"):
        self.samples_file = Path(samples_file)
        self.results = {
            "passed": [],
            "failed": [],
            "errors": [],
        }
        self.command_stats = defaultdict(lambda: {"passed": 0, "failed": 0, "errors": 0})

    def load_samples(self) -> Dict[str, List[Dict]]:
        """Load sample transactions from file."""
        if not self.samples_file.exists():
            print(f"❌ Samples file not found: {self.samples_file}")
            print(f"   Run: python3 test_harness/fetch_mainnet_samples.py")
            sys.exit(1)

        with open(self.samples_file, 'r') as f:
            samples = json.load(f)

        total = sum(len(txs) for txs in samples.values())
        print(f"✅ Loaded {total} sample transactions across {len(samples)} command types")
        return samples

    def decode_with_python(self, input_hex: str) -> Tuple[bool, Dict]:
        """Decode transaction with Python decoder."""
        try:
            from uniswap_universal_router_decoder import RouterCodec

            codec = RouterCodec()
            input_bytes = bytes.fromhex(input_hex[2:] if input_hex.startswith("0x") else input_hex)

            # The Python decoder returns (function_name, decoded_params)
            result = codec.decode.function_input(input_bytes)

            # Handle both tuple and dict returns
            if isinstance(result, tuple):
                fn_name, params = result
            else:
                fn_name = result.get("function")
                params = result

            # Convert Function object to string
            fn_name = str(fn_name) if hasattr(fn_name, '__str__') else fn_name
            # Extract just the function name if it's a full signature
            if fn_name.startswith("<Function "):
                fn_name = "execute"  # Normalize to just "execute"

            # Extract commands - params might be a NamedTuple or dict
            if hasattr(params, '_asdict'):
                params = params._asdict()

            commands = params.get("commands", b"")
            inputs = params.get("inputs", [])

            # Convert commands to hex if it's bytes
            if isinstance(commands, bytes):
                commands_hex = commands.hex()
            else:
                commands_hex = commands

            # Decode each command
            decoded_commands = []
            for i, (cmd_byte, cmd_input) in enumerate(zip(commands, inputs)):
                cmd_type = cmd_byte if isinstance(cmd_byte, int) else ord(cmd_byte)
                cmd_type_masked = cmd_type & 0x3f
                revert_on_fail = bool(cmd_type & 0x80)

                # Decode command input
                try:
                    cmd_result = codec.decode.function_input(cmd_input)
                    if isinstance(cmd_result, tuple):
                        cmd_fn_name, cmd_params = cmd_result
                    else:
                        cmd_fn_name = cmd_result.get("function")
                        cmd_params = cmd_result

                    # Convert Function object to string
                    cmd_fn_name = str(cmd_fn_name) if hasattr(cmd_fn_name, '__str__') else cmd_fn_name
                    # Clean up function name
                    if cmd_fn_name.startswith("<Function "):
                        # Extract name from "<Function NAME(...)>"
                        import re
                        match = re.search(r'<Function ([A-Z_0-9]+)\(', cmd_fn_name)
                        if match:
                            cmd_fn_name = match.group(1)

                    # Convert NamedTuple to dict if needed
                    if hasattr(cmd_params, '_asdict'):
                        cmd_params = cmd_params._asdict()

                    # Convert any Address/bytes objects to strings for JSON serialization
                    cmd_params = self._json_serialize(cmd_params)

                    decoded_commands.append({
                        "index": i,
                        "function": cmd_fn_name,
                        "revert_on_fail": revert_on_fail,
                        "params": cmd_params,
                    })
                except Exception as e:
                    # If cmd_input is bytes, convert to hex
                    raw_hex = cmd_input.hex() if isinstance(cmd_input, bytes) else str(cmd_input)
                    decoded_commands.append({
                        "index": i,
                        "error": str(e),
                        "raw": raw_hex,
                    })

            return True, {
                "function": fn_name,
                "commands": commands_hex,
                "num_commands": len(decoded_commands),
                "commands_detail": decoded_commands,
            }

        except Exception as e:
            return False, {"error": str(e), "traceback": traceback.format_exc()}

    def _json_serialize(self, obj):
        """Convert objects to JSON-serializable format."""
        if isinstance(obj, dict):
            return {k: self._json_serialize(v) for k, v in obj.items()}
        elif isinstance(obj, (list, tuple)):
            return [self._json_serialize(item) for item in obj]
        elif isinstance(obj, bytes):
            return f"0x{obj.hex()}"
        elif hasattr(obj, '__dict__'):
            return str(obj)
        else:
            return obj

    def decode_with_rust(self, input_hex: str) -> Tuple[bool, Dict]:
        """Decode transaction with Rust decoder."""
        try:
            # Ensure input has 0x prefix
            if not input_hex.startswith("0x"):
                input_hex = f"0x{input_hex}"

            # Call Rust decoder
            result = subprocess.run(
                ["cargo", "run", "--quiet", "--bin", "decode", "--", input_hex],
                cwd="uniswap-router-decoder-rs",
                capture_output=True,
                text=True,
                timeout=30
            )

            if result.returncode != 0:
                return False, {
                    "error": "Rust decoder failed",
                    "stderr": result.stderr,
                    "stdout": result.stdout,
                }

            # Parse JSON output
            output = json.loads(result.stdout)
            return True, output

        except subprocess.TimeoutExpired:
            return False, {"error": "Rust decoder timed out"}
        except json.JSONDecodeError as e:
            return False, {"error": f"Invalid JSON from Rust: {e}", "output": result.stdout}
        except Exception as e:
            return False, {"error": str(e), "traceback": traceback.format_exc()}

    def compare_results(self, python_result: Dict, rust_result: Dict) -> Tuple[bool, List[str]]:
        """Compare Python and Rust decoder results."""
        issues = []

        # Compare function name
        if python_result.get("function") != rust_result.get("function"):
            issues.append(f"Function mismatch: {python_result.get('function')} vs {rust_result.get('function')}")

        # Compare commands bytes
        py_cmds = python_result.get("commands", "").lower()
        rust_cmds = rust_result.get("commands", "").lower().replace("0x", "")
        if py_cmds != rust_cmds:
            issues.append(f"Commands mismatch: {py_cmds} vs {rust_cmds}")

        # Compare number of commands
        if python_result.get("num_commands") != rust_result.get("num_commands"):
            issues.append(
                f"Command count mismatch: {python_result.get('num_commands')} vs {rust_result.get('num_commands')}"
            )

        return len(issues) == 0, issues

    def validate_sample(self, command_type: str, sample: Dict, index: int) -> Dict:
        """Validate a single sample transaction."""
        tx_hash = sample.get("hash", "unknown")
        input_data = sample.get("input", "")

        print(f"\n  [{index}] Tx: {tx_hash[:16]}...")

        # Decode with Python
        py_success, py_result = self.decode_with_python(input_data)
        if not py_success:
            print(f"      ❌ Python decoder failed: {py_result.get('error', 'Unknown error')}")
            return {
                "tx_hash": tx_hash,
                "command_type": command_type,
                "status": "error",
                "error": f"Python decoder failed: {py_result.get('error')}",
            }

        # Decode with Rust
        rust_success, rust_result = self.decode_with_rust(input_data)
        if not rust_success:
            print(f"      ❌ Rust decoder failed: {rust_result.get('error', 'Unknown error')}")
            return {
                "tx_hash": tx_hash,
                "command_type": command_type,
                "status": "error",
                "error": f"Rust decoder failed: {rust_result.get('error')}",
            }

        # Compare results
        match, issues = self.compare_results(py_result, rust_result)

        if match:
            print(f"      ✅ PASSED - Decoders match ({py_result.get('num_commands')} commands)")
            return {
                "tx_hash": tx_hash,
                "command_type": command_type,
                "status": "passed",
                "num_commands": py_result.get("num_commands"),
            }
        else:
            print(f"      ❌ FAILED - Decoders mismatch:")
            for issue in issues:
                print(f"         - {issue}")
            return {
                "tx_hash": tx_hash,
                "command_type": command_type,
                "status": "failed",
                "issues": issues,
                "python_result": py_result,
                "rust_result": rust_result,
            }

    def validate_all(self, samples: Dict[str, List[Dict]]):
        """Validate all sample transactions."""
        print(f"\n{'='*80}")
        print(f"Validating Samples")
        print(f"{'='*80}")

        for command_type, command_samples in sorted(samples.items()):
            print(f"\n{command_type} ({len(command_samples)} samples):")

            for i, sample in enumerate(command_samples, 1):
                result = self.validate_sample(command_type, sample, i)

                status = result["status"]
                if status == "passed":
                    self.results["passed"].append(result)
                    self.command_stats[command_type]["passed"] += 1
                elif status == "failed":
                    self.results["failed"].append(result)
                    self.command_stats[command_type]["failed"] += 1
                else:
                    self.results["errors"].append(result)
                    self.command_stats[command_type]["errors"] += 1

    def print_summary(self):
        """Print validation summary."""
        print(f"\n{'='*80}")
        print(f"Validation Summary")
        print(f"{'='*80}")

        total_passed = len(self.results["passed"])
        total_failed = len(self.results["failed"])
        total_errors = len(self.results["errors"])
        total = total_passed + total_failed + total_errors

        print(f"\nOverall Results:")
        print(f"  ✅ Passed: {total_passed}/{total} ({100*total_passed/total if total > 0 else 0:.1f}%)")
        print(f"  ❌ Failed: {total_failed}/{total} ({100*total_failed/total if total > 0 else 0:.1f}%)")
        print(f"  ⚠️  Errors: {total_errors}/{total} ({100*total_errors/total if total > 0 else 0:.1f}%)")

        print(f"\nResults by Command Type:")
        for command_type in sorted(self.command_stats.keys()):
            stats = self.command_stats[command_type]
            cmd_total = stats["passed"] + stats["failed"] + stats["errors"]
            status_icon = "✅" if stats["failed"] == 0 and stats["errors"] == 0 else "❌"
            print(f"  {status_icon} {command_type:30s}: {stats['passed']}/{cmd_total} passed")

        if self.results["failed"]:
            print(f"\n{'='*80}")
            print(f"Failed Transactions (Decoders Mismatch):")
            print(f"{'='*80}")
            for result in self.results["failed"]:
                print(f"\n  Tx: {result['tx_hash']}")
                print(f"  Command: {result['command_type']}")
                print(f"  Issues:")
                for issue in result.get("issues", []):
                    print(f"    - {issue}")

        if self.results["errors"]:
            print(f"\n{'='*80}")
            print(f"Error Transactions (Decoder Failures):")
            print(f"{'='*80}")
            for result in self.results["errors"]:
                print(f"\n  Tx: {result['tx_hash']}")
                print(f"  Command: {result['command_type']}")
                print(f"  Error: {result.get('error', 'Unknown')}")

    def save_report(self, output_file: str = "test_outputs/validation_report.json"):
        """Save detailed validation report."""
        report = {
            "summary": {
                "total": len(self.results["passed"]) + len(self.results["failed"]) + len(self.results["errors"]),
                "passed": len(self.results["passed"]),
                "failed": len(self.results["failed"]),
                "errors": len(self.results["errors"]),
            },
            "by_command": dict(self.command_stats),
            "results": self.results,
        }

        output_path = Path(output_file)
        output_path.parent.mkdir(parents=True, exist_ok=True)

        with open(output_path, 'w') as f:
            json.dump(report, f, indent=2)

        print(f"\n✅ Detailed report saved to: {output_file}")


def main():
    print(f"""
╔══════════════════════════════════════════════════════════════════════════════╗
║         Uniswap Universal Router - Sample Validator                         ║
║                                                                              ║
║  Validates all collected mainnet samples with Python and Rust decoders      ║
╚══════════════════════════════════════════════════════════════════════════════╝
""")

    validator = SampleValidator()

    # Load samples
    samples = validator.load_samples()

    # Validate all
    validator.validate_all(samples)

    # Print summary
    validator.print_summary()

    # Save report
    validator.save_report()

    # Exit with appropriate code
    if validator.results["failed"] or validator.results["errors"]:
        print(f"\n❌ Validation completed with failures")
        sys.exit(1)
    else:
        print(f"\n✅ All samples validated successfully!")
        sys.exit(0)


if __name__ == "__main__":
    main()
