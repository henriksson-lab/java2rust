#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
module="${STATISTICS_MODULE:-ranking}"
out_dir="${STATISTICS_TRANSLATION_ROOT:-$repo_root/.tmp/commons-statistics-1.3-$module-baseline}"
staging="${STATISTICS_STAGING_ROOT:-$out_dir-staged}"
bash "$repo_root/scripts/prepare-commons-statistics-1.3.sh"
mkdir -p "$(dirname "$staging")" "$(dirname "$out_dir")"
python3 "$repo_root/scripts/commons-statistics-1.3-corpus.py" \
  --module "$module" --stage "$staging" --inventory "$out_dir/scope" \
  > "$staging-scope.json"
if [[ -n "${STATISTICS_EXTRA_SOURCE:-}" ]]; then
  # Caller-side probes are additional inputs, never modifications to the
  # complete pinned production closure recorded by the inventory above.
  [[ -f "$STATISTICS_EXTRA_SOURCE" && "$STATISTICS_EXTRA_SOURCE" == *.java ]] || {
    echo 'STATISTICS_EXTRA_SOURCE must name a Java source file' >&2; exit 1;
  }
  mkdir -p "$staging/acceptance"
  extra_target="$staging/acceptance/$(basename "$STATISTICS_EXTRA_SOURCE")"
  cp "$STATISTICS_EXTRA_SOURCE" "$extra_target"
  cmp "$STATISTICS_EXTRA_SOURCE" "$extra_target"
fi
exec bash "$repo_root/scripts/translate-focused-package.sh" \
  --primary "$staging" --out "$out_dir" --jobs "${JOBS:-2}" --offline-cargo \
  --external-pack-manifest "$repo_root/runtime/java-math/manifests/big-integer.facts" "$@"
