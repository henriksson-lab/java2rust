#!/usr/bin/env bash
# Complete production declaration census, including private/nested members.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${NUMBERS_SOURCE_ROOT:-$repo_root/.tmp/commons-numbers-1.3-src}"
module="${NUMBERS_MODULE:-core}"
case "$module" in
  primes|rootfinder|core|complex) modules=("$module") ;;
  quaternion|angle) modules=(core "$module") ;;
  *) echo 'Unknown Commons Numbers module' >&2; exit 1 ;;
esac
out_dir="${NUMBERS_API_OUT:-$repo_root/.tmp/commons-numbers-1.3-$module-api}"
bash "$repo_root/scripts/prepare-commons-numbers-1.3.sh" >&2
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
javac -encoding UTF-8 -d "$out_dir/classes" "${sources[@]}" > "$out_dir/javac.log" 2>&1
mapfile -t class_files < <(rg --files "$out_dir/classes/org/apache/commons/numbers/$module" -g '*.class' | sort)
[[ "${#class_files[@]}" != 0 ]] || { echo 'Missing production class files' >&2; exit 1; }
classes=()
for class_file in "${class_files[@]}"; do
  relative="${class_file#"$out_dir/classes/"}"
  class_name="${relative%.class}"
  classes+=("${class_name//\//.}")
done
javap -classpath "$out_dir/classes" -p -s "${classes[@]}"
