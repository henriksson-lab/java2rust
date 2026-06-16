#!/usr/bin/env bash
# Rebuild translator, re-translate bjalign, compile, and report error histogram.
set -e
cd "$(dirname "$0")/.."
cargo build --release 2>&1 | grep -E '^error' && { echo "TRANSLATOR BUILD FAILED"; exit 1; } || true
rm -rf /tmp/bjalign-rs
./target/release/java2rust-rs -d testdata/biojava_alignment -o /tmp/bjalign-rs --crate -s >/dev/null 2>&1
cd /tmp/bjalign-rs
TOTAL=$(cargo build 2>&1 | grep -cE '^error\[|^error:')
echo "=== TOTAL ERRORS: $TOTAL ==="
cargo build 2>&1 | grep -E '^error' | sed -E 's/`[^`]*`/`X`/g' | sort | uniq -c | sort -rn | head -20
