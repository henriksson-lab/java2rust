#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${RNG_SOURCE_ROOT:-$repo_root/.tmp/commons-rng-1.0-src}"
staging="${RNG_STAGING_ROOT:-$repo_root/.tmp/commons-rng-1.0-source64-src}"
out_dir="${RNG_TRANSLATION_ROOT:-$repo_root/.tmp/commons-rng-1.0-source64-baseline}"
bash "$repo_root/scripts/prepare-commons-rng-1.0.sh"
while IFS= read -r relative; do
  if [[ "$relative" == org/apache/commons/rng/core/* ]]; then
    original="$checkout/commons-rng-core/src/main/java/$relative"
  else
    original="$checkout/commons-rng-client-api/src/main/java/$relative"
  fi
  mkdir -p "$staging/$(dirname "$relative")"
  cp "$original" "$staging/$relative"
  cmp "$original" "$staging/$relative"
done < "$repo_root/scripts/focus-sets/commons-rng-1.0-source64.txt"
source_count="$(find "$staging" -type f -name '*.java' | wc -l | tr -d ' ')"
if [[ "$source_count" != "11" ]]; then
  echo "Expected the full 11-file generator dependency closure; found $source_count" >&2
  exit 1
fi
"$repo_root/scripts/translate-focused-package.sh" \
  --primary "$staging" --out "$out_dir" --jobs "${JOBS:-7}" --offline-cargo "$@"
