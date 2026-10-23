#!/usr/bin/env bash
# Complete pinned production declaration census, including private nested types.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${NUMBERS_SOURCE_ROOT:-$repo_root/.tmp/commons-numbers-1.3-src}"
out_dir="${NUMBERS_API_OUT:-$repo_root/.tmp/commons-numbers-1.3-angle-api}"
bash "$repo_root/scripts/prepare-commons-numbers-1.3.sh" >&2
mkdir -p "$out_dir/classes"
mapfile -t numbers_sources < <(rg --files "$checkout/commons-numbers-core/src/main/java" \
  "$checkout/commons-numbers-angle/src/main/java" -g '*.java' | sort)
[[ "${#numbers_sources[@]}" == 15 ]] || { echo 'Expected all 15 Angle+Core sources' >&2; exit 1; }
javac -encoding UTF-8 -d "$out_dir/classes" "${numbers_sources[@]}" > "$out_dir/javac.log" 2>&1
mapfile -t numbers_class_files < <(rg --files "$out_dir/classes/org/apache/commons/numbers/angle" -g '*.class' | sort)
numbers_classes=()
for numbers_class_file in "${numbers_class_files[@]}"; do
  numbers_relative="${numbers_class_file#"$out_dir/classes/"}"
  numbers_class="${numbers_relative%.class}"
  numbers_classes+=("${numbers_class//\//.}")
done
javap -classpath "$out_dir/classes" -p -s "${numbers_classes[@]}"
