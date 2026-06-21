#!/usr/bin/env bash
# Rebuild translator, re-translate Trimmomatic, compile, and report error histogram.
set -e
cd "$(dirname "$0")/.."
cargo build --release 2>&1 | grep -E '^error' && { echo "TRANSLATOR BUILD FAILED"; exit 1; } || true
rm -rf /tmp/jahmm-rs
timeout 300 ./target/release/java2rust-rs -d testdata/jahmm/src/main/java -o /tmp/jahmm-rs --crate -s >/dev/null 2>&1
cd /tmp/jahmm-rs
TOTAL=$(cargo build 2>&1 | grep -cE '^error\[|^error:')
echo "=== TOTAL ERRORS: $TOTAL ==="
cargo build 2>&1 | grep -E '^error' | sed -E 's/`[^`]*`/`X`/g' | sort | uniq -c | sort -rn | head -25
