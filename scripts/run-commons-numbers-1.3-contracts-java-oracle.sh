#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
module="${NUMBERS_MODULE:-core}"
case "$module" in
  core) expected=11; client=CoreContractsOracle; modules=(core) ;;
  complex) expected=2; client=ComplexContractsOracle; modules=(complex) ;;
  quaternion) expected=14; client=QuaternionContractsOracle; modules=(core quaternion) ;;
  angle) expected=15; client=AngleContractsOracle; modules=(core angle) ;;
  *) echo 'Contract module must be core, complex, quaternion or angle' >&2; exit 1 ;;
esac
checkout="${NUMBERS_SOURCE_ROOT:-$repo_root/.tmp/commons-numbers-1.3-src}"
out_dir="${NUMBERS_ORACLE_OUT:-$repo_root/.tmp/commons-numbers-1.3-$module-contracts-java-oracle}"
bash "$repo_root/scripts/prepare-commons-numbers-1.3.sh"
mkdir -p "$out_dir/classes"
sources=()
manifest="$repo_root/scripts/focus-sets/commons-numbers-1.3-all.txt"
for selected in "${modules[@]}"; do
    prefix="commons-numbers-$selected/src/main/java/"
    diff -u <(find "$checkout/$prefix" -type f -name '*.java' -printf '%P\n' | sort) \
      <(sed -n "s|^$prefix||p" "$manifest" | sort)
    while IFS= read -r relative; do
        [[ "$relative" == "$prefix"* ]] || continue
        sources+=("$checkout/$relative")
    done < "$manifest"
done
[[ "${#sources[@]}" == "$expected" ]] || { echo "Expected all $expected production sources" >&2; exit 1; }
javac -encoding UTF-8 -d "$out_dir/classes" "${sources[@]}" \
    "$repo_root/tests/corpus/numbers/$client.java" > "$out_dir/javac.log" 2>&1
java -cp "$out_dir/classes" "org.apache.commons.numbers.$module.$client" \
    > "$out_dir/output.txt" 2> "$out_dir/stderr.log"
cat "$out_dir/output.txt"
