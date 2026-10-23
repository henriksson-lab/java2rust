#!/usr/bin/env bash
# Descriptor census of every pinned production class, including private helpers.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${NUMBERS_SOURCE_ROOT:-$repo_root/.tmp/commons-numbers-1.3-src}"
out_dir="${NUMBERS_API_OUT:-$repo_root/.tmp/commons-numbers-1.3-core-api}"
bash "$repo_root/scripts/prepare-commons-numbers-1.3.sh" >&2
mkdir -p "$out_dir/classes"
mapfile -t numbers_sources < <(rg --files "$checkout/commons-numbers-core/src/main/java" -g '*.java' | sort)
if [[ "${#numbers_sources[@]}" != 11 ]]; then
  echo 'Expected all11 Core production sources' >&2
  exit 1
fi
javac -encoding UTF-8 -d "$out_dir/classes" "${numbers_sources[@]}" > "$out_dir/javac.log" 2>&1
mapfile -t numbers_class_files < <(rg --files "$out_dir/classes" -g '*.class' | sort)
numbers_classes=()
for numbers_class_file in "${numbers_class_files[@]}"; do
  numbers_relative="${numbers_class_file#"$out_dir/classes/"}"
  numbers_class="${numbers_relative%.class}"
  numbers_classes+=("${numbers_class//\//.}")
done
javap -classpath "$out_dir/classes" -p -s "${numbers_classes[@]}"
