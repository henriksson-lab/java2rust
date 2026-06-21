#!/usr/bin/env bash
# Rebuild translator, re-translate fastq, compile, and report error histogram.
set -e
cd "$(dirname "$0")/.."
cargo build --release 2>&1 | grep -E '^error' && { echo "TRANSLATOR BUILD FAILED"; exit 1; } || true
rm -rf /tmp/fastq-rs
timeout 300 ./target/release/java2rust-rs -d testdata/htsjdk_fastq -o /tmp/fastq-rs --crate -s >/dev/null 2>&1
cd /tmp/fastq-rs
TOTAL=$(cargo build 2>&1 | grep -cE '^error\[|^error:')
echo "=== TOTAL ERRORS: $TOTAL ==="
cargo build 2>&1 | grep -E '^error' | sed -E 's/`[^`]*`/`X`/g' | sort | uniq -c | sort -rn | head -20
