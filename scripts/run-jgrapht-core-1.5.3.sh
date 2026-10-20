#!/usr/bin/env bash
set -euo pipefail

skip_resolver_bench=0
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --skip-resolver-bench) skip_resolver_bench=1 ;;
    --help|-h)
      echo "Usage: $0 [--skip-resolver-bench]"
      echo "Skip only the separate resolver benchmark when its prior evidence still applies."
      echo "Pinned-source validation and production translation/assembly/checks still run."
      exit 0
      ;;
    *) echo "Unknown argument: $1 (use --help)" >&2; exit 2 ;;
  esac
  shift
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${JGRAPHT_SOURCE_ROOT:-$repo_root/.tmp/jgrapht-1.5.3-src}"
java_root="$checkout/jgrapht-core/src/main/java"
focus_list="$repo_root/scripts/focus-sets/jgrapht-core-graph-shortestpath.txt"
out_dir="${JGRAPHT_TRANSLATION_ROOT:-$repo_root/.tmp/jgrapht-core-1.5.3-baseline}"
focused_source_root="${JGRAPHT_FOCUSED_SOURCE_ROOT:-$repo_root/.tmp/jgrapht-core-1.5.3-focus-src}"
expected_commit="be2abc30b9188e4099d737179e11bd669cf1e041"

if [[ ! -d "$checkout/.git" ]]; then
  echo "JGraphT checkout is missing; run scripts/prepare-jgrapht-core-1.5.3.sh first" >&2
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

source_count="$(find "$java_root/org/jgrapht" -type f -name '*.java' | wc -l | tr -d ' ')"
focus_count="$(grep -cvE '^[[:space:]]*(#|$)' "$focus_list")"
if [[ "$source_count" != "463" ]]; then
  echo "expected 463 jgrapht-core production sources; found $source_count" >&2
  exit 1
fi
if [[ "$focus_count" != "104" ]]; then
  echo "expected 104 focused production sources; found $focus_count" >&2
  exit 1
fi

translate_args=(
  --out "$out_dir"
  --jobs "${JOBS:-10}"
  --offline-cargo
)
if [[ "$skip_resolver_bench" -eq 1 ]]; then
  translate_args+=(--skip-resolver-bench)
fi
if [[ "${JGRAPHT_BROAD_CONTEXT:-0}" == "1" ]]; then
  # This is the expensive broad gate. All 463 sources participate in semantic
  # and signature analysis while only the dependency closure is emitted.
  translate_args+=(--primary "$java_root" --focus-list "$focus_list")
else
  # Tight iterations analyze only the bytecode-derived source closure. Copying
  # preserves package-relative paths and leaves the pinned checkout untouched.
  mkdir -p "$focused_source_root"
  while IFS= read -r relative; do
    [[ -n "$relative" && "$relative" != \#* ]] || continue
    mkdir -p "$focused_source_root/$(dirname "$relative")"
    cp "$java_root/$relative" "$focused_source_root/$relative"
    cmp "$java_root/$relative" "$focused_source_root/$relative"
  done < "$focus_list"
  staged_count="$(find "$focused_source_root" -type f -name '*.java' | wc -l | tr -d ' ')"
  if [[ "$staged_count" != "$focus_count" ]]; then
    echo "focused source tree contains $staged_count files; expected $focus_count" >&2
    exit 1
  fi
  translate_args+=(--primary "$focused_source_root")
fi

"$repo_root/scripts/translate-focused-package.sh" "${translate_args[@]}"

printf 'JGraphT focused translation written to %s\n' "$out_dir"

