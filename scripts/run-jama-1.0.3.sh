#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${JAMA_SOURCE_ROOT:-$repo_root/.tmp/jama-1.0.3-src}"
staging="${JAMA_STAGING_ROOT:-$repo_root/.tmp/jama-1.0.3-production-src}"
out_dir="${JAMA_TRANSLATION_ROOT:-$repo_root/.tmp/jama-1.0.3-baseline}"

bash "$repo_root/scripts/prepare-jama-1.0.3.sh"
while IFS= read -r relative; do
  mkdir -p "$staging/$(dirname "$relative")"
  cp "$checkout/$relative" "$staging/$relative"
  cmp "$checkout/$relative" "$staging/$relative"
done < "$repo_root/scripts/focus-sets/jama-1.0.3-production.txt"
source_count="$(find "$staging" -type f -name '*.java' | wc -l | tr -d ' ')"
if [[ "$source_count" != "7" ]]; then
  echo "Expected all seven JAMA production sources; found $source_count" >&2
  exit 1
fi

"$repo_root/scripts/translate-focused-package.sh" \
  --primary "$staging" --out "$out_dir" --jobs "${JOBS:-7}" \
  --offline-cargo "$@"
