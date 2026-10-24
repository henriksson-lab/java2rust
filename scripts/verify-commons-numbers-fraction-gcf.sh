#!/usr/bin/env bash
# Reuse a complete generated Fraction closure; callers remain independent.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checkout="${NUMBERS_SOURCE_ROOT:-$repo_root/.tmp/commons-numbers-1.3-src}"
translation="${NUMBERS_TRANSLATION_ROOT:?Set NUMBERS_TRANSLATION_ROOT to a complete Fraction translation}"
output="${NUMBERS_BEHAVIOR_ROOT:-$repo_root/.tmp/commons-numbers-fraction-gcf-behavior}"
case "${1:-gcf}" in
  gcf) oracle_class=GeneralizedFractionOracle; oracle_rust=generalized_fraction_oracle.rs ;;
  fraction) oracle_class=FractionOracle; oracle_rust=fraction_oracle.rs ;;
  bigfraction) oracle_class=BigFractionOracle; oracle_rust=big_fraction_oracle.rs ;;
  continued) oracle_class=ContinuedFractionOracle; oracle_rust=continued_fraction_oracle.rs ;;
  boundary) oracle_class=FractionBoundaryOracle; oracle_rust=fraction_boundary_oracle.rs ;;
  exception) oracle_class=FractionExceptionIdentityOracle; oracle_rust=fraction_exception_identity_oracle.rs ;;
  null-exception) oracle_class=FractionNullExceptionOracle; oracle_rust=fraction_null_exception_oracle.rs ;;
  *) printf 'Usage: %s [gcf|fraction|bigfraction|continued|boundary|exception|null-exception]\n' "$0" >&2; exit 2 ;;
esac
bash "$repo_root/scripts/prepare-commons-numbers-1.3.sh" > /dev/null
python3 - "$repo_root" "$checkout" "$translation" "$output" "$oracle_rust" "$oracle_class" <<'PY'
import csv, hashlib, json, pathlib, sys
repo, checkout, translation, output = map(pathlib.Path, sys.argv[1:5])
oracle_rust = sys.argv[5]
oracle_java = sys.argv[6] + '.java'
fixture_name = {'continued_fraction_oracle.rs': 'ContinuedFractionExtension.java', 'fraction_boundary_oracle.rs': 'FractionNullBoundary.java', 'fraction_null_exception_oracle.rs': 'FractionNullBoundary.java'}.get(oracle_rust)
fixture = repo / 'tests/corpus/numbers/fraction-acceptance' / fixture_name if fixture_name else None
prefixes = ('commons-numbers-core/', 'commons-numbers-fraction/')
expected = {line for line in (repo / 'scripts/focus-sets/commons-numbers-1.3-all.txt').read_text().splitlines() if line.startswith(prefixes)}
with (translation / 'batch-manifest.tsv').open() as stream:
    rows = list(csv.DictReader(stream, delimiter='\t'))
actual = set()
extra_count = 0
provenance = []
for row in rows:
    source = pathlib.Path(row['source'])
    relative = next((str(source)[str(source).index(prefix):] for prefix in prefixes if prefix in str(source)), None)
    if relative is None:
        assert fixture is not None and source.name == fixture.name, f'Unexpected extension source: {source}'
        assert source.read_bytes() == fixture.read_bytes(), f'Modified extension fixture: {source}'
        extra_count += 1
    else:
        assert relative in expected, f'Unexpected production source: {source}'
        assert source.read_bytes() == (checkout / relative).read_bytes(), f'Modified source: {source}'
        actual.add(relative)
    assert pathlib.Path(row['rust_path']).is_file(), f'Missing generated file: {row}'
    assert row['invalid_patterns'] == '0' and row['stubbed_constants'] == '0', row
    provenance.append(('translated-input', relative or source.name,
                       hashlib.sha256(source.read_bytes()).hexdigest()))
assert extra_count == int(fixture is not None), 'Missing or duplicate extension fixture'
assert actual == expected and len(rows) == len(expected) + extra_count, 'Incomplete Fraction production closure'
probe = output / 'probe'
(probe / 'src').mkdir(parents=True, exist_ok=True)
generated = (translation / 'generated-crate').resolve()
# cargo run releases its build lock before executing the binary. Distinct
# callers/checkpoints sharing CARGO_TARGET_DIR must not overwrite that path.
caller_name = 'fraction-independent-' + hashlib.sha256(str(probe.resolve()).encode()).hexdigest()[:16]
(probe / 'Cargo.toml').write_text('[package]\nname=' + json.dumps(caller_name) + '\nversion="0.0.0"\nedition="2021"\n[dependencies]\ntranslated={package="new-java2rust-focused-output",path=' + json.dumps(str(generated)) + '}\n')
if oracle_rust in ('big_fraction_oracle.rs', 'fraction_null_exception_oracle.rs'):
    with (probe / 'Cargo.toml').open('a') as stream:
        stream.write('java-math-runtime={path=' + json.dumps(str(repo / 'runtime/java-math')) + '}\n')
snapshots = output / 'callers'
snapshots.mkdir(parents=True, exist_ok=True)
for name in [oracle_java, oracle_rust] + ([fixture_name] if fixture_name else []):
    data = (repo / 'tests/corpus/numbers/fraction-acceptance' / name).read_bytes()
    (snapshots / name).write_bytes(data)
    provenance.append(('independent-caller' if name != fixture_name else 'translated-extension',
                       name, hashlib.sha256(data).hexdigest()))
(probe / 'src/main.rs').write_bytes((snapshots / oracle_rust).read_bytes())
(snapshots / 'verify-commons-numbers-fraction-gcf.sh').write_bytes(
    (repo / 'scripts/verify-commons-numbers-fraction-gcf.sh').read_bytes())
(output / 'translation-producer.tsv').write_bytes((translation / 'translation-producer.tsv').read_bytes())
with (output / 'input-provenance.tsv').open('w') as stream:
    writer = csv.writer(stream, delimiter='\t', lineterminator='\n')
    writer.writerow(('role', 'source', 'sha256'))
    writer.writerows(sorted(provenance))
PY
mkdir -p "$output/classes"
sources=()
while IFS= read -r relative; do
  case "$relative" in
    commons-numbers-core/*|commons-numbers-fraction/*) sources+=("$checkout/$relative") ;;
  esac
done < "$repo_root/scripts/focus-sets/commons-numbers-1.3-all.txt"
if [[ "$oracle_class" == ContinuedFractionOracle ]]; then
  sources+=("$output/callers/ContinuedFractionExtension.java")
elif [[ "$oracle_class" == FractionBoundaryOracle || "$oracle_class" == FractionNullExceptionOracle ]]; then
  sources+=("$output/callers/FractionNullBoundary.java")
fi
javac -encoding UTF-8 -d "$output/classes" "${sources[@]}" \
  "$output/callers/$oracle_class.java" > "$output/javac.log" 2>&1
java -cp "$output/classes" "$oracle_class" > "$output/java.tsv"
CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true cargo run --quiet --offline \
  --manifest-path "$output/probe/Cargo.toml" > "$output/rust.tsv" 2> "$output/rust-build.log"
diff -u "$output/java.tsv" "$output/rust.tsv" > "$output/diff.txt"
printf '%s: %s independent rows match exactly\n' "$oracle_class" "$(wc -l < "$output/java.tsv")"
