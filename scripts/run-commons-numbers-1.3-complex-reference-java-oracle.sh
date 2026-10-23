#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${NUMBERS_SOURCE_ROOT:-$repo_root/.tmp/commons-numbers-1.3-src}"
out_dir="${NUMBERS_ORACLE_OUT:-$repo_root/.tmp/commons-numbers-1.3-complex-reference-java-oracle}"
bash "$repo_root/scripts/prepare-commons-numbers-1.3.sh"
python3 "$repo_root/scripts/complex_reference.py" "$checkout" "$out_dir" > /dev/null
mkdir -p "$out_dir/classes"
mapfile -t sources < <(rg --files "$checkout/commons-numbers-complex/src/main/java" -g '*.java' | sort)
[[ "${#sources[@]}" == 2 ]] || { echo 'Expected complete Complex production sources' >&2; exit 1; }
javac -encoding UTF-8 -d "$out_dir/classes" "${sources[@]}" \
  "$repo_root/tests/corpus/numbers/ComplexReferenceOracle.java" > "$out_dir/javac.log" 2>&1
java -cp "$out_dir/classes" org.apache.commons.numbers.complex.ComplexReferenceOracle "$out_dir/requests.tsv" \
  > "$out_dir/output.txt" 2> "$out_dir/stderr.log"
tail -n 1 "$out_dir/output.txt"
wc -l "$out_dir/output.txt"
