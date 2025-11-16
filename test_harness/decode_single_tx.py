#!/usr/bin/env python3
"""
Quick test script to decode a specific transaction.
Usage: python decode_single_tx.py <rpc_endpoint> <tx_hash>
"""
import sys
import json
from web3 import Web3
from uniswap_universal_router_decoder import RouterCodec


def decode_and_analyze(rpc_endpoint: str, tx_hash: str):
    """Decode a transaction and print detailed analysis."""
    print(f"Decoding transaction: {tx_hash}")
    print("=" * 80)

    codec = RouterCodec(rpc_endpoint=rpc_endpoint)

    try:
        decoded_trx = codec.decode.transaction(tx_hash)

        # Transaction basics
        print(f"\nTransaction Hash: {decoded_trx['hash'].hex()}")
        print(f"From: {decoded_trx['from']}")
        print(f"To: {decoded_trx['to']}")
        print(f"Value: {decoded_trx['value']} wei")
        print(f"Gas Used: {decoded_trx.get('gas', 'N/A')}")

        # Decoded input
        decoded_input = decoded_trx['decoded_input']
        print(f"\nFunction: execute")
        print(f"Deadline: {decoded_input.get('deadline', 'None')}")
        print(f"Commands: {decoded_input['commands'].hex()}")
        print(f"Number of commands: {len(decoded_input['inputs'])}")

        # Command details
        print("\n" + "=" * 80)
        print("COMMAND DETAILS:")
        print("=" * 80)

        for i, cmd in enumerate(decoded_input['inputs']):
            print(f"\nCommand {i + 1}:")

            if isinstance(cmd, str):
                print(f"  Type: UNKNOWN")
                print(f"  Raw hex: {cmd}")
            else:
                func, params, metadata = cmd
                print(f"  Function: {func.fn_name}")
                print(f"  Revert on fail: {metadata.get('revert_on_fail', True)}")
                print(f"  Parameters:")

                for key, value in params.items():
                    if isinstance(value, bytes):
                        value_str = f"0x{value.hex()}"
                        if len(value_str) > 100:
                            value_str = value_str[:100] + "..."
                    elif isinstance(value, list):
                        if len(value) > 0 and isinstance(value[0], str):
                            value_str = f"[{', '.join(value[:3])}{'...' if len(value) > 3 else ''}]"
                        else:
                            value_str = str(value)
                    else:
                        value_str = str(value)

                    print(f"    {key}: {value_str}")

                # Special handling for V3 paths
                if 'path' in params and func.fn_name in ['V3_SWAP_EXACT_IN', 'V3_SWAP_EXACT_OUT']:
                    try:
                        decoded_path = codec.decode.v3_path(func.fn_name, params['path'])
                        print(f"    Decoded V3 path:")
                        for j, element in enumerate(decoded_path):
                            if j % 2 == 0:  # Token
                                print(f"      Token: {element}")
                            else:  # Fee
                                print(f"      Fee: {element} ({element/10000}%)")
                    except Exception as e:
                        print(f"    Could not decode path: {e}")

        # Output as JSON for comparison
        print("\n" + "=" * 80)
        print("JSON OUTPUT (for comparison):")
        print("=" * 80)

        output = {
            "tx_hash": tx_hash,
            "function": "execute",
            "deadline": str(decoded_input.get('deadline', 'None')),
            "commands": decoded_input['commands'].hex(),
            "num_commands": len(decoded_input['inputs']),
            "commands_detail": []
        }

        for cmd in decoded_input['inputs']:
            if isinstance(cmd, str):
                output["commands_detail"].append({
                    "type": "unknown",
                    "raw": cmd
                })
            else:
                func, params, metadata = cmd
                # Serialize params
                serialized_params = {}
                for k, v in params.items():
                    if isinstance(v, bytes):
                        serialized_params[k] = f"0x{v.hex()}"
                    elif isinstance(v, list):
                        serialized_params[k] = [str(item) for item in v]
                    else:
                        serialized_params[k] = str(v)

                output["commands_detail"].append({
                    "function": func.fn_name,
                    "params": serialized_params,
                    "revert_on_fail": metadata.get('revert_on_fail', True)
                })

        print(json.dumps(output, indent=2))

    except Exception as e:
        print(f"\nERROR: {e}")
        import traceback
        traceback.print_exc()
        return 1

    return 0


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("Usage: python decode_single_tx.py <rpc_endpoint> <tx_hash>")
        print("\nExample:")
        print("  python decode_single_tx.py https://eth.llamarpc.com 0x02f904cf0181b7...")
        sys.exit(1)

    rpc_endpoint = sys.argv[1]
    tx_hash = sys.argv[2]

    sys.exit(decode_and_analyze(rpc_endpoint, tx_hash))
