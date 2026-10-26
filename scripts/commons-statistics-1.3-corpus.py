#!/usr/bin/env python3
"""Inventory pinned production modules and stage complete dependency closures.

POM compile/runtime/provided dependencies determine the closure, not import-name
heuristics or a selected list of algorithm classes. No Java source is rewritten.
Run prepare-commons-statistics-1.3.sh first to validate the clean checkout pins.
"""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import xml.etree.ElementTree as ET

REPO = Path(__file__).resolve().parent.parent
NS = {'p': 'http://maven.apache.org/POM/4.0.0'}
PINS = {
    'statistics': ('1.3', '2937eb2e711483d8ea9dc216af45c16fd0066b77', 'STATISTICS_SOURCE_ROOT'),
    'numbers': ('1.3', 'ae8abefb51c2c4d1a05882d61cefc8015e493a8f', 'NUMBERS_SOURCE_ROOT'),
    'rng': ('1.6', '618490cf3f072ddc327de90109fac04f404831cb', 'STATISTICS_RNG_SOURCE_ROOT'),
}
TARGETS = ('ranking', 'descriptive', 'distribution', 'inference', 'interval')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--module', choices=(*TARGETS, 'all'), default='all')
    parser.add_argument('--inventory', type=Path, required=True)
    parser.add_argument('--stage', type=Path)
    args = parser.parse_args()
    roots = {}
    modules = {}
    for family, (version, commit, variable) in PINS.items():
        root = Path(os.environ.get(variable, REPO / '.tmp' / f'commons-{family}-{version}-src')).resolve()
        assert subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip() == commit
        assert not subprocess.check_output(['git', '-C', str(root), 'status', '--porcelain'], text=True)
        roots[family] = root
        for directory in sorted(root.glob(f'commons-{family}-*')):
            sources = sorted((directory / 'src/main/java').rglob('*.java'))
            if not sources:
                continue
            pom = ET.parse(directory / 'pom.xml').getroot()
            artifact = pom.findtext('p:artifactId', namespaces=NS)
            dependencies = []
            for dep in pom.findall('p:dependencies/p:dependency', NS):
                scope = dep.findtext('p:scope', default='compile', namespaces=NS)
                if scope == 'test':
                    continue
                assert scope in ('compile', 'runtime', 'provided'), (artifact, scope)
                assert dep.findtext('p:groupId', namespaces=NS) == 'org.apache.commons', artifact
                dependencies.append(dep.findtext('p:artifactId', namespaces=NS))
            modules[artifact] = {
                'family': family, 'version': version, 'commit': commit,
                'directory': str(directory), 'dependencies': dependencies,
                'sources': sources,
            }
    statistics = ET.parse(roots['statistics'] / 'pom.xml').getroot()
    assert statistics.findtext('p:properties/p:statistics.commons.rng.version', namespaces=NS) == PINS['rng'][0]
    assert statistics.findtext('p:properties/p:statistics.commons.numbers.version', namespaces=NS) == PINS['numbers'][0]
    declared = [x.text for x in statistics.findall('p:modules/p:module', NS)]
    production = {x for x in declared if x in modules}
    assert production == {f'commons-statistics-{name}' for name in TARGETS}, production

    def closure(artifact):
        found = set()
        pending = [artifact]
        while pending:
            current = pending.pop()
            if current in found:
                continue
            assert current in modules, ('Unresolved production dependency', current)
            found.add(current)
            pending.extend(modules[current]['dependencies'])
        return sorted(found)

    closures = {name: closure(f'commons-statistics-{name}') for name in TARGETS}
    selected = sorted(set().union(*(closures.values() if args.module == 'all' else [closures[args.module]])))
    rows = []
    for artifact in sorted(set().union(*closures.values())):
        module = modules[artifact]
        root = roots[module['family']]
        for source in module['sources']:
            rows.append({
                'family': module['family'], 'module': artifact,
                'relative_path': str(source.relative_to(root)),
                'sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
                'source': str(source),
            })
    args.inventory.mkdir(parents=True, exist_ok=True)
    with (args.inventory / 'production-sources.tsv').open('w') as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]), delimiter='\t', lineterminator='\n')
        writer.writeheader()
        writer.writerows(rows)
    report = {
        'pins': {family: {'version': data[0], 'commit': data[1], 'root': str(roots[family])} for family, data in PINS.items()},
        'production_targets': list(TARGETS),
        'excluded': {'metadata_and_documentation': [x for x in declared if x not in production],
                     'nonproduction': ['commons-statistics-examples', 'dist-archive', 'src/test', 'benchmarks']},
        'modules': {name: {**{k:v for k,v in value.items() if k != 'sources'}, 'java_files': len(value['sources'])} for name,value in modules.items() if name in set().union(*closures.values())},
        'closures': {name: {'modules': deps, 'java_files': sum(len(modules[d]['sources']) for d in deps)} for name,deps in closures.items()},
        'selected': selected,
    }
    if args.stage:
        assert not args.stage.exists(), f'Use a fresh staging root: {args.stage}'
        for row in rows:
            if row['module'] not in selected:
                continue
            destination = args.stage / row['family'] / row['relative_path']
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(row['source'], destination)
            assert destination.read_bytes() == Path(row['source']).read_bytes()
        count = len(list(args.stage.rglob('*.java')))
        assert count == sum(len(modules[d]['sources']) for d in selected)
        report['staged_java_files'] = count
    (args.inventory / 'production-scope.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
