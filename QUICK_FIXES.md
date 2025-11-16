# Quick Fixes for Clippy Warnings

Apply these fixes to resolve all Clippy warnings:

## 1. Remove Unused Imports

### src/decoder.rs
```rust
// Line 8 - REMOVE this unused import:
- use crate::constants::RouterCommands;
```

### src/v3_path.rs
```rust
// Line 166 in test module - REMOVE:
- use hex;
```

## 2. Fix Unused Variable

### src/decoder.rs (line ~584)
```rust
// Change:
- let decoder = Decoder::<()>::new_offline();
// To:
+ let _decoder = Decoder::<()>::new_offline();
// Or remove if truly unused
```

## 3. Use `.is_multiple_of()` (Rust 1.80+)

### src/v3_path.rs (line 28)
```rust
// Change:
- if path.len() % 23 != 0 {
// To:
+ if !path.len().is_multiple_of(23) {
```

Note: This requires Rust 1.80+. If using older version, keep as-is or add feature gate.

## 4. Fix Hex Literal Casing

### src/constants.rs (line 19)
Use consistent lowercase for hex literals or checksummed addresses.

## 5. Performance Optimizations

### src/decoder.rs

```rust
// Line ~136 - Pre-allocate Vec:
- let mut decoded = Vec::new();
+ let mut decoded = Vec::with_capacity(commands.len());

// Line ~102 - Use static str:
- function_name: "execute".to_string(),
+ function_name: "execute".into(),  // Slightly better
// Or define const:
const EXECUTE_FN: &str = "execute";
// ...
function_name: EXECUTE_FN.to_string(),
```

## 6. Add Documentation (suppress warnings for now)

### src/lib.rs
Add at top of modules that need docs:
```rust
#![allow(missing_docs)]  // TODO: Add documentation
```

Or better, add actual docs:
```rust
/// Constants used for router decoding
pub mod constants;

/// Main decoder implementation
pub mod decoder;

/// Router function and action enums
pub mod enums;

/// Error types
pub mod error;

/// Type definitions
pub mod types;

/// V3 path encoding/decoding utilities
pub mod v3_path;
```

## 7. CLI Binary Improvements

### src/bin/decode.rs (line 42-43)
These debug prints should use `--verbose` flag or be removed for production:

```rust
// Option 1: Remove debug output
- eprintln!("Input length: {} bytes", input_bytes.len());
- eprintln!("Selector: 0x{}", hex::encode(&input_bytes[0..4.min(input_bytes.len())]));

// Option 2: Add verbose flag
use clap::Parser;

#[derive(Parser)]
struct Args {
    input_hex: String,
    #[arg(short, long)]
    verbose: bool,
}

// Then:
if args.verbose {
    eprintln!("Input length: {} bytes", input_bytes.len());
    eprintln!("Selector: 0x{}", hex::encode(&input_bytes[0..4.min(input_bytes.len())]));
}
```

## Run These Commands

```bash
# 1. Check for issues:
cargo clippy --all-targets --all-features

# 2. Auto-fix what's possible:
cargo clippy --fix --all-targets --allow-dirty

# 3. Format code:
cargo fmt

# 4. Check docs:
cargo doc --no-deps --open

# 5. Run tests:
cargo test

# 6. Build in release mode:
cargo build --release
```

## Recommended Cargo.toml Additions

```toml
[lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[lints.clippy]
# Pedantic lints
all = "warn"
pedantic = "warn"

# Allow some pedantic lints that are too strict:
module_name_repetitions = "allow"
must_use_candidate = "allow"
```

## CI/CD Quality Gates

Add to `.github/workflows/rust.yml`:

```yaml
- name: Clippy
  run: cargo clippy --all-targets --all-features -- -D warnings

- name: Format check
  run: cargo fmt -- --check

- name: Doc check
  run: cargo doc --no-deps
```
