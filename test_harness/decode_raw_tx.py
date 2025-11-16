#!/usr/bin/env python3
"""
Decode a raw transaction or transaction hash.
Handles both raw RLP-encoded transactions and transaction hashes.
"""
import sys
import json
from web3 import Web3
from eth_account import Account
from uniswap_universal_router_decoder import RouterCodec


def decode_raw_transaction_data(raw_tx_data: str):
    """
    Decode raw transaction data and extract the input.
    Returns the transaction input bytes that can be decoded.
    """
    # Remove 0x prefix if present
    if raw_tx_data.startswith('0x'):
        raw_tx_data = raw_tx_data[2:]

    # Convert to bytes
    tx_bytes = bytes.fromhex(raw_tx_data)

    # Decode the transaction
    # Type 2 transactions start with 0x02
    if tx_bytes[0] == 0x02:
        print("Detected EIP-1559 (Type 2) transaction")

        # Use eth_account to decode
        from eth_account._utils.legacy_transactions import Transaction
        from eth_account._utils.typed_transactions import TypedTransaction

        try:
            # Try to decode as typed transaction
            typed_tx = TypedTransaction.from_bytes(tx_bytes)
            print(f"Transaction type: {typed_tx.transaction_type}")

            # Access the actual transaction data
            tx_dict = typed_tx.as_dict()

            return {
                'to': tx_dict.get('to'),
                'input': tx_dict.get('data'),
                'value': tx_dict.get('value', 0),
                'from': tx_dict.get('from'),
                'chainId': tx_dict.get('chainId'),
                'nonce': tx_dict.get('nonce'),
                'gas': tx_dict.get('gas'),
                'maxFeePerGas': tx_dict.get('maxFeePerGas'),
                'maxPriorityFeePerGas': tx_dict.get('maxPriorityFeePerGas'),
            }
        except Exception as e:
            print(f"Error decoding typed transaction: {e}")

            # Fallback: manually parse EIP-1559 transaction
            # This is a simplified parser - may not work for all cases
            from rlp import decode as rlp_decode
            from eth_utils import to_checksum_address

            try:
                # Skip the transaction type byte
                rlp_data = tx_bytes[1:]
                decoded = rlp_decode(rlp_data)

                return {
                    'chainId': int.from_bytes(decoded[0], 'big') if decoded[0] else 1,
                    'nonce': int.from_bytes(decoded[1], 'big') if decoded[1] else 0,
                    'maxPriorityFeePerGas': int.from_bytes(decoded[2], 'big') if decoded[2] else 0,
                    'maxFeePerGas': int.from_bytes(decoded[3], 'big') if decoded[3] else 0,
                    'gas': int.from_bytes(decoded[4], 'big') if decoded[4] else 0,
                    'to': to_checksum_address(decoded[5]) if decoded[5] else None,
                    'value': int.from_bytes(decoded[6], 'big') if decoded[6] else 0,
                    'input': decoded[7],
                }
            except Exception as e2:
                print(f"Error with fallback parsing: {e2}")
                raise

    else:
        # Legacy transaction
        from rlp import decode as rlp_decode
        from eth_utils import to_checksum_address

        decoded = rlp_decode(tx_bytes)

        return {
            'nonce': int.from_bytes(decoded[0], 'big') if decoded[0] else 0,
            'gasPrice': int.from_bytes(decoded[1], 'big') if decoded[1] else 0,
            'gas': int.from_bytes(decoded[2], 'big') if decoded[2] else 0,
            'to': to_checksum_address(decoded[3]) if decoded[3] else None,
            'value': int.from_bytes(decoded[4], 'big') if decoded[4] else 0,
            'input': decoded[5],
        }


def decode_from_input_data(input_data: bytes):
    """Decode router input data without needing blockchain access."""
    codec = RouterCodec()  # No RPC needed for input-only decoding

    try:
        decoded_fn, decoded_params = codec.decode.function_input(input_data)

        print(f"\nFunction: {decoded_fn.fn_name}")
        print(f"Commands: {decoded_params['commands'].hex()}")
        print(f"Number of commands: {len(decoded_params['inputs'])}")

        if 'deadline' in decoded_params:
            print(f"Deadline: {decoded_params['deadline']}")

        print("\n" + "=" * 80)
        print("COMMAND DETAILS:")
        print("=" * 80)

        for i, cmd in enumerate(decoded_params['inputs']):
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
                            value_str = value_str[:50] + "..." + value_str[-50:]
                    elif isinstance(value, list):
                        if len(value) > 0 and isinstance(value[0], str):
                            value_str = f"[{', '.join(str(v) for v in value[:3])}{'...' if len(value) > 3 else ''}]"
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

        return 0

    except Exception as e:
        print(f"\nERROR decoding: {e}")
        import traceback
        traceback.print_exc()
        return 1


def main():
    if len(sys.argv) < 2:
        print("Usage:")
        print("  python decode_raw_tx.py <raw_tx_data>")
        print("  python decode_raw_tx.py <rpc_endpoint> <tx_hash>")
        print("\nExample (raw transaction):")
        print("  python decode_raw_tx.py 0x02f904cf0181b7...")
        print("\nExample (transaction hash):")
        print("  python decode_raw_tx.py https://eth.llamarpc.com 0x1234...")
        sys.exit(1)

    if len(sys.argv) == 2:
        # Raw transaction data
        raw_tx_data = sys.argv[1]
        print("Processing raw transaction data...")
        print("=" * 80)

        try:
            tx_data = decode_raw_transaction_data(raw_tx_data)

            print(f"\nExtracted transaction info:")
            print(f"  To: {tx_data.get('to')}")
            print(f"  Value: {tx_data.get('value', 0)} wei")
            print(f"  Gas: {tx_data.get('gas', 0)}")
            print(f"  Input length: {len(tx_data['input'])} bytes")

            print("\n" + "=" * 80)
            print("DECODING INPUT DATA:")
            print("=" * 80)

            return decode_from_input_data(tx_data['input'])

        except Exception as e:
            print(f"Error processing raw transaction: {e}")
            import traceback
            traceback.print_exc()
            return 1

    elif len(sys.argv) == 3:
        # RPC endpoint + transaction hash
        rpc_endpoint = sys.argv[1]
        tx_hash = sys.argv[2]

        print(f"Fetching transaction from blockchain: {tx_hash}")
        print("=" * 80)

        codec = RouterCodec(rpc_endpoint=rpc_endpoint)

        try:
            decoded_trx = codec.decode.transaction(tx_hash)
            decoded_input = decoded_trx['decoded_input']

            print(f"\nTransaction Hash: {decoded_trx['hash'].hex()}")
            print(f"From: {decoded_trx['from']}")
            print(f"To: {decoded_trx['to']}")

            # Use the same decoding logic
            return decode_from_input_data(decoded_trx['input'])

        except Exception as e:
            print(f"Error fetching/decoding transaction: {e}")
            import traceback
            traceback.print_exc()
            return 1

    else:
        print("Error: Invalid number of arguments")
        return 1


if __name__ == "__main__":
    sys.exit(main())
