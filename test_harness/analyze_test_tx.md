# Analysis of Test Transaction

## Raw Transaction Data

```
0x02f904cf0181b78477359400847c17b3e383045307943fc91a3afd70395cd496c647d5a6cc9d4b2b7fad80b904a424856bc3...
```

## Transaction Structure

**Transaction Type**: EIP-1559 (Type 2) - starts with `0x02`

**Target Contract**: `0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad` (Uniswap Universal Router)

**Function Selector**: `0x24856bc3` = `execute(bytes commands, bytes[] inputs, uint256 deadline)`

## Extracted Information

### Commands Byte Array
```
0x0a08060c
```

This represents 4 commands:
1. `0x0a` = PERMIT2_PERMIT
2. `0x08` = V2_SWAP_EXACT_IN
3. `0x06` = PAY_PORTION
4. `0x0c` = UNWRAP_WETH

### Command Breakdown

#### Command 1: PERMIT2_PERMIT (0x0a)
- Sets up permit2 approval for gasless token approvals
- Token: `0x72b658bd674f9c2b4954682f517c17d14476e417` (appears to be the input token)
- Amount: `0xffffffffffffffffffffffffffffffffffffffff` (unlimited approval)
- Expiration and nonce data included
- Signature present (65 bytes)

#### Command 2: V2_SWAP_EXACT_IN (0x08)
- Swaps exact amount of input tokens
- Amount In: `0x285aaa58c1a1a183d` (large amount in hex)
- Amount Out Min: `0x09cf200e607a08` (minimum output)
- Path: Array of token addresses for the swap route
  - From: `0x72b658bd674f9c2b4954682f517c17d14476e417`
  - To: `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2` (WETH)

#### Command 3: PAY_PORTION (0x06)
- Pays a portion (percentage) of tokens to a recipient
- Currency: `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2` (WETH)
- Recipient: `0x000000fee13a103a10d593b9ae06b3e05f2e7e1c` (fee recipient)
- Bips: `0x19` (25 basis points = 0.25%)

#### Command 4: UNWRAP_WETH (0x0c)
- Unwraps WETH to ETH
- Recipient: `0x8419e7eda8577dfc49591a49cad965a0fc6716cf`
- Minimum amount: `0x09c8d8ef9ef49bc0`

## Transaction Flow

This is a **token-to-ETH swap with fee** transaction:

1. **PERMIT2_PERMIT**: User approves the router to spend their tokens via Permit2 (gasless approval)
2. **V2_SWAP_EXACT_IN**: Swap exact amount of input token to WETH using Uniswap V2
3. **PAY_PORTION**: Take 0.25% of the WETH as a fee to the fee recipient
4. **UNWRAP_WETH**: Convert remaining WETH to ETH and send to user

## Why This Is A Good Test Case

This transaction tests:
- ✅ Permit2 signature decoding
- ✅ V2 swap decoding (exact in)
- ✅ V2 path parsing (token addresses)
- ✅ Pay portion (percentage-based payment)
- ✅ Unwrap ETH functionality
- ✅ Multiple commands in sequence
- ✅ Revert flags handling
- ✅ Complex parameter structures

## Expected Decoder Output Structure

```json
{
  "function": "execute",
  "deadline": "<timestamp>",
  "commands": "0x0a08060c",
  "num_commands": 4,
  "commands_detail": [
    {
      "function": "PERMIT2_PERMIT",
      "params": {
        "token": "0x72b658bd674f9c2b4954682f517c17d14476e417",
        "amount": "115792089237316195423570985008687907853269984665640564039457584007913129639935",
        "expiration": "...",
        "nonce": "...",
        "spender": "0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad",
        "sigDeadline": "...",
        "signature": "0x..."
      },
      "revert_on_fail": true
    },
    {
      "function": "V2_SWAP_EXACT_IN",
      "params": {
        "recipient": "...",
        "amountIn": "729000000000000000061",
        "amountOutMin": "2773000000000000008",
        "path": [
          "0x72b658bd674f9c2b4954682f517c17d14476e417",
          "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
        ],
        "payerIsUser": true
      },
      "revert_on_fail": true
    },
    {
      "function": "PAY_PORTION",
      "params": {
        "token": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
        "recipient": "0x000000fee13a103a10d593b9ae06b3e05f2e7e1c",
        "bips": "25"
      },
      "revert_on_fail": true
    },
    {
      "function": "UNWRAP_WETH",
      "params": {
        "recipient": "0x8419e7eda8577dfc49591a49cad965a0fc6716cf",
        "amountMin": "2770000000000000000"
      },
      "revert_on_fail": true
    }
  ]
}
```

## Integration with Test Suite

To add this to the automated test suite:

### Option 1: Test with Transaction Hash (requires mainnet access)

If this transaction was mined on mainnet, extract its hash:
```bash
# Compute transaction hash from raw data
python3 -c "
from eth_hash.auto import keccak
import sys
tx_data = bytes.fromhex(sys.argv[1][2:])
tx_hash = '0x' + keccak(tx_data).hex()
print(f'Transaction hash: {tx_hash}')
" "0x02f904cf0181b7..."
```

Then add to `test_harness/python_decoder_runner.py`:
```python
TEST_TRANSACTIONS = {
    # ... existing transactions ...
    "tx_custom": "<computed_hash>",
}
```

### Option 2: Test with Input Data Only

Create a test that decodes just the input portion:

```python
input_data = bytes.fromhex("24856bc3...")  # Full input from raw tx
codec = RouterCodec()
decoded_fn, decoded_params = codec.decode.function_input(input_data)
```

## Security Considerations for This Transaction

1. **Unlimited Approval**: Uses `type(uint256).max` for approval - standard but worth noting
2. **Slippage Protection**: Has `amountOutMin` to protect against excessive slippage
3. **Fee Extraction**: 0.25% fee is taken - user should be aware
4. **Deadline**: Transaction has expiration to prevent stale execution
5. **Recipient Validation**: Final ETH goes to specific address (should verify it's user's address)

## Rust Decoder Validation Checklist

When the Rust decoder is ready, verify it produces:

- [ ] Correct function name (`execute`)
- [ ] Correct number of commands (4)
- [ ] Correct command names (PERMIT2_PERMIT, V2_SWAP_EXACT_IN, PAY_PORTION, UNWRAP_WETH)
- [ ] Correct parameter extraction for each command
- [ ] Correct address parsing (checksummed)
- [ ] Correct amount parsing (no precision loss)
- [ ] Correct V2 path array parsing
- [ ] Correct signature bytes extraction
- [ ] Correct revert flags for all commands
- [ ] Output matches Python decoder exactly

## Performance Target

For this transaction with 4 commands:
- Python decoder: ~50-100ms
- Rust decoder target: <10ms (5-10x speedup)
