#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
translation="${NUMBERS_TRANSLATION_ROOT:-$repo_root/.tmp/commons-numbers-1.3-arrays-baseline}"
output="${NUMBERS_BEHAVIOR_ROOT:-$repo_root/.tmp/commons-numbers-arrays-behavior}"
checkout="${NUMBERS_SOURCE_ROOT:-$repo_root/.tmp/commons-numbers-1.3-src}"
mode="${1:-all}"
case "$mode" in
  all) modes=(selection sort counter helpers) ;;
  selection|sort|counter|helpers|null-boundary|null-exception) modes=("$mode") ;;
  *) echo 'Expected all, selection, sort, counter, helpers, null-boundary or null-exception' >&2; exit 2 ;;
esac
[[ ! -e "$output" ]] || { echo "Use a fresh behavior root: $output" >&2; exit 1; }
bash "$repo_root/scripts/prepare-commons-numbers-1.3.sh" > /dev/null
python3 - "$repo_root" "$checkout" "$translation" "$output" "$mode" <<'PY'
import csv, hashlib, json, pathlib, sys
repo, checkout, translation, output = map(pathlib.Path, sys.argv[1:5])
boundary = sys.argv[5] in ('null-boundary', 'null-exception')
prefix = 'commons-numbers-arrays/'
expected = {line for line in (repo / 'scripts/focus-sets/commons-numbers-1.3-all.txt').read_text().splitlines() if line.startswith(prefix)}
summary = dict(csv.reader((translation / 'summary.tsv').open(), delimiter='\t'))
for key in ('translate_failed_files', 'parse_failures', 'unsupported_statements', 'unsupported_expressions', 'unsupported_members', 'stubbed_constants', 'invalid_pattern_files', 'noop_facade_call_sites', 'cargo_check_errors'):
    assert summary[key] == '0', (key, summary[key])
with (translation / 'batch-manifest.tsv').open() as stream:
    rows = list(csv.DictReader(stream, delimiter='\t'))
actual = set()
provenance = []
extra = 0
for row in rows:
    source = pathlib.Path(row['source'])
    if prefix not in str(source):
        assert boundary and source.name == 'ArraysNullBoundary.java', source
        assert source.read_bytes() == (repo / 'tests/corpus/numbers/arrays-acceptance/ArraysNullBoundary.java').read_bytes(), source
        assert pathlib.Path(row['rust_path']).is_file() and row['invalid_patterns'] == row['stubbed_constants'] == '0', row
        provenance.append(('translated-null-client', source.name, hashlib.sha256(source.read_bytes()).hexdigest()))
        extra += 1
        continue
    relative = str(source)[str(source).index(prefix):]
    assert relative in expected and relative not in actual, relative
    assert source.read_bytes() == (checkout / relative).read_bytes(), source
    assert pathlib.Path(row['rust_path']).is_file(), row
    assert row['invalid_patterns'] == row['stubbed_constants'] == '0', row
    actual.add(relative)
    provenance.append(('translated-input', relative, hashlib.sha256(source.read_bytes()).hexdigest()))
assert actual == expected and extra == int(boundary) and len(rows) == 11 + extra, 'Incomplete Arrays production closure'
output.mkdir(parents=True)
snapshots = output / 'callers'
snapshots.mkdir()
oracle_java, oracle_rust = {'null-boundary': ('ArraysNullBoundary.java', 'arrays_null_oracle.rs'), 'null-exception': ('ArraysNullExceptionOracle.java', 'arrays_null_exception_oracle.rs')}.get(sys.argv[5], ('ArraysOracle.java', 'arrays_oracle.rs'))
caller_files = [oracle_java, oracle_rust]
if sys.argv[5] == 'null-exception': caller_files.append('ArraysNullBoundary.java')
for name in caller_files:
    data = (repo / 'tests/corpus/numbers/arrays-acceptance' / name).read_bytes()
    (snapshots / name).write_bytes(data)
    provenance.append(('independent-caller', name, hashlib.sha256(data).hexdigest()))
(snapshots / 'verify-commons-numbers-arrays.sh').write_bytes((repo / 'scripts/verify-commons-numbers-arrays.sh').read_bytes())
(output / 'translation-producer.tsv').write_bytes((translation / 'translation-producer.tsv').read_bytes())
probe = output / 'probe'
(probe / 'src').mkdir(parents=True)
name = 'arrays-independent-' + hashlib.sha256(str(probe.resolve()).encode()).hexdigest()[:16]
(output / 'caller-name.txt').write_text(name + '\n')
(probe / 'Cargo.toml').write_text('[package]\nname=' + json.dumps(name) + '\nversion="0.0.0"\nedition="2021"\n[dependencies]\ntranslated={package="new-java2rust-focused-output",path=' + json.dumps(str((translation / 'generated-crate').resolve())) + '}\n')
(probe / 'src/main.rs').write_bytes((snapshots / oracle_rust).read_bytes())
with (output / 'input-provenance.tsv').open('w') as stream:
    writer = csv.writer(stream, delimiter='\t', lineterminator='\n')
    writer.writerow(('role','source','sha256'))
    writer.writerows(sorted(provenance))
PY
mkdir -p "$output/classes"
sources=()
while IFS= read -r relative; do
  [[ "$relative" == commons-numbers-arrays/* ]] && sources+=("$checkout/$relative")
done < "$repo_root/scripts/focus-sets/commons-numbers-1.3-all.txt"
if [[ "$mode" == null-boundary ]]; then
  oracle_class=ArraysNullBoundary
elif [[ "$mode" == null-exception ]]; then
  oracle_class=ArraysNullExceptionOracle
  sources+=("$output/callers/ArraysNullBoundary.java")
else
  oracle_class=org.apache.commons.numbers.arrays.ArraysOracle
fi
oracle_java="${oracle_class##*.}.java"
javac -encoding UTF-8 -d "$output/classes" "${sources[@]}" "$output/callers/$oracle_java" > "$output/javac.log" 2>&1
target_dir="${CARGO_TARGET_DIR:-$output/probe/target}"
caller_name="$(cat "$output/caller-name.txt")"
CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$target_dir" cargo build --offline --manifest-path "$output/probe/Cargo.toml" > "$output/rust-build.log" 2>&1
result=0
for selected in "${modes[@]}"; do
  mkdir "$output/$selected"
  java -Xmx256m -cp "$output/classes" "$oracle_class" "$selected" > "$output/$selected/java.tsv" 2> "$output/$selected/java-run.log" || result=1
  "$target_dir/debug/$caller_name" "$selected" > "$output/$selected/rust.tsv" 2> "$output/$selected/rust-run.log" || result=1
  if diff -u "$output/$selected/java.tsv" "$output/$selected/rust.tsv" > "$output/$selected/diff.txt"; then
    printf '%s: %s independent rows match exactly\n' "$selected" "$(wc -l < "$output/$selected/java.tsv")"
  else
    printf '%s: independent outputs differ; see %s\n' "$selected" "$output/$selected/diff.txt"
    result=1
  fi
  if [[ "$selected" == null-boundary ]]; then
    python3 - "$output/$selected/java.tsv" <<'PY' || result=1
import pathlib, sys
assert pathlib.Path(sys.argv[1]).read_text().splitlines() == [f'{i}\t0' for i in range(20)], 'A null boundary failed its exception-type/state contract'
PY
  fi
  if [[ "$selected" == null-exception ]]; then
    python3 - "$output/$selected/java.tsv" <<'PY' || result=1
import pathlib, sys
rows=[line.split('\t') for line in pathlib.Path(sys.argv[1]).read_text().splitlines()]
assert len(rows)==20 and [row[0] for row in rows]==list(map(str,range(20)))
assert all(row[1:3]==['java.lang.NullPointerException','true'] for row in rows), 'Missing direct NPE observation'
PY
  fi
done
exit "$result"
