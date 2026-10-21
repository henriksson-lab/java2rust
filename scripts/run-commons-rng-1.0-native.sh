#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export RNG_FOCUS_SET="$repo_root/scripts/focus-sets/commons-rng-1.0-native.txt"
export RNG_EXPECTED_SOURCE_COUNT=24
export RNG_STAGING_ROOT="${RNG_STAGING_ROOT:-$repo_root/.tmp/commons-rng-1.0-native-src}"
export RNG_TRANSLATION_ROOT="${RNG_TRANSLATION_ROOT:-$repo_root/.tmp/commons-rng-1.0-native-baseline}"
exec bash "$repo_root/scripts/run-commons-rng-1.0-source64.sh" "$@"
