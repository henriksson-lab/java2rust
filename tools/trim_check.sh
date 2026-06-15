#!/usr/bin/env bash
# Rebuild translator, re-translate Trimmomatic, compile, and report error histogram.
set -e
cd "$(dirname "$0")/.."
cargo build --release 2>&1 | grep -E '^error' && { echo "TRANSLATOR BUILD FAILED"; exit 1; } || true
rm -rf /tmp/trimmomatic-rs
./target/release/java2rust-rs -d testdata/trimmomatic/src/main/java -o /tmp/trimmomatic-rs --crate -s >/dev/null 2>&1
cd /tmp/trimmomatic-rs
TOTAL=$(cargo build 2>&1 | grep -cE '^error\[|^error:')
echo "=== TOTAL ERRORS: $TOTAL ==="
cargo build 2>&1 | grep -E '^error' | sed -E 's/`[^`]*`/`X`/g' | sort | uniq -c | sort -rn | head -25
