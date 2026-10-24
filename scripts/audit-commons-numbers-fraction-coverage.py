#!/usr/bin/env python3
"""Link the complete javap public inventory to independent observation labels.

This checks the explicitly reviewed operation-to-request mapping. It does not
infer branch coverage from row counts or replace the translation/source audit.
"""
import argparse
import csv
import pathlib
import re

parser = argparse.ArgumentParser(description=__doc__)
for mode in ('fraction', 'bigfraction', 'gcf', 'continued', 'boundary', 'exception', 'null-exception'):
    parser.add_argument('--' + mode, type=pathlib.Path, required=True)
parser.add_argument('--output', type=pathlib.Path, required=True)
args = parser.parse_args()
repo = pathlib.Path(__file__).resolve().parent.parent
labels = {}
for mode in ('fraction', 'bigfraction', 'gcf', 'continued', 'boundary', 'exception', 'null-exception'):
    root = getattr(args, mode.replace('-', '_'))
    java = (root / 'java.tsv').read_bytes()
    rust = (root / 'rust.tsv').read_bytes()
    assert java and java == rust, f'{mode}: missing or unequal independent outputs'
    rows = java.decode().splitlines()
    labels[mode] = [row.split('\t', 1)[0] for row in rows]
    assert len(labels[mode]) == len(set(labels[mode])), f'{mode}: duplicate request labels'
    if mode == 'null-exception':
        assert len(rows) == 23, 'Missing null-exception observations, including nullable Supplier source calls'
        assert all(row.split('\t')[1:3] == ['java.lang.NullPointerException', 'true'] for row in rows), 'A null request did not throw the expected Java exception'


def request(owner, member, descriptor):
    """Explicit mapping of source APIs and compiler bridges to caller cases."""
    if owner == 'ContinuedFraction$1':
        assert member == 'get' and descriptor in (
            '()Lorg/apache/commons/numbers/fraction/GeneralizedContinuedFraction$Coefficient;',
            '()Ljava/lang/Object;'), (owner, member, descriptor)
        return 'continued', r'0:0:0:0:32:0'
    if owner == 'Coefficient':
        assert member in ('of', 'getA', 'getB'), (owner, member, descriptor)
        return 'gcf', r'coefficient:\d+'
    if owner == 'GeneralizedContinuedFraction':
        assert member == 'value', (owner, member, descriptor)
        overloads = ['(Ljava/util/function/Supplier;)D',
                     '(Ljava/util/function/Supplier;D)D',
                     '(Ljava/util/function/Supplier;DI)D',
                     '(DLjava/util/function/Supplier;)D',
                     '(DLjava/util/function/Supplier;D)D',
                     '(DLjava/util/function/Supplier;DI)D']
        return 'gcf', str(overloads.index(descriptor)) + r':\d+:\d+:\d+'
    if owner == 'ContinuedFraction':
        if member == owner:
            assert descriptor == '()V', descriptor
            return 'continued', r'.+:initial'
        if member in ('getA', 'getB'):
            assert descriptor == '(ID)D', descriptor
            # This successful evaluation records both callbacks' index/order
            # in the retained source object's call count and trace.
            return 'continued', r'0:0:0:0:32:0'
        assert member == 'evaluate', (owner, member, descriptor)
        defaults = {'(DD)D': 1, '(DDI)D': 0}[descriptor]
        return 'continued', rf'\d+:{defaults}:\d+:\d+:\d+:[01]'
    assert owner in ('Fraction', 'BigFraction'), (owner, member, descriptor)
    mode = owner.lower()
    parameters, returned = descriptor.split(')')
    parameters += ')'
    if member == 'compareTo' and parameters == '(Ljava/lang/Object;)':
        return 'boundary', 'comparable:[0-2]' if owner == 'Fraction' else 'comparable:[3-5]'
    if returned == 'Ljava/lang/Object;':
        # javac's erased NativeOperators bridges are actually invoked through
        # the interface by both Java and Rust callers, including self aliases.
        operation = {'add': 0, 'subtract': 1, 'multiply': 2, 'divide': 3,
                     'pow': 4, 'negate': 6, 'reciprocal': 7, 'zero': 8, 'one': 9}[member]
        if member == 'multiply' and parameters == '(I)':
            operation = 5
        return mode, rf'interface:\d+:{operation}'
    if member in ('ZERO', 'ONE'):
        return mode, 'constant-' + member.lower()
    if member == 'of':
        prefixes = {'(I)': 'of-int', '(II)': 'of-pair'} if owner == 'Fraction' else {
            '(I)': 'of-int', '(J)': 'of-long', '(Ljava/math/BigInteger;)': 'of-big',
            '(II)': 'of-int-pair', '(JJ)': 'of-long-pair',
            '(Ljava/math/BigInteger;Ljava/math/BigInteger;)': 'of-big-pair'}
        return mode, prefixes[parameters] + r':\d+'
    if member == 'from':
        return mode, {'(D)': r'from:\d+', '(DDI)': r'from-epsilon:\d+', '(DI)': r'from-limit:\d+'}[parameters]
    if member == 'parse':
        assert parameters == '(Ljava/lang/String;)', descriptor
        return mode, r'parse:\d+'
    if member in ('zero', 'one', 'abs', 'negate', 'reciprocal'):
        operation = {'abs': 0, 'negate': 1, 'reciprocal': 2, 'zero': 3, 'one': 4}[member]
        return mode, rf'unary:\d+:{operation}'
    if member in ('add', 'subtract', 'multiply', 'divide'):
        operation = {'add': 0, 'subtract': 1, 'multiply': 2, 'divide': 3}[member]
        if owner == 'Fraction':
            family = 'integer' if parameters == '(I)' else 'binary'
            assert parameters in ('(I)', '(Lorg/apache/commons/numbers/fraction/Fraction;)'), descriptor
            return mode, rf'{family}:\d+:-?\d+:{operation}'
        kind = {'(I)': 0, '(J)': 1, '(Ljava/math/BigInteger;)': 2,
                '(Lorg/apache/commons/numbers/fraction/BigFraction;)': 3}[parameters]
        return mode, rf'arithmetic:\d+:-?\d+:{kind}:{operation}'
    if member == 'pow':
        assert parameters == '(I)', descriptor
        return mode, r'pow:\d+:-?\d+'
    if member == 'bigDecimalValue':
        return mode, {'()': r'decimal-exact:\d+',
                      '(Ljava/math/RoundingMode;)': r'decimal-round:\d+:\d+',
                      '(ILjava/math/RoundingMode;)': r'decimal-scale:\d+:\d+:-?\d+'}[parameters]
    if member == 'compareTo':
        return mode, r'binary:\d+:\d+:4' if owner == 'Fraction' else r'compare:\d+:\d+'
    if member == 'equals':
        return mode, r'binary:\d+:\d+:5' if owner == 'Fraction' else r'equals:\d+:\d+'
    if member in ('getNumerator', 'getDenominator', 'getNumeratorAsInt', 'getNumeratorAsLong',
                  'getDenominatorAsInt', 'getDenominatorAsLong', 'isZero', 'isOne', 'signum',
                  'doubleValue', 'floatValue', 'intValue', 'longValue', 'byteValue', 'shortValue',
                  'hashCode', 'toString'):
        assert parameters == '()', descriptor
        # Every successfully constructed/output fraction is observed through
        # all of these getters in view(), including raw FP bits and UTF16 text.
        return mode, r'of-pair:\d+' if owner == 'Fraction' else r'of-big-pair:\d+'
    raise AssertionError(f'Unmapped public operation: {owner}.{member}{descriptor}')


owner = None
pending = None
coverage = []
for line in (repo / 'tests/corpus/numbers/Fraction-1.3-declarations.txt').read_text().splitlines():
    declaration = re.match(r'.*\bclass ([\w.$]+).*\{', line)
    if declaration:
        owner = declaration[1].rsplit('.', 1)[-1]
        if owner == 'GeneralizedContinuedFraction$Coefficient':
            owner = 'Coefficient'
    if (line.startswith('  public ') or line.startswith('  protected abstract ')) and line.endswith(';'):
        if '(' in line:
            member = re.split(r'[ .$]', line.split('(', 1)[0])[-1]
        else:
            member = line.rstrip(';').split()[-1]
        pending = (owner, member)
    elif pending and line.strip().startswith('descriptor:'):
        descriptor = line.split('descriptor:', 1)[1].strip()
        api_owner, member = pending
        if member in ('ZERO', 'ONE'):
            mode, pattern = api_owner.lower(), 'constant-' + member.lower()
        else:
            mode, pattern = request(api_owner, member, descriptor)
        matches = [label for label in labels[mode] if re.fullmatch(pattern, label)]
        assert matches, f'No observations: {api_owner}.{member}{descriptor} => {mode}:{pattern}'
        coverage.append((api_owner, member, descriptor, mode, pattern, len(matches)))
        pending = None

# javap's declared-member inventory omits these inherited Number defaults.
# Both independent view() callers explicitly observe them on every result.
for member, descriptor in [('byteValue', '()B'), ('shortValue', '()S')]:
    if not any(row[:2] == ('Fraction', member) for row in coverage):
        mode, pattern = request('Fraction', member, descriptor)
        matches = [label for label in labels[mode] if re.fullmatch(pattern, label)]
        assert matches, member
        coverage.append(('Fraction', member, descriptor, mode, pattern, len(matches)))

assert {row[0] for row in coverage} == {'BigFraction', 'Fraction', 'ContinuedFraction', 'ContinuedFraction$1', 'Coefficient', 'GeneralizedContinuedFraction'}
args.output.parent.mkdir(parents=True, exist_ok=True)
with args.output.open('w') as stream:
    writer = csv.writer(stream, delimiter='\t', lineterminator='\n')
    writer.writerow(('owner', 'member', 'descriptor', 'caller', 'request_pattern', 'observations'))
    writer.writerows(coverage)
print(f'{len(coverage)} public declarations mapped to matching independent observations')
