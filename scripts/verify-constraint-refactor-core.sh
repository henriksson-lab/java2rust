#!/usr/bin/env bash
# Full pinned Commons Numbers core module + existing translated contract client.
# This is a regression gate, not a benchmark or a generated-code repair script.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out_root="${CONSTRAINT_REFACTOR_OUT:-$repo_root/.tmp/constraint-refactor-core}"
mkdir -p "$out_root"
out="$(mktemp -d "$out_root/run.XXXXXX")"
printf '%s\n' "$out" > "$out_root/latest-run.txt"
trap 'echo "artifacts: $out" >&2' EXIT
NUMBERS_CONTRACTS_STAGING_ROOT="$out/staged" \
NUMBERS_CONTRACTS_TRANSLATION_ROOT="$out/translation" \
  bash "$repo_root/scripts/run-commons-numbers-1.3-core-contracts.sh" \
    --skip-resolver-bench > "$out/translation-driver.log" 2>&1
summary="$out/translation/summary.tsv"
require_metric() {
  local actual
  actual="$(awk -F '\t' -v key="$1" '$1 == key { print $2 }' "$summary")"
  [[ "$actual" == "$2" ]] || { echo "$1: expected $2, got $actual" >&2; exit 1; }
}
require_metric files 12
require_metric translate_failed_files 0
require_metric cargo_check ok
require_metric cargo_check_errors 0
require_metric invalid_pattern_files 0
NUMBERS_ORACLE_OUT="$out/java" \
  bash "$repo_root/scripts/run-commons-numbers-1.3-core-contracts-java-oracle.sh" \
    > "$out/java-driver.log" 2>&1
crate="$out/translation/generated-crate"
mkdir -p "$crate/examples"
# An external launcher exercises the generated Java main unchanged. This adds
# no implementation, conversion, or recovery code to the translated program.
cat > "$crate/examples/constraint_refactor_core.rs" <<'RUST'
fn main() {
    new_java2rust_focused_output::CoreContractsOracle::main(&[]).unwrap();
}
RUST
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo_root/.tmp/focused-cargo-target}" \
  cargo run --quiet --offline --manifest-path "$crate/Cargo.toml" \
    --example constraint_refactor_core > "$out/rust.out" 2> "$out/rust.stderr.log"
diff -u "$out/java/output.txt" "$out/rust.out"
echo "Commons Numbers core: all 11 production files + contract client compile and match Java"
echo "artifacts: $out"
