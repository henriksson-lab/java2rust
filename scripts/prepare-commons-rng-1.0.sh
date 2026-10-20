#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${RNG_SOURCE_ROOT:-$repo_root/.tmp/commons-rng-1.0-src}"
expected_commit="4581a4520315657a4219b37c81f5db80e4a4e43c"
if [[ ! -e "$checkout" ]]; then
  git clone --depth 1 --branch RNG_1_0 https://github.com/apache/commons-rng.git "$checkout"
fi
if [[ ! -d "$checkout/.git" ]] ||
   [[ "$(git -C "$checkout" rev-parse HEAD)" != "$expected_commit" ]] ||
   [[ -n "$(git -C "$checkout" status --porcelain)" ]]; then
  echo "Commons RNG requires untouched commit $expected_commit: $checkout" >&2
  exit 1
fi
printf 'Commons RNG 1.0 validated at %s\ncommit\t%s\n' "$checkout" "$expected_commit"
