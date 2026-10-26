#!/usr/bin/env python3
"""Reproduce runtime/RSS/parity measurements using frozen, unpatched translations.

Does not run or modify the translator. Java/Rust callers only construct inputs,
invoke generated algorithms, and serialize observations. One fresh process per
configuration; compilation is excluded from timing. See benchmarks/showcase/README.txt.
"""
import argparse
import csv
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys

REPO = Path(__file__).resolve().parent.parent
CALLERS = REPO / 'benchmarks/showcase'
DEFAULTS = {
    'varscan': ('/home/mahogny/github/claude/java2rust/testdata/varscan/src', '.tmp/varscan-full-v5', 18),
    'jahmm': ('/home/mahogny/github/claude/java2rust/testdata/jahmm/src/main/java', '.tmp/jahmm-full-v122', 95),
}

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def fnv(path):
    value = 0xcbf29ce484222325
    for byte in path.read_bytes():
        value = ((value ^ byte) * 0x100000001b3) & 0xffffffffffffffff
    return f'{value:016x}'

def run(command, out, name, cwd=None, env=None, timed=False):
    actual = list(map(str, command))
    if timed:
        actual = ['/usr/bin/time', '-f', 'wall_s=%e\nuser_s=%U\nsystem_s=%S\nmax_rss_kib=%M\nexit=%x', '-o', str(out / (name + '.time')), *actual]
    with (out / (name + '.stdout')).open('wb') as stdout, (out / (name + '.stderr')).open('wb') as stderr:
        result = subprocess.run(actual, cwd=cwd, env=env, stdout=stdout, stderr=stderr, timeout=600)
    record = {'command': actual, 'exit_code': result.returncode}
    if timed and (out / (name + '.time')).exists():
        record['metrics'] = dict(line.split('=', 1) for line in (out / (name + '.time')).read_text().splitlines() if '=' in line)
        print(name, record['metrics'], flush=True)
    if result.returncode:
        raise RuntimeError(f'{name} exited {result.returncode}; inspect {out / (name + ".stderr")}')
    return record

def compare(kind, java, rust, length=None, batches=None):
    left, right = java.read_bytes(), rust.read_bytes()
    result = {'stdout_byte_identical': left == right, 'java_lines': len(left.splitlines()), 'rust_lines': len(right.splitlines()), 'java_sha256': sha(java), 'rust_sha256': sha(rust)}
    if kind == 'varscan':
        result['parity_passed'] = left == right
        return result
    a, b = [x.decode().split('\t') for x in left.splitlines()], [x.decode().split('\t') for x in right.splitlines()]
    if len(a) != batches or len(b) != batches:
        raise ValueError('Missing Jahmm sequence observations')
    paths = True
    floats = []
    for x, y in zip(a, b):
        if len(x) != 4 or len(y) != 4 or x[0] != y[0]:
            raise ValueError('Malformed Jahmm observations')
        if len(x[3].split(',')) != length or len(y[3].split(',')) != length:
            raise ValueError('Missing decoded states')
        paths &= x[3] == y[3]
        for label, i in [('viterbi_log_likelihood', 1), ('scaled_forward_log_likelihood', 2)]:
            xb, yb = int(x[i], 16), int(y[i], 16)
            xf, yf = [struct.unpack('>d', z.to_bytes(8, 'big'))[0] for z in (xb, yb)]
            ordered = lambda z: (~z & 0xffffffffffffffff) if z >> 63 else z | (1 << 63)
            floats.append({'sequence': int(x[0]), 'quantity': label, 'java_bits': x[i], 'rust_bits': y[i], 'exact_bits': xb == yb,
                           'absolute_error': abs(xf-yf) if math.isfinite(xf) and math.isfinite(yf) else None,
                           'ulp_distance': abs(ordered(xb)-ordered(yb))})
    result.update(decoded_paths_exact=paths, states_compared=length*batches, likelihood_values=len(floats),
                  likelihood_bits_exact=all(x['exact_bits'] for x in floats),
                  max_abs_likelihood_error=max((x['absolute_error'] for x in floats if x['absolute_error'] is not None), default=None),
                  max_likelihood_ulp_distance=max(x['ulp_distance'] for x in floats), likelihood_details=floats,
                  parity_passed=left == right, numerical_tolerance_applied=False)
    return result

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('corpus', choices=DEFAULTS)
    parser.add_argument('--source-root', type=Path, help='Directory containing the original complete Java production source tree')
    parser.add_argument('--translation-root', type=Path, help='Archived translation directory containing generated-crate and batch-manifest.tsv')
    parser.add_argument('--out', type=Path, required=True, help='Fresh output directory (prefer /tmp for build space)')
    parser.add_argument('--target-dir', type=Path)
    parser.add_argument('--positions', type=int, default=100000, help='VarScan pileup positions; multiple of four')
    parser.add_argument('--states', type=int, default=16)
    parser.add_argument('--length', type=int, default=4000)
    parser.add_argument('--sequences', type=int, default=32)
    parser.add_argument('--java-heap', default='256m', help='Additional JVM run with -Xms32m and this -Xmx; use none to skip')
    args = parser.parse_args()
    source_default, translation_default, count = DEFAULTS[args.corpus]
    source = (args.source_root or Path(source_default)).resolve()
    translation = (args.translation_root or REPO / translation_default).resolve()
    out = args.out.resolve()
    if out.exists(): parser.error('--out must be a fresh directory')
    if min(args.states, args.length, args.sequences) < 1 or args.positions < 4 or args.positions % 4:
        parser.error('HMM dimensions must be positive; positions must be a positive multiple of four')
    files = sorted(source.rglob('*.java'))
    if len(files) != count: parser.error(f'Expected {count} Java files; found {len(files)} under {source}')
    manifest = list(csv.DictReader((translation / 'batch-manifest.tsv').open(), delimiter='\t'))
    if len(manifest) != count: parser.error('Archived translation census differs')
    summary = dict(csv.reader((translation / 'summary.tsv').open(), delimiter='\t'))
    primary = Path(summary['primary'])
    if sorted(str(p.relative_to(source)) for p in files) != sorted(str(Path(r['source']).relative_to(primary)) for r in manifest):
        parser.error('Java source census differs from archived translation')
    raw = translation / 'generated-raw-before-support-strip'
    raw_checks = {Path(r['rust_path']).name: fnv(raw / Path(r['rust_path']).name) == r['rust_hash'] for r in manifest}
    if not all(raw_checks.values()): parser.error('Archived raw generated modules differ from translator manifest; refusing benchmark')
    out.mkdir(parents=True)
    original = translation / 'generated-crate'
    copied = out / 'translated'
    copied.mkdir()
    for name in ('Cargo.toml', 'Cargo.lock'):
        if (original / name).exists(): shutil.copy2(original / name, copied / name)
    for name in ('src', 'ndarray-shim'):
        if (original / name).exists(): shutil.copytree(original / name, copied / name)
    (copied / 'examples').mkdir()
    wrapper = CALLERS / (args.corpus + '_main.rs')
    shutil.copy2(wrapper, copied / 'examples/showcase.rs')
    classes = out / 'java-classes'; classes.mkdir()
    frozen = {str(p): sha(p) for p in files}
    frozen.update({str(p): sha(p) for p in original.joinpath('src').rglob('*.rs')})
    frozen.update({str(p): sha(p) for p in (wrapper, Path(__file__).resolve(), original/'Cargo.toml')})
    if args.corpus == 'jahmm': frozen[str(CALLERS/'JahmmShowcase.java')] = sha(CALLERS/'JahmmShowcase.java')
    provenance = {'status': 'running', 'corpus': args.corpus, 'source_root': str(source), 'translation_root': str(translation), 'source_files': count,
                  'source_lines_including_comments_and_blanks': sum(len(p.read_bytes().splitlines()) for p in files), 'inputs': frozen,
                  'raw_manifest_checks': raw_checks, 'archived_summary': summary, 'repetitions_per_configuration': 1,
                  'timing_scope': 'Fresh process including startup/JIT, allocations and output; compilation excluded; no regeneration or generated body repairs.',
                  'commands': {}, 'arguments': {k: str(v) if isinstance(v, Path) else v for k,v in vars(args).items()}}
    results = {}
    try:
        env = os.environ.copy()
        env.pop('CARGO_MAKEFLAGS', None); env.pop('MAKEFLAGS', None)
        target = (args.target_dir or out / 'cargo-target').resolve()
        env.update(CARGO_INCREMENTAL='0', CARGO_NET_OFFLINE='true', CARGO_BUILD_JOBS='8', CARGO_TARGET_DIR=str(target))
        provenance['commands']['rust_build'] = run(['cargo','build','--release','--offline','--manifest-path',copied/'Cargo.toml','--example','showcase'],out,'rust-build',env=env)
        java_sources = [*files, *([CALLERS/'JahmmShowcase.java'] if args.corpus == 'jahmm' else [])]
        provenance['commands']['java_build'] = run(['javac','-d',classes,*java_sources],out,'java-build')
        for name, command in [('java-version',['java','-version']), ('rust-version',['rustc','--version']), ('cpu',['lscpu'])]:
            provenance['commands'][name] = run(command,out,name)
        rust = target / 'release/examples/showcase'; provenance['rust_executable_sha256'] = sha(rust)
        if args.corpus == 'varscan':
            fixture = (REPO / 'tests/fixtures/varscan-mpileup2snp.pileup').read_text().splitlines()
            assert len(fixture) == 4
            workload = out / 'workload.pileup'
            with workload.open('w') as output:
                for r in range(args.positions//4):
                    for i,line in enumerate(fixture):
                        columns=line.split('\t'); columns[1]=str(r*4+i+1); output.write('\t'.join(columns)+'\n')
            common = ['mpileup2snp',str(workload),'--min-coverage','1','--min-reads2','1','--min-var-freq','0.01','--p-value','1','--strand-filter','0']
            java = ['java','-cp',str(classes),'net.sf.varscan.VarScan',*common]
            rust_command = [str(rust),*common]; provenance['workload_sha256'] = sha(workload)
        else:
            common = list(map(str,(args.states,args.length,args.sequences)))
            java = ['java','-cp',str(classes),'JahmmShowcase',*common]
            rust_command = [str(rust),*common]
            # Same small model/sequence construction; only entrypoints invoke
            # library algorithms. This smoke check is outside measured runs.
            smoke_args = ['3','12','2']
            run([*java[:-3],*smoke_args],out,'java-smoke')
            run([str(rust),*smoke_args],out,'rust-smoke')
            results['smoke_parity'] = compare('jahmm',out/'java-smoke.stdout',out/'rust-smoke.stdout',12,2)
        results['java'] = run(java,out,'java',timed=True)
        results['rust'] = run(rust_command,out,'rust',timed=True)
        results['parity'] = compare(args.corpus,out/'java.stdout',out/'rust.stdout',args.length,args.sequences)
        results['parity']['stderr_byte_identical'] = (out/'java.stderr').read_bytes() == (out/'rust.stderr').read_bytes()
        if args.java_heap != 'none':
            results['java_capped'] = run(['java','-Xms32m','-Xmx'+args.java_heap,*java[1:]],out,'java-capped',timed=True)
            results['capped_parity'] = compare(args.corpus,out/'java.stdout',out/'java-capped.stdout',args.length,args.sequences)
            results['capped_parity']['stderr_byte_identical'] = (out/'java.stderr').read_bytes() == (out/'java-capped.stderr').read_bytes()
        for name, digest in frozen.items():
            if sha(Path(name)) != digest: raise RuntimeError(f'Input changed during benchmark: {name}')
        provenance['status'] = 'completed'
        print('parity:', {k:v for k,v in results['parity'].items() if k != 'likelihood_details'},flush=True)
    except Exception as error:
        provenance['status'] = 'failed'; provenance['error'] = str(error)
        raise
    finally:
        (out/'results.json').write_text(json.dumps(results,indent=2,allow_nan=False)+'\n')
        (out/'provenance.json').write_text(json.dumps(provenance,indent=2,allow_nan=False)+'\n')
    return 0 if results['parity']['parity_passed'] else 2

if __name__ == '__main__':
    sys.exit(main())
