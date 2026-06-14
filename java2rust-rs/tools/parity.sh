#!/usr/bin/env bash
# Parity test against a real Java tree: convert every .java file with both the
# original java2rust.jar and this Rust port, then classify the per-file results.
#
# Usage: tools/parity.sh <java-src-dir>
#   e.g. tools/parity.sh ../htsjdk/src/main/java
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
jar="${JAVA2RUST_JAR:-$here/../java-to-rust/java2rust.jar}"
src="${1:?usage: parity.sh <java-src-dir>}"
jout="$(mktemp -d)"; rout="$(mktemp -d)"

javac -cp "$jar" -d "$here/tools" "$here/tools/BatchConvert.java"
java -Xss16m -cp "$jar:$here/tools" BatchConvert "$src" "$jout"
cargo run -q --release --manifest-path "$here/Cargo.toml" --example batch -- "$src" "$rout"

strip() { grep -vE "^\s*(//|/\*|\*)" "$1" | sed "s/[[:space:]]*$//"; }
total=0; same=0; co=0; cd=0; rp=0
while IFS= read -r f; do
  rel="${f#"$jout"/}"; r="$rout/$rel"; total=$((total+1))
  if grep -q "RUST_PANIC" "$r" 2>/dev/null; then rp=$((rp+1)); continue; fi
  if diff -q "$f" "$r" >/dev/null 2>&1; then same=$((same+1)); continue; fi
  if diff <(strip "$f") <(strip "$r") >/dev/null 2>&1; then co=$((co+1)); else cd=$((cd+1)); fi
done < <(find "$jout" -name '*.rs')

echo "files=$total  identical=$same  comment-only-diff=$co  code-diff=$cd  panic=$rp"
echo "jar output:  $jout"
echo "rust output: $rout"
