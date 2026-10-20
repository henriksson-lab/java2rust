#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
archive="${JAMA_ARCHIVE:-$repo_root/.tmp/Jama-1.0.3.tar.gz}"
checkout="${JAMA_SOURCE_ROOT:-$repo_root/.tmp/jama-1.0.3-src}"
expected_sha="a8f4c8f3a4a5d5f8908304638d03aa181b7c71debb2e3c08bb9dc14fcc350a75"

if [[ ! -f "$archive" ]]; then
  mkdir -p "$(dirname "$archive")"
  curl -fL --retry 2 https://math.nist.gov/javanumerics/jama/Jama-1.0.3.tar.gz -o "$archive"
fi
printf '%s  %s\n' "$expected_sha" "$archive" | sha256sum --check --status
if [[ ! -e "$checkout" ]]; then
  mkdir -p "$checkout"
  tar --no-same-owner -xzf "$archive" -C "$checkout"
fi
while IFS= read -r relative; do
  cmp "$checkout/$relative" <(tar -xOzf "$archive" "$relative")
done < "$repo_root/scripts/focus-sets/jama-1.0.3-production.txt"
printf 'JAMA 1.0.3 source validated at %s\narchive_sha256\t%s\n' "$checkout" "$expected_sha"
