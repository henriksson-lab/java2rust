#!/usr/bin/env bash
# End-to-end producer + real shell assembly, not just the Python merger.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
[[ $# == 1 && -n "$1" ]] || { echo 'usage: test-lossless-text-assembly.sh <new-output-directory>' >&2; exit 2; }
out="$1"
[[ ! -e "$out" ]] || { echo "output already exists: $out" >&2; exit 2; }
: "${NEW_JAVA2RUST_BIN:?set NEW_JAVA2RUST_BIN to the producer under test}"
fixture="$repo_root/tests/corpus/numbers/text-assembly"
mkdir -p "$out/java"
javac -encoding UTF-8 -d "$out/java" "$fixture/TextAssemblyClient.java" "$fixture/TextAssemblyValues.java"
java -cp "$out/java" TextAssemblyClient > "$out/java-observations.txt"
[[ "$(< "$out/java-observations.txt")" == 'contract=0' ]]
bash "$repo_root/scripts/translate-focused-package.sh" --primary "$fixture" --out "$out" \
  --skip-resolver-bench --offline-cargo --jobs 2 > "$out/runner.log" 2>&1
# Add only an independent test caller. Generated production modules/root are
# never repaired: the preceding runner must already have compiled them.
mkdir -p "$out/generated-crate/tests"
cp "$fixture/contract.rs" "$out/generated-crate/tests/text-contract.rs"
cargo test --offline --manifest-path "$out/generated-crate/Cargo.toml" \
  --test text-contract > "$out/rust-observations.txt" 2>&1
mkdir -p "$out/generated-crate/examples"
cp "$fixture/stdout.rs" "$out/generated-crate/examples/text-output.rs"
cargo run --quiet --offline --manifest-path "$out/generated-crate/Cargo.toml" \
  --example text-output > "$out/rust-stdout.txt" 2> "$out/rust-stdout-stderr.txt"
cmp "$out/java-observations.txt" "$out/rust-stdout.txt"
