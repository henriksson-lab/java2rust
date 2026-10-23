#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${NUMBERS_SOURCE_ROOT:-$repo_root/.tmp/commons-numbers-1.3-src}"
module="${NUMBERS_MODULE:-primes}"
case "$module" in
  primes|rootfinder|core|complex) modules=("$module") ;;
  quaternion|angle) modules=(core "$module") ;;
  *) echo 'NUMBERS_MODULE must be primes, rootfinder, core, complex, quaternion or angle' >&2; exit 1 ;;
esac
bash "$repo_root/scripts/prepare-commons-numbers-1.3.sh"
manifest="$repo_root/scripts/focus-sets/commons-numbers-1.3-all.txt"
staging="${NUMBERS_STAGING_ROOT:-$repo_root/.tmp/commons-numbers-1.3-$module-staged}"
out_dir="${NUMBERS_TRANSLATION_ROOT:-$repo_root/.tmp/commons-numbers-1.3-$module-baseline}"
expected_count=0
for selected in "${modules[@]}"; do
  prefix="commons-numbers-$selected/src/main/java/"
  diff -u <(find "$checkout/$prefix" -type f -name '*.java' -printf '%P\n' | sort) \
    <(sed -n "s|^$prefix||p" "$manifest" | sort)
  while IFS= read -r relative; do
    [[ "$relative" == "$prefix"* ]] || continue
    mkdir -p "$staging/$(dirname "$relative")"
    cp "$checkout/$relative" "$staging/$relative"
    cmp "$checkout/$relative" "$staging/$relative"
    expected_count=$((expected_count + 1))
  done < "$manifest"
done
source_count="$(find "$staging" -type f -name '*.java' | wc -l | tr -d ' ')"
if [[ "$source_count" != "$expected_count" ]]; then
  echo "Expected $expected_count source files for $module including dependencies; found $source_count" >&2
  exit 1
fi
exec bash "${FOCUSED_PACKAGE_RUNNER:-$repo_root/scripts/translate-focused-package.sh}" \
  --primary "$staging" --out "$out_dir" --jobs "${JOBS:-4}" --offline-cargo \
  --external-pack-manifest "$repo_root/runtime/java-math/manifests/big-integer.facts" "$@"
