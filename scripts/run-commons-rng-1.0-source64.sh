#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$repo_root/scripts/commons-rng-1.0-paths.sh"
checkout="${RNG_SOURCE_ROOT:-$repo_root/.tmp/commons-rng-1.0-src}"
staging="${RNG_STAGING_ROOT:-$repo_root/.tmp/commons-rng-1.0-source64-src}"
out_dir="${RNG_TRANSLATION_ROOT:-$repo_root/.tmp/commons-rng-1.0-source64-baseline}"
focus_set="${RNG_FOCUS_SET:-$repo_root/scripts/focus-sets/commons-rng-1.0-source64.txt}"
expected_count="${RNG_EXPECTED_SOURCE_COUNT:-11}"
if [[ "$(wc -l < "$focus_set" | tr -d ' ')" != "$expected_count" ]]; then
  echo "Expected $expected_count manifest entries: $focus_set" >&2
  exit 1
fi
bash "$repo_root/scripts/prepare-commons-rng-1.0.sh"
while IFS= read -r relative; do
  original="$(commons_rng_source_for_relative "$checkout" "$relative")"
  mkdir -p "$staging/$(dirname "$relative")"
  cp "$original" "$staging/$relative"
  cmp "$original" "$staging/$relative"
done < "$focus_set"
source_count="$(find "$staging" -type f -name '*.java' | wc -l | tr -d ' ')"
if [[ "$source_count" != "$expected_count" ]]; then
  echo "Expected the full $expected_count-file generator dependency closure; found $source_count" >&2
  exit 1
fi
bash "${FOCUSED_PACKAGE_RUNNER:-$repo_root/scripts/translate-focused-package.sh}" \
  --primary "$staging" --out "$out_dir" --jobs "${JOBS:-7}" --offline-cargo "$@"
