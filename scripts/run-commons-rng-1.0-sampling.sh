#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
stage="${RNG_SAMPLING_STAGE:-all}"
case "$stage" in
  collections) count=27 ;;
  all) count=46 ;;
  *) echo 'RNG_SAMPLING_STAGE must be collections or all' >&2; exit 1 ;;
esac
export RNG_FOCUS_SET="$repo_root/scripts/focus-sets/commons-rng-1.0-sampling-$stage.txt"
export RNG_EXPECTED_SOURCE_COUNT="$count"
export RNG_STAGING_ROOT="${RNG_STAGING_ROOT:-$repo_root/.tmp/commons-rng-1.0-sampling-$stage-src}"
export RNG_TRANSLATION_ROOT="${RNG_TRANSLATION_ROOT:-$repo_root/.tmp/commons-rng-1.0-sampling-$stage-baseline}"
exec bash "$repo_root/scripts/run-commons-rng-1.0-source64.sh" "$@"
