#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${NUMBERS_SOURCE_ROOT:-$repo_root/.tmp/commons-numbers-1.3-src}"
expected_commit="ae8abefb51c2c4d1a05882d61cefc8015e493a8f"
if [[ ! -e "$checkout" ]]; then
  git clone --depth 1 --branch rel/commons-numbers-1.3 https://github.com/apache/commons-numbers.git "$checkout"
fi
if [[ ! -d "$checkout/.git" ]] ||
   [[ "$(git -C "$checkout" rev-parse HEAD)" != "$expected_commit" ]] ||
   [[ -n "$(git -C "$checkout" status --porcelain)" ]]; then
  echo "Commons Numbers requires untouched commit $expected_commit: $checkout" >&2
  exit 1
fi
printf 'Commons Numbers 1.3 validated at %s\ncommit\t%s\n' "$checkout" "$expected_commit"
