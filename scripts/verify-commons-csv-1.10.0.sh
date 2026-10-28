#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_root="${1:-$repo_root/.tmp/commons-csv-1.10.0-src}"
java_root="$source_root/src/main/java"
expected_commit="5b99c860986a87f9da664bf47a3a7c0026673123"
fixture_root="$repo_root/tests/real_world/commons_csv_1_10_0"

if [[ ! -d "$source_root/.git" ]]; then
  echo "Apache Commons CSV checkout is missing or is not a Git checkout: $source_root" >&2
  exit 1
fi
actual_commit="$(git -C "$source_root" rev-parse HEAD)"
if [[ "$actual_commit" != "$expected_commit" ]]; then
  echo "Apache Commons CSV checkout must be pinned to $expected_commit; found $actual_commit" >&2
  exit 1
fi

mapfile -t java_sources < <(find "$java_root" -type f -name '*.java' | sort)
if [[ "${#java_sources[@]}" -ne 12 ]]; then
  echo "expected 12 Apache Commons CSV production files; found ${#java_sources[@]}" >&2
  exit 1
fi

gate_root="$(mktemp -d "${TMPDIR:-/tmp}/new-java2rust-commons-csv.XXXXXX")"
cleanup() {
  if [[ "${KEEP_COMMONS_CSV_GATE:-0}" == "1" ]]; then
    echo "kept Commons CSV gate artifacts at $gate_root" >&2
  else
    rm -rf "$gate_root"
  fi
}
trap cleanup EXIT

translation_root="${COMMONS_CSV_TRANSLATION_ROOT:-$gate_root/translation}"
if [[ -z "${COMMONS_CSV_TRANSLATION_ROOT:-}" ]]; then
  "$repo_root/scripts/translate-focused-package.sh" \
    --skip-resolver-bench \
    --offline-cargo \
    --jobs "${JOBS:-10}" \
    --primary "$java_root" \
    --out "$translation_root"
fi

metric() {
  awk -F '\t' -v key="$1" '$1 == key { print $2; found = 1; exit } END { if (!found) exit 1 }' \
    "$translation_root/summary.tsv"
}
assert_metric() {
  local key="$1"
  local expected="$2"
  local actual
  actual="$(metric "$key")"
  if [[ "$actual" != "$expected" ]]; then
    echo "unexpected $key: expected $expected, found $actual" >&2
    exit 1
  fi
}

assert_metric files 12
assert_metric translate_failed_files 0
assert_metric parse_failures 0
assert_metric unsupported_statements 0
assert_metric unsupported_members 0
assert_metric unsupported_expressions 0
assert_metric stubbed_constants 0
assert_metric invalid_pattern_files 0
assert_metric cargo_check ok

generated_crate="$translation_root/generated-crate"
mkdir -p "$generated_crate/examples" "$gate_root/java-classes"
cp "$fixture_root/commons_csv_oracle.rs" "$generated_crate/examples/commons_csv_oracle.rs"

javac -d "$gate_root/java-classes" "${java_sources[@]}" \
  "$fixture_root/CommonsCsvReaderOracle.java" "$fixture_root/CommonsCsvOracle.java"
java -cp "$gate_root/java-classes" CommonsCsvOracle > "$gate_root/java.out"

CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo_root/.tmp/focused-cargo-target}" \
  cargo run --quiet --offline --manifest-path "$generated_crate/Cargo.toml" \
    --example commons_csv_oracle > "$gate_root/rust.out"

diff -u "$gate_root/java.out" "$gate_root/rust.out"
echo "Apache Commons CSV 1.10.0 translation and differential runtime gate passed"
