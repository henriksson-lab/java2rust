#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
validate_pin() {
  local checkout="$1" remote="$2" tag="$3" expected="$4"
  if [[ ! -e "$checkout" ]]; then
    git clone --depth 1 --branch "$tag" "$remote" "$checkout"
  fi
  if [[ ! -d "$checkout/.git" ]] ||
     [[ "$(git -C "$checkout" rev-parse HEAD)" != "$expected" ]] ||
     [[ -n "$(git -C "$checkout" status --porcelain)" ]]; then
    echo "Requires untouched commit $expected: $checkout" >&2
    exit 1
  fi
  printf 'validated\t%s\t%s\n' "$checkout" "$expected"
}
validate_pin "${STATISTICS_SOURCE_ROOT:-$repo_root/.tmp/commons-statistics-1.3-src}" \
  https://github.com/apache/commons-statistics.git rel/commons-statistics-1.3 \
  2937eb2e711483d8ea9dc216af45c16fd0066b77
validate_pin "${STATISTICS_RNG_SOURCE_ROOT:-$repo_root/.tmp/commons-rng-1.6-src}" \
  https://github.com/apache/commons-rng.git rel/commons-rng-1.6 \
  618490cf3f072ddc327de90109fac04f404831cb
bash "$repo_root/scripts/prepare-commons-numbers-1.3.sh"
