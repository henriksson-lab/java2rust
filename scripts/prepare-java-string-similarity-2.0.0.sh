#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${1:-$repo_root/.tmp/java-string-similarity-v2.0.0-src}"
expected_commit="1372d0b4549f79b9a3d80f5c678cf8e3a787a856"

if [[ ! -e "$checkout" ]]; then
  git clone --depth 1 --branch v2.0.0 \
    https://github.com/tdebatty/java-string-similarity.git "$checkout"
fi
if [[ ! -d "$checkout/.git" ]]; then
  echo "Not a Git checkout: $checkout" >&2
  exit 1
fi
actual_commit="$(git -C "$checkout" rev-parse HEAD)"
if [[ "$actual_commit" != "$expected_commit" ]]; then
  echo "Expected $expected_commit; found $actual_commit" >&2
  exit 1
fi
if [[ -n "$(git -C "$checkout" status --porcelain)" ]]; then
  echo "Pinned checkout must remain untouched: $checkout" >&2
  exit 1
fi
printf 'java-string-similarity v2.0.0 prepared at %s\ncommit\t%s\n' \
  "$checkout" "$actual_commit"
