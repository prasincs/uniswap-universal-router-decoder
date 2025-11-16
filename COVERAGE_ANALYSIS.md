# Test Coverage Analysis & Strategy

## Current Coverage Status

### Tested Commands (4/14)
From previous mainnet validation:
- ✅ **PAY_PORTION** (0x06) - 1/1 passed
- ✅ **PERMIT2_PERMIT** (0x0a) - 1/1 passed
- ✅ **UNWRAP_WETH** (0x0c) - 1/1 passed
- ✅ **V2_SWAP_EXACT_IN** (0x08) - 1/1 passed

### Untested Commands (10/14)
- ❌ **V3_SWAP_EXACT_IN** (0x00) - Decoder implemented, no mainnet samples
- ❌ **V3_SWAP_EXACT_OUT** (0x01) - Decoder implemented, no mainnet samples
- ❌ **PERMIT2_TRANSFER_FROM** (0x02) - Decoder implemented, no mainnet samples
- ❌ **SWEEP** (0x04) - Decoder implemented, no mainnet samples
- ❌ **TRANSFER** (0x05) - Decoder implemented, no mainnet samples
- ❌ **V2_SWAP_EXACT_OUT** (0x09) - Decoder implemented, no mainnet samples
- ❌ **WRAP_ETH** (0x0b) - Decoder implemented, no mainnet samples
- ❌ **V4_SWAP** (0x10) - Placeholder decoder only
- ❌ **V4_INITIALIZE_POOL** (0x13) - Placeholder decoder only
- ❌ **V4_POSITION_MANAGER_CALL** (0x14) - Placeholder decoder only

### V4 Actions Coverage (0/16)
All V4 actions currently return placeholder hex:
- SwapExactInSingle, SwapExactIn, SwapExactOutSingle, SwapExactOut
- MintPosition, MintPositionFromDeltas
- SettlePair, TakePair, CloseCurrency, ClearOrTake
- Settle, Take, SettleAll, TakeAll, TakePortion
- Sweep, Wrap, Unwrap

**Current Coverage: 28.6% (4/14 router functions)**

---

## Strategy: Using Alloy + Revm for Comprehensive Coverage

### Why Alloy + Revm?

1. **Alloy**: Modern Ethereum library with full provider, signer, and contract support
   - Already a dependency (alloy-provider, alloy-primitives, alloy-sol-types)
   - Type-safe ABI encoding/decoding
   - Can fetch and replay mainnet transactions

2. **Revm**: Rust EVM implementation
   - Fork mainnet at any block
   - Simulate transactions without RPC calls
   - Fast execution for testing
   - Can create synthetic test scenarios

### Implementation Plan

#### Phase 1: Mainnet Transaction Replay (Alloy)
**Goal**: Find and test real mainnet transactions for all 14 commands

```rust
// Use alloy to fetch historical transactions
// Group by command type
// Validate decoder against real data
```

**Files to create**:
- `tests/integration_mainnet_replay.rs`
- Use alloy-provider to fetch from Etherscan/RPC
- Test all untested command types

#### Phase 2: Revm Fork Testing
**Goal**: Create synthetic test scenarios for edge cases

```rust
// Fork mainnet at block
// Simulate Universal Router calls
// Test decoder against simulated txs
```

**Files to create**:
- `tests/integration_revm_fork.rs`
- Fork mainnet and simulate transactions
- Test error cases, edge cases
- Test V4 commands (once available on mainnet)

#### Phase 3: V4 Action Decoder Implementation
**Goal**: Complete V4 action parameter decoding

Current status: V4 actions return hex placeholders
Need to implement:
- SwapExactInSingle/SwapExactIn parameter decoding
- MintPosition/MintPositionFromDeltas parameter decoding
- Settle/Take variations parameter decoding

**Approach**:
1. Use alloy-sol-types to define V4 action ABIs
2. Implement decoders similar to router commands
3. Test against V4 mainnet transactions (when available)

#### Phase 4: Property-Based Testing (proptest)
**Goal**: Fuzz test encoder/decoder roundtrips

```rust
proptest! {
    #[test]
    fn encode_decode_roundtrip(path: V3Path) {
        let encoded = encode_v3_path(&path);
        let decoded = decode_v3_path(&encoded, false);
        assert_eq!(path, decoded);
    }
}
```

---

## Dependencies to Add

```toml
[dev-dependencies]
# Revm for EVM simulation
revm = { version = "14", features = ["std", "serde"] }
revm-primitives = "9"

# Property-based testing
proptest = "1.5"

# Alloy test utilities (already have alloy-provider)
alloy-rpc-client = "0.6"
alloy-node-bindings = "0.6"  # For anvil integration

# Async runtime for integration tests
tokio = { version = "1", features = ["full"] }
```

---

## Test Structure

```
tests/
├── integration_mainnet_replay.rs   # Real mainnet tx replay
├── integration_revm_fork.rs        # Forked mainnet simulation
├── integration_anvil.rs            # Local anvil testing
├── property_v3_path.rs             # V3 path property tests
├── property_command_encoding.rs    # Command encoding tests
└── fixtures/
    ├── mainnet_txs.json            # Known good transactions
    └── v4_samples.json             # V4 transaction samples
```

---

## Expected Coverage After Implementation

### Router Functions: 100% (14/14)
All commands tested with:
- ✅ Real mainnet transactions
- ✅ Simulated edge cases
- ✅ Error conditions

### V4 Actions: 100% (16/16)
All actions with:
- ✅ Full parameter decoding
- ✅ Mainnet validation
- ✅ Type-safe handling

### Code Coverage Target: 90%+
Using cargo-tarpaulin in CI:
- Unit tests: ~80% coverage
- Integration tests: +10-15% coverage
- Property tests: Edge case coverage

---

## CI/CD Updates

Add new jobs to `rust-ci.yml`:

```yaml
integration-tests:
  name: Integration Tests
  runs-on: ubuntu-latest
  env:
    # Use public RPC for mainnet replay
    ETH_RPC_URL: https://eth.public-rpc.com
  steps:
    - run: cargo test --test integration_mainnet_replay
    - run: cargo test --test integration_revm_fork

property-tests:
  name: Property Tests
  runs-on: ubuntu-latest
  steps:
    - run: cargo test --test property_*
```

---

## Timeline Estimate

- **Phase 1 (Mainnet Replay)**: 2-3 hours
  - Find mainnet txs for untested commands
  - Write integration tests
  - Achieve 100% router function coverage

- **Phase 2 (Revm Fork)**: 3-4 hours
  - Set up revm fork infrastructure
  - Create synthetic test scenarios
  - Test edge cases and errors

- **Phase 3 (V4 Decoders)**: 4-5 hours
  - Research V4 action ABIs
  - Implement parameter decoders
  - Test against V4 mainnet data

- **Phase 4 (Property Tests)**: 2-3 hours
  - Set up proptest framework
  - Write property tests for encode/decode
  - Test invariants

**Total: ~12-15 hours for 100% coverage**

---

## Success Metrics

1. **Coverage**: 100% of router functions tested (14/14)
2. **V4 Complete**: Full V4 action parameter decoding
3. **Validation**: 90%+ pass rate on mainnet transactions
4. **CI**: All tests passing in automated pipeline
5. **Documentation**: Each command type documented with examples

---

## Next Steps

1. ✅ Add revm and proptest dependencies
2. ✅ Create integration test files
3. ✅ Implement mainnet replay tests
4. ✅ Set up revm fork testing
5. ✅ Complete V4 action decoders
6. ✅ Add property-based tests
7. ✅ Update CI/CD
8. ✅ Achieve 100% coverage

This will transform the decoder from "mostly tested" to "battle-tested" with comprehensive coverage of all edge cases and real-world scenarios.
