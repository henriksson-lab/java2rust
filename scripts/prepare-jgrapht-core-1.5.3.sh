#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${1:-$repo_root/.tmp/jgrapht-1.5.3-src}"
expected_commit="be2abc30b9188e4099d737179e11bd669cf1e041"
tag="jgrapht-1.5.3"

if [[ ! -e "$checkout" ]]; then
  git clone --depth 1 --branch "$tag" https://github.com/jgrapht/jgrapht.git "$checkout"
fi
if [[ ! -d "$checkout/.git" ]]; then
  echo "JGraphT checkout is not a Git checkout: $checkout" >&2
  exit 1
fi

actual_commit="$(git -C "$checkout" rev-parse HEAD)"
if [[ "$actual_commit" != "$expected_commit" ]]; then
  echo "JGraphT checkout must be pinned to $expected_commit; found $actual_commit" >&2
  exit 1
fi
if [[ -n "$(git -C "$checkout" status --short --untracked-files=no)" ]]; then
  echo "JGraphT tracked sources must remain untouched: $checkout" >&2
  exit 1
fi

maven_cache="${JGRAPHT_MAVEN_CACHE:-$repo_root/.tmp/jgrapht-m2}"
mvn -q -Dmaven.repo.local="$maven_cache" -f "$checkout/pom.xml" \
  -pl jgrapht-core -am package -DskipTests

printf 'JGraphT %s prepared at %s\n' "$tag" "$checkout"
printf 'commit\t%s\n' "$actual_commit"

