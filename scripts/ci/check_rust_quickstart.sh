#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
scratch_dir="$(mktemp -d)"
trap 'rm -rf "$scratch_dir"' EXIT

mkdir -p "$scratch_dir/src"
awk '
  /^## 2\. Create a commerce engine/ { in_section = 1; next }
  in_section && /^```rust$/ { in_block = 1; next }
  in_block && /^```$/ { exit }
  in_block { print }
' "$repo_root/QUICKSTART.md" > "$scratch_dir/src/main.rs"

if ! grep -q 'fn main()' "$scratch_dir/src/main.rs"; then
  echo "error: could not extract the runnable Rust quickstart from QUICKSTART.md" >&2
  exit 1
fi

cat > "$scratch_dir/Cargo.toml" <<EOF
[package]
name = "stateset-rust-quickstart-smoke"
version = "0.0.0"
edition = "2024"

[workspace]

[dependencies]
stateset-sdk = { path = "$repo_root/crates/stateset-sdk", features = ["full"] }
EOF

target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
quickstart_output="$(
  cd "$scratch_dir"
  CARGO_TARGET_DIR="$target_dir" cargo run --quiet
)"
printf '%s\n' "$quickstart_output"
for expected in 'Customer: Alice' 'Stock: 100 units' 'Order: ORD-' '— $99.98' 'Payment:' 'Shipment:' 'Orders created: 1' 'Payments completed: 1'; do
  if [[ "$quickstart_output" != *"$expected"* ]]; then
    echo "error: Rust quickstart output is missing: $expected" >&2
    exit 1
  fi
done

# The HTTP section is a separate program and should compile with exactly the
# extra dependencies its preceding commands tell readers to add.
mkdir -p "$scratch_dir/src/bin"
awk '
  /^## 4\. Serve the REST API/ { in_section = 1; next }
  in_section && /^```rust$/ { in_block = 1; next }
  in_block && /^```$/ { exit }
  in_block { print }
' "$repo_root/QUICKSTART.md" > "$scratch_dir/src/bin/http.rs"

if ! grep -q 'ServerBuilder' "$scratch_dir/src/bin/http.rs"; then
  echo "error: could not extract the Rust HTTP quickstart from QUICKSTART.md" >&2
  exit 1
fi

cat >> "$scratch_dir/Cargo.toml" <<EOF
stateset-http = { path = "$repo_root/crates/stateset-http" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
EOF

CARGO_TARGET_DIR="$target_dir" cargo check --quiet --manifest-path "$scratch_dir/Cargo.toml" --bin http
