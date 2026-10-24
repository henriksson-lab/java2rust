#!/usr/bin/env python3
"""Map the reviewed Arrays public inventory to independent caller requests.

Writes a declaration coverage report even when other requests differ, but exits
unsuccessfully if any comparison is missing or unequal. The mapping proves API
invocation, not exhaustive branch coverage or production-closure completeness.
"""
import argparse
import csv
import pathlib
import re

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('behavior_root', type=pathlib.Path)
parser.add_argument('--helpers-root', type=pathlib.Path)
parser.add_argument('--output', type=pathlib.Path, required=True)
args = parser.parse_args()
repo = pathlib.Path(__file__).resolve().parent.parent
labels = {}
differences = {}
for mode in ('selection', 'sort', 'counter', 'helpers'):
    root = args.helpers_root if mode == 'helpers' and args.helpers_root else args.behavior_root
    java = (root / mode / 'java.tsv').read_text().splitlines()
    rust = (root / mode / 'rust.tsv').read_text().splitlines()
    assert java and rust, f'{mode}: missing observations'
    expected = {row.split('\t', 1)[0]: row for row in java}
    actual = {row.split('\t', 1)[0]: row for row in rust}
    assert len(expected) == len(java) and len(actual) == len(rust), f'{mode}: duplicate labels'
    assert expected.keys() == actual.keys(), f'{mode}: differing request sets'
    labels[mode] = [label for label, row in expected.items() if actual[label] == row]
    differences[mode] = len(java) - len(labels[mode])


def request(owner, member, descriptor):
    if owner in ('BitIndexUpdatingInterval', 'KeyUpdatingInterval', 'UpdatingInterval'):
        assert member in ('left', 'right', 'updateLeft', 'updateRight', 'splitLeft'), (owner, member)
        bit = {'BitIndexUpdatingInterval': '1', 'KeyUpdatingInterval': '0', 'UpdatingInterval': '[01]'}[owner]
        phase = 'initial' if member in ('left', 'right') else 'split' if member == 'splitLeft' else 'reuse'
        return 'helpers', rf'interval:{bit}:{phase}'
    if owner == 'Selection':
        assert member == 'select', member
        kind = {'D': 0, 'I': 1, 'J': 2}[descriptor[2]]
        suffix = descriptor[3:]
        variant = {'I)V': 0, '[I)V': 1, 'III)V': 2, 'II[I)V': 3}[suffix]
        return 'selection', rf'selection:{kind}:\d+:\d+:{variant}:\d+'
    if owner == 'MultidimensionalCounter':
        phase = {'of': 'construct', 'getDimension': 'view', 'getSize': 'view',
                 'getSizes': 'copies', 'toString': 'view', 'toMulti': r'index:\d+',
                 'toUni': r'index:\d+'}[member]
        return 'counter', rf'counter:\d+:{phase}'
    if owner == 'SortInPlace':
        if member == 'apply':
            assert descriptor == '([D[[D)V', descriptor
            return 'sort', r'sort:[01]:\d+:\d+:[0-6]'
        assert member in ('ASCENDING', 'DESCENDING', 'values', 'valueOf', 'name', 'ordinal', 'compareTo', 'toString'), member
        return 'sort', 'enum:' + {'ASCENDING': '0', 'DESCENDING': '1'}.get(member, '[01]')
    raise AssertionError((owner, member, descriptor))


coverage = []


def record(owner, member, descriptor):
    mode, pattern = request(owner, member, descriptor)
    matches = [label for label in labels[mode] if re.fullmatch(pattern, label)]
    assert matches, f'No matching independent observations for {owner}.{member}{descriptor}'
    coverage.append((owner, member, descriptor, mode, pattern, len(matches)))


owner = pending = None
inventory = repo / 'tests/corpus/numbers/Arrays-1.3-declarations.txt'
for line in inventory.read_text().splitlines():
    declaration = re.match(r'.*\b(?:class|interface) ([\w.$]+).*\{', line)
    if declaration:
        owner = declaration[1].rsplit('.', 1)[-1]
    if line.startswith('  public ') and line.endswith(';'):
        member = line.split('(', 1)[0].split()[-1].rsplit('.', 1)[-1] if '(' in line else line.rstrip(';').split()[-1]
        pending = (owner, member)
    elif pending and line.strip().startswith('descriptor:'):
        record(*pending, line.split('descriptor:', 1)[1].strip())
        pending = None

for member, descriptor in (('name', '()Ljava/lang/String;'), ('ordinal', '()I'),
                           ('compareTo', '(Ljava/lang/Enum;)I'), ('toString', '()Ljava/lang/String;')):
    record('SortInPlace', member, descriptor)
assert len(coverage) == 43, f'Unexpected public API count: {len(coverage)}'
args.output.parent.mkdir(parents=True, exist_ok=True)
with args.output.open('w') as stream:
    writer = csv.writer(stream, delimiter='\t', lineterminator='\n')
    writer.writerow(('owner', 'member', 'descriptor', 'caller', 'request_pattern', 'exact_observations'))
    writer.writerows(coverage)
print(f'{len(coverage)} public/inherited declarations mapped to exact independent observations')
print('Unequal requests: ' + ', '.join(f'{mode}={count}' for mode, count in differences.items()))
raise SystemExit(int(any(differences.values())))
