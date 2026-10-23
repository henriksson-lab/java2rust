#!/usr/bin/env bash
# Complete pinned production module plus one explicitly separate translated client.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
module="${NUMBERS_MODULE:-core}"
case "$module" in
  core) expected=11; client=CoreContractsOracle; modules=(core) ;;
  complex) expected=2; client=ComplexContractsOracle; modules=(complex) ;;
  quaternion) expected=14; client=QuaternionContractsOracle; modules=(core quaternion) ;;
  angle) expected=15; client=AngleContractsOracle; modules=(core angle) ;;
  *) echo 'Contract module must be core, complex, quaternion or angle' >&2; exit 1 ;;
esac
checkout="${NUMBERS_SOURCE_ROOT:-$repo_root/.tmp/commons-numbers-1.3-src}"
staging="${NUMBERS_CONTRACTS_STAGING_ROOT:-$repo_root/.tmp/commons-numbers-1.3-$module-contracts-staged}"
out_dir="${NUMBERS_CONTRACTS_TRANSLATION_ROOT:-$repo_root/.tmp/commons-numbers-1.3-$module-contracts-baseline}"
bash "$repo_root/scripts/prepare-commons-numbers-1.3.sh"
manifest="$repo_root/scripts/focus-sets/commons-numbers-1.3-all.txt"
count=0
production_sources=()
for selected in "${modules[@]}"; do
    prefix="commons-numbers-$selected/src/main/java/"
    diff -u <(find "$checkout/$prefix" -type f -name '*.java' -printf '%P\n' | sort) \
      <(sed -n "s|^$prefix||p" "$manifest" | sort)
    while IFS= read -r relative; do
        [[ "$relative" == "$prefix"* ]] || continue
        mkdir -p "$staging/$(dirname "$relative")"
        cp "$checkout/$relative" "$staging/$relative"
        cmp "$checkout/$relative" "$staging/$relative"
        production_sources+=("$relative")
        count=$((count + 1))
    done < "$manifest"
done
[[ "$count" == "$expected" ]] || { echo "Expected all $expected production $module files" >&2; exit 1; }
caller="caller/org/apache/commons/numbers/$module/$client.java"
mkdir -p "$staging/$(dirname "$caller")"
cp "$repo_root/tests/corpus/numbers/$client.java" "$staging/$caller"
cmp "$repo_root/tests/corpus/numbers/$client.java" "$staging/$caller"
[[ "$(find "$staging" -type f -name '*.java' | wc -l | tr -d ' ')" == "$((expected + 1))" ]] || {
    echo "Expected exactly $expected production files and one client-only contract fixture" >&2; exit 1;
}
mkdir -p "$out_dir"
{
    printf 'production\t%s\n' "${production_sources[@]}"
    printf 'test-caller\t%s\n' "$caller"
} > "$out_dir/contract-source-manifest.tsv"
exec bash "${FOCUSED_PACKAGE_RUNNER:-$repo_root/scripts/translate-focused-package.sh}" \
    --primary "$staging" --out "$out_dir" --jobs "${JOBS:-4}" --offline-cargo \
    --external-pack-manifest "$repo_root/runtime/java-math/manifests/big-integer.facts" "$@"
