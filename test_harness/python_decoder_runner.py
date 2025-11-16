#!/usr/bin/env python3
"""
Python decoder runner for test harness.
Decodes Uniswap Universal Router transactions and outputs structured JSON.
"""
import json
import sys
from typing import Any, Dict
from web3 import Web3
from uniswap_universal_router_decoder import RouterCodec


# Mainnet transaction hashes from test_decoder.py
TEST_TRANSACTIONS = {
    "tx_01": "0x52e63b75f41a352ad9182f9e0f923c8557064c3b1047d1778c1ea5b11b979dd9",
    "tx_02": "0x3247555a5dbc877ade17c4b49362bc981af5fb5064e0b3cbd91411e085fe3093",
    "tx_03": "0x889b34a27b730dd664cd71579b4310522c3b495fb34f17f08d1131c0cec651fa",
    "tx_04": "0xf99ac4237df313794747759550db919b37d7c8a67d4a7e12be8f5cbaacd51376",
    "tx_05": "0x47c0f1dd13edf9f1608f9f34bdba9ad40cb95dd081033cad69f5b88e451b4b55",
    "tx_06": "0xe648089f71b2d2e7b70bdcbfdcfeecce6c5248b8eb64b2c79089b7c74c835a45",
    "tx_07": "0xb5a64e9935b46282080d9198f4478a4c4c1993d590eab8daa0f220c0dca5fe33",
    "tx_08": "0x62176a906ef7f178814a0924d390082053bd8992c2902f436756194693644c21",
    "tx_09": "0x2b6af8ef8fe18829a0fcf2b0f391c55daf76f53bb68369ecaefdb1f38045f919",
    "tx_10": "0x586d51e2f92bd16573f0e7e302755ed02b7c2a4b721d63f46bcdcf7179d2f40e",
}


def serialize_web3_value(obj: Any) -> Any:
    """Convert Web3 types to JSON-serializable types."""
    if hasattr(obj, 'hex'):
        return obj.hex()
    elif isinstance(obj, bytes):
        return '0x' + obj.hex()
    elif isinstance(obj, (list, tuple)):
        return [serialize_web3_value(item) for item in obj]
    elif isinstance(obj, dict):
        return {k: serialize_web3_value(v) for k, v in obj.items()}
    elif hasattr(obj, '__dict__'):
        # Handle Web3 contract functions
        if hasattr(obj, 'fn_name'):
            return obj.fn_name
        return serialize_web3_value(obj.__dict__)
    else:
        return obj


def decode_transaction(rpc_endpoint: str, tx_hash: str) -> Dict[str, Any]:
    """
    Decode a transaction using the Python RouterCodec.

    Returns a structured dict with:
    - tx_hash: the transaction hash
    - function_name: the main router function (execute)
    - commands: list of command bytes
    - decoded_commands: list of decoded command details
    """
    try:
        codec = RouterCodec(rpc_endpoint=rpc_endpoint)
        decoded_trx = codec.decode.transaction(tx_hash)

        # Extract key information
        decoded_input = decoded_trx['decoded_input']
        commands = decoded_input['inputs']

        # Process commands
        decoded_commands = []
        for cmd in commands:
            if isinstance(cmd, str):
                # Raw hex string (unknown command)
                decoded_commands.append({
                    "type": "unknown",
                    "raw_hex": cmd
                })
            else:
                # Decoded command (tuple of function, params, metadata)
                func, params, metadata = cmd
                decoded_commands.append({
                    "type": "decoded",
                    "function_name": func.fn_name if hasattr(func, 'fn_name') else str(func),
                    "params": serialize_web3_value(params),
                    "revert_on_fail": metadata.get('revert_on_fail', True)
                })

        result = {
            "tx_hash": tx_hash,
            "success": True,
            "function_name": "execute",
            "commands_raw": serialize_web3_value(decoded_input['commands']),
            "deadline": serialize_web3_value(decoded_input.get('deadline')),
            "num_commands": len(decoded_commands),
            "decoded_commands": decoded_commands
        }

        return result

    except Exception as e:
        return {
            "tx_hash": tx_hash,
            "success": False,
            "error": str(e),
            "error_type": type(e).__name__
        }


def main():
    """
    Main entry point.
    Usage: python python_decoder_runner.py <rpc_endpoint> <tx_hash> [tx_hash...]
    """
    if len(sys.argv) < 3:
        print("Usage: python_decoder_runner.py <rpc_endpoint> <tx_hash> [tx_hash...]", file=sys.stderr)
        print("   or: python_decoder_runner.py <rpc_endpoint> ALL  (decode all test transactions)", file=sys.stderr)
        sys.exit(1)

    rpc_endpoint = sys.argv[1]

    # Handle "ALL" keyword to decode all test transactions
    if len(sys.argv) == 3 and sys.argv[2].upper() == "ALL":
        tx_hashes = list(TEST_TRANSACTIONS.values())
    else:
        tx_hashes = sys.argv[2:]

    results = []
    for tx_hash in tx_hashes:
        print(f"Decoding {tx_hash}...", file=sys.stderr)
        result = decode_transaction(rpc_endpoint, tx_hash)
        results.append(result)

    # Output results as JSON
    output = {
        "decoder": "python",
        "version": "original",
        "results": results
    }
    print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()
