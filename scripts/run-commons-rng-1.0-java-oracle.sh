#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${RNG_SOURCE_ROOT:-$repo_root/.tmp/commons-rng-1.0-src}"
out_dir="${RNG_ORACLE_OUT:-$repo_root/.tmp/commons-rng-1.0-java-oracle}"
focus_set="${RNG_ORACLE_FOCUS_SET:-$repo_root/scripts/focus-sets/commons-rng-1.0-source64.txt}"
driver="${RNG_ORACLE_DRIVER:-$repo_root/tests/corpus/rng/GeneratorOracle.java}"
main_class="$(basename "$driver" .java)"
bash "$repo_root/scripts/prepare-commons-rng-1.0.sh"
sources=()
while IFS= read -r relative; do
  if [[ "$relative" == org/apache/commons/rng/core/* ]]; then
    sources+=("$checkout/commons-rng-core/src/main/java/$relative")
  else
    sources+=("$checkout/commons-rng-client-api/src/main/java/$relative")
  fi
done < "$focus_set"
if [[ "$driver" != "$repo_root/tests/corpus/rng/GeneratorOracle.java" ]]; then
  sources+=("$repo_root/tests/corpus/rng/GeneratorOracle.java")
fi
mkdir -p "$out_dir/classes"
java -version > "$out_dir/java-version.log" 2>&1
javac -d "$out_dir/classes" "${sources[@]}" \
  "$driver" > "$out_dir/javac.log" 2>&1
java -cp "$out_dir/classes" "$main_class" > "$out_dir/output.txt" 2> "$out_dir/stderr.log"
if [[ "$(tail -n 1 "$out_dir/output.txt")" != "oracle=ok" ]]; then
  echo "Oracle did not report completion: $out_dir/output.txt" >&2
  exit 1
fi
printf 'Java reference oracle passes: %s\n' "$out_dir/output.txt"
