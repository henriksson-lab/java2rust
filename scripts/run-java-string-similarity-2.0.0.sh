#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${STRING_SIMILARITY_SOURCE_ROOT:-$repo_root/.tmp/java-string-similarity-v2.0.0-src}"
out_dir="${STRING_SIMILARITY_TRANSLATION_ROOT:-$repo_root/.tmp/java-string-similarity-2.0.0-baseline}"
expected_commit="1372d0b4549f79b9a3d80f5c678cf8e3a787a856"

if [[ ! -d "$checkout/.git" ]]; then
  echo "Missing checkout; run scripts/prepare-java-string-similarity-2.0.0.sh" >&2
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
java_root="$checkout/src/main/java"
source_count="$(find "$java_root" -type f -name '*.java' | wc -l | tr -d ' ')"
if [[ "$source_count" != "27" ]]; then
  echo "Expected all 27 production Java files; found $source_count" >&2
  exit 1
fi

# No cherry-picked subset: includes examples, experimental Sift4, callback
# interfaces and the method-local Offset class. The annotation dependency is
# compile-time metadata, not a replacement implementation of source methods.
"$repo_root/scripts/translate-focused-package.sh" \
  --primary "$java_root" --out "$out_dir" --jobs "${JOBS:-10}" \
  --offline-cargo "$@"
