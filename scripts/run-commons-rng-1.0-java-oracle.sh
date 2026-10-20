#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${RNG_SOURCE_ROOT:-$repo_root/.tmp/commons-rng-1.0-src}"
out_dir="${RNG_ORACLE_OUT:-$repo_root/.tmp/commons-rng-1.0-java-oracle}"
bash "$repo_root/scripts/prepare-commons-rng-1.0.sh"
sources=()
while IFS= read -r relative; do
  if [[ "$relative" == org/apache/commons/rng/core/* ]]; then
    sources+=("$checkout/commons-rng-core/src/main/java/$relative")
  else
    sources+=("$checkout/commons-rng-client-api/src/main/java/$relative")
  fi
done < "$repo_root/scripts/focus-sets/commons-rng-1.0-source64.txt"
mkdir -p "$out_dir/classes"
java -version > "$out_dir/java-version.log" 2>&1
javac -d "$out_dir/classes" "${sources[@]}" \
  "$repo_root/tests/corpus/rng/GeneratorOracle.java" > "$out_dir/javac.log" 2>&1
java -cp "$out_dir/classes" GeneratorOracle > "$out_dir/output.txt" 2> "$out_dir/stderr.log"
if [[ "$(tail -n 1 "$out_dir/output.txt")" != "oracle=ok" ]]; then
  echo "Oracle did not report completion: $out_dir/output.txt" >&2
  exit 1
fi
printf 'Java reference oracle passes: %s\n' "$out_dir/output.txt"
