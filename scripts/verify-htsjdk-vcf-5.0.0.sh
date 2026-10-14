#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_root="${1:-/home/mahogny/github/claude/java2rust/testdata/htsjdk_vcf}"
fixture_root="$repo_root/tests/real_world/htsjdk_vcf"

mapfile -t java_sources < <(find "$source_root" -maxdepth 1 -type f -name '*.java' | sort)
if [[ "${#java_sources[@]}" -ne 32 ]]; then
  echo "expected 32 untouched HTSJDK VCF production files; found ${#java_sources[@]}" >&2
  exit 1
fi

gate_root="$(mktemp -d "${TMPDIR:-/tmp}/new-java2rust-htsjdk-vcf.XXXXXX")"
cleanup() {
  local status=$?
  if [[ "${KEEP_HTSJDK_VCF_GATE:-0}" == "1" ]]; then
    echo "kept HTSJDK VCF gate artifacts at $gate_root" >&2
  else
    rm -rf "$gate_root"
  fi
  return "$status"
}
trap cleanup EXIT

translation_root="${HTSJDK_VCF_TRANSLATION_ROOT:-$gate_root/translation}"
if [[ -z "${HTSJDK_VCF_TRANSLATION_ROOT:-}" ]]; then
  "$repo_root/scripts/translate-focused-package.sh" \
    --skip-resolver-bench \
    --offline-cargo \
    --jobs "${JOBS:-10}" \
    --primary "$source_root" \
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

assert_metric files 32
assert_metric translate_failed_files 0
assert_metric parse_failures 0
assert_metric unsupported_statements 0
assert_metric unsupported_members 0
assert_metric unsupported_expressions 0
assert_metric invalid_pattern_files 0
assert_metric cargo_check ok

generated_crate="$translation_root/generated-crate"
mkdir -p "$generated_crate/examples" "$gate_root/java-classes"
cp "$fixture_root/htsjdk_vcf_oracle.rs" "$generated_crate/examples/htsjdk_vcf_oracle.rs"

javac -d "$gate_root/java-classes" \
  "$fixture_root/java_stub/htsjdk/samtools/util/RuntimeIOException.java" \
  "$fixture_root/java_stub/htsjdk/tribble/TribbleException.java" \
  "$fixture_root/java_stub/htsjdk/tribble/util/ParsingUtils.java" \
  "$fixture_root/java_stub/htsjdk/variant/variantcontext/Allele.java" \
  "$fixture_root/java_stub/htsjdk/variant/variantcontext/Genotype.java" \
  "$fixture_root/java_stub/htsjdk/variant/variantcontext/GenotypeBuilder.java" \
  "$fixture_root/java_stub/htsjdk/variant/variantcontext/GenotypesContext.java" \
  "$fixture_root/java_stub/htsjdk/variant/variantcontext/LazyGenotypesContext.java" \
  "$fixture_root/java_stub/htsjdk/variant/variantcontext/VariantContext.java" \
  "$fixture_root/java_stub/htsjdk/variant/variantcontext/writer/IntGenotypeFieldAccessors.java" \
  "$fixture_root/java_stub/htsjdk/variant/vcf/VCFHeader.java" \
  "$fixture_root/java_stub/htsjdk/variant/vcf/VCFInfoHeaderLine.java" \
  "$source_root/VCFConstants.java" \
  "$source_root/VCFEncoder.java" \
  "$source_root/VCFHeaderLine.java" \
  "$source_root/VCFIDHeaderLine.java" \
  "$source_root/VCFSimpleHeaderLine.java" \
  "$source_root/VCFHeaderLineTranslator.java" \
  "$source_root/VCFHeaderVersion.java" \
  "$source_root/VCFHeaderLineCount.java" \
  "$source_root/VCFHeaderLineType.java" \
  "$source_root/VCFTextTransformer.java" \
  "$source_root/VCFPassThruTextTransformer.java" \
  "$source_root/VCFPercentEncodedTextTransformer.java" \
  "$fixture_root/HtsjdkVcfOracle.java"
java -cp "$gate_root/java-classes" htsjdk.variant.vcf.HtsjdkVcfOracle > "$gate_root/java.out"

CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo_root/.tmp/focused-cargo-target}" \
  cargo run --quiet --offline --manifest-path "$generated_crate/Cargo.toml" \
    --example htsjdk_vcf_oracle > "$gate_root/rust.out"

diff -u "$gate_root/java.out" "$gate_root/rust.out"
echo "HTSJDK VCF 5.0.0 translation and differential runtime gate passed"
