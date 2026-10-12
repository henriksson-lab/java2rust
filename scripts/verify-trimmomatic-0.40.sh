#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_root="${1:-/home/mahogny/github/claude/java2rust/testdata/trimmomatic}"
java_root="$source_root/src/main/java"
expected_commit="3deaa141b2825daa28a085f00c309d10b8b4d0e1"
fixture_root="$repo_root/tests/real_world/trimmomatic_0_40"

if [[ ! -d "$source_root/.git" ]]; then
  echo "Trimmomatic checkout is missing or is not a Git checkout: $source_root" >&2
  exit 1
fi
actual_commit="$(git -C "$source_root" rev-parse HEAD)"
if [[ "$actual_commit" != "$expected_commit" ]]; then
  echo "Trimmomatic checkout must be pinned to $expected_commit; found $actual_commit" >&2
  exit 1
fi

mapfile -t java_sources < <(find "$java_root" -type f -name '*.java' | sort)
if [[ "${#java_sources[@]}" -ne 65 ]]; then
  echo "expected 65 Trimmomatic production files; found ${#java_sources[@]}" >&2
  exit 1
fi

gate_root="$(mktemp -d "${TMPDIR:-/tmp}/new-java2rust-trimmomatic.XXXXXX")"
cleanup() {
  if [[ "${KEEP_TRIMMOMATIC_GATE:-0}" == "1" ]]; then
    echo "kept Trimmomatic gate artifacts at $gate_root" >&2
  else
    rm -rf "$gate_root"
  fi
}
trap cleanup EXIT

translation_root="${TRIMMOMATIC_TRANSLATION_ROOT:-$gate_root/translation}"
if [[ -z "${TRIMMOMATIC_TRANSLATION_ROOT:-}" ]]; then
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

assert_metric files 65
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
cp "$fixture_root/trimmomatic_oracle.rs" "$generated_crate/examples/trimmomatic_oracle.rs"

cat > "$gate_root/input.fastq" <<'EOF'
@keep
ACGTAC
+one
!!II!!
@drop
AC
+
!!
@long
AACCGG
+meta
IIIIII
EOF
cat > "$gate_root/malformed.fastq" <<'EOF'
not-a-fastq-name
AC
+
II
EOF

oracle_java_sources=(
  "$java_root/org/usadellab/trimmomatic/fastq/FastqRecord.java"
  "$java_root/org/usadellab/trimmomatic/fastq/FastqParser.java"
  "$java_root/org/usadellab/trimmomatic/fastq/FastqSerializer.java"
  "$java_root/org/usadellab/trimmomatic/util/PositionTrackingInputStream.java"
  "$java_root/org/usadellab/trimmomatic/trim/Trimmer.java"
  "$java_root/org/usadellab/trimmomatic/trim/AbstractSingleRecordTrimmer.java"
  "$java_root/org/usadellab/trimmomatic/trim/CropTrimmer.java"
  "$java_root/org/usadellab/trimmomatic/trim/LeadingTrimmer.java"
  "$java_root/org/usadellab/trimmomatic/trim/MinLenTrimmer.java"
  "$java_root/org/usadellab/trimmomatic/trim/SlidingWindowTrimmer.java"
  "$java_root/org/usadellab/trimmomatic/trim/TrailingTrimmer.java"
  "$fixture_root/java_stub/org/usadellab/trimmomatic/util/compression/CompressionFormat.java"
)
javac -d "$gate_root/java-classes" "${oracle_java_sources[@]}" "$fixture_root/TrimmomaticOracle.java"
java -cp "$gate_root/java-classes" TrimmomaticOracle \
  "$gate_root/input.fastq" "$gate_root/java-roundtrip.fastq" "$gate_root/malformed.fastq" \
  > "$gate_root/java.out"

CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo_root/.tmp/focused-cargo-target}" \
  cargo run --quiet --offline --manifest-path "$generated_crate/Cargo.toml" \
    --example trimmomatic_oracle -- \
    "$gate_root/input.fastq" "$gate_root/rust-roundtrip.fastq" "$gate_root/malformed.fastq" \
    > "$gate_root/rust.out"

diff -u "$gate_root/java.out" "$gate_root/rust.out"
diff -u "$gate_root/java-roundtrip.fastq" "$gate_root/rust-roundtrip.fastq"
echo "Trimmomatic 0.40 translation and differential runtime gate passed"
