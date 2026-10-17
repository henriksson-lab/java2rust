#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${JGRAPHT_SOURCE_ROOT:-$repo_root/.tmp/jgrapht-1.5.3-src}"
maven_cache="${JGRAPHT_MAVEN_CACHE:-$repo_root/.tmp/jgrapht-m2}"
out_dir="${JGRAPHT_ORACLE_OUT:-$repo_root/.tmp/jgrapht-java-oracle}"
expected_commit="be2abc30b9188e4099d737179e11bd669cf1e041"
if [[ "$(git -C "$checkout" rev-parse HEAD)" != "$expected_commit" ]] ||
   [[ -n "$(git -C "$checkout" status --short --untracked-files=no)" ]]; then
  echo "Java oracle requires the untouched pinned JGraphT release" >&2
  exit 1
fi
core_jar="$checkout/jgrapht-core/target/jgrapht-core-1.5.3.jar"
heap_jar="$maven_cache/org/jheaps/jheaps/0.14/jheaps-0.14.jar"
for jar in "$core_jar" "$heap_jar"; do
  if [[ ! -f "$jar" ]]; then
    echo "Missing $jar; run scripts/prepare-jgrapht-core-1.5.3.sh first" >&2
    exit 1
  fi
done
mkdir -p "$out_dir/classes"
fixture="$repo_root/tests/corpus/jgrapht/GraphOracle.java"
classpath="$core_jar:$heap_jar"
javac -cp "$classpath" -d "$out_dir/classes" "$fixture"
java -cp "$out_dir/classes:$classpath" GraphOracle > "$out_dir/actual.tsv"
diff -u "$repo_root/tests/corpus/jgrapht/expected.tsv" "$out_dir/actual.tsv"
sha256sum "$core_jar" "$heap_jar" "$fixture" > "$out_dir/input-sha256.txt"
java -version 2> "$out_dir/java-version.txt"
printf 'Java graph oracle passed: %s\n' "$out_dir/actual.tsv"
