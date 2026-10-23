#!/usr/bin/env python3
"""Independent geometric checks on production-call oracle output.

Inputs are QuaternionOracle.java/.rs outputs. No interpolation algorithm is
reimplemented: check unit length, the great-circle plane and angular position
of spherical-path observations against high-precision scalar geometry.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct

import mpmath as mp

mp.mp.dps = 80
LIMIT = mp.mpf("1e-14")
TIMES = [mp.mpf(t) for t in ("-0.5", "0", "0", "0.25", "0.5", "0.75", "1", "1.5")]
PAIRS = [((1, 0, 0, 0), (0, 1, 0, 0)),
         ((1, 0, 0, 0), (1, mp.mpf("0.00001"), 0, 0)),
         ((0, 1, 0, 0), (0, -1, 0, 0)),
         ((1, 2, 3, 4), (-2, 3, -4, 5))]


def dot(left, right):
    return sum(a * b for a, b in zip(left, right))


def unit(value):
    norm = mp.sqrt(dot(value, value))
    return [x / norm for x in value]


def audit(path):
    raw = path.read_bytes()
    rows = {}
    for line in raw.decode().splitlines():
        match = re.fullmatch(r"slerp\.([0-3])\.([0-7])\.(apply|callback|repeat)=([0-9a-f,]+)", line)
        if match:
            key = (int(match[1]), int(match[2]), match[3])
            if key in rows:
                raise ValueError(f"duplicate interpolation observation: {key}")
            bits = match[4].split(",")
            if len(bits) != 4 or any(not re.fullmatch(r"[0-9a-f]{1,16}", b) for b in bits):
                raise ValueError(f"invalid quaternion components: {key}")
            rows[key] = bits
    if len(rows) != 96 or not raw.decode().endswith("oracle=ok\n"):
        raise ValueError("incomplete interpolation observations or missing footer")
    maxima = dict(unit_length=mp.mpf(0), plane=mp.mpf(0), angular_position=mp.mpf(0))
    spherical_observations = 0
    for (pair, index, mode), bits in rows.items():
        if bits != rows[pair, index, "apply"]:
            raise ValueError(f"callback/repeated-state divergence: {(pair, index, mode)}")
        q = [mp.mpf(struct.unpack(">d", bytes.fromhex(b.zfill(16)))[0]) for b in bits]
        if not all(mp.isfinite(x) for x in q) or q[0] < 0:
            raise ValueError(f"non-finite or negative-polar result: {(pair, index, mode)}")
        maxima["unit_length"] = max(maxima["unit_length"], abs(dot(q, q) - 1))
        if pair not in (0, 3):
            continue  # Linear/identical paths do not promise constant angular velocity.
        start, end = [unit(v) for v in PAIRS[pair]]
        if start[0] < 0:
            start = [-x for x in start]
        if dot(start, end) < 0:
            end = [-x for x in end]
        cosine = dot(start, end)
        normal = unit([b - cosine * a for a, b in zip(start, end)])
        a, b = dot(q, start), dot(q, normal)
        plane_error = mp.sqrt(sum((x - a * s - b * n) ** 2 for x, s, n in zip(q, start, normal)))
        phase = TIMES[index] * mp.acos(cosine)
        # Quaternion sign represents the same rotation: use a sign-invariant
        # angular residual, not component-wise reconstruction of Slerp.
        phase_error = abs(a * mp.sin(phase) - b * mp.cos(phase))
        maxima["plane"] = max(maxima["plane"], plane_error)
        maxima["angular_position"] = max(maxima["angular_position"], phase_error)
        spherical_observations += 1
    if any(value > LIMIT for value in maxima.values()):
        raise ValueError(f"geometric accuracy failure: {maxima}")
    return {"input_sha256": hashlib.sha256(raw).hexdigest(), "observations": len(rows),
            "spherical_observations": spherical_observations,
            "maximum_residuals": {k: mp.nstr(v, 18) for k, v in maxima.items()}}


def compare_rows(expected, actual):
    """Classify strict differences; acceptance ALSO requires both geometric audits.

    Only finite, non-endpoint spherical interpolation may differ numerically.
    Linear paths, exact zeros, exceptions and every other observation stay exact.
    """
    if len(expected) != len(actual):
        raise ValueError("observation count differs")
    differences = []
    seen = set()
    for java, rust in zip(expected, actual):
        key, separator, value = java.partition("=")
        rust_key, rust_separator, rust_value = rust.partition("=")
        if not separator or not rust_separator or key != rust_key or key in seen:
            raise ValueError("missing, reordered or duplicate observation key")
        seen.add(key)
        if value == rust_value:
            continue
        if not re.fullmatch(r"slerp\.[03]\.[03457]\.(apply|callback|repeat)", key):
            raise ValueError(f"strict observation mismatch: {key}")
        left, right = value.split(","), rust_value.split(",")
        if len(left) != 4 or len(right) != 4:
            raise ValueError(f"invalid numerical observation: {key}")
        for a, b in zip(left, right):
            if not all(re.fullmatch(r"[0-9a-f]{1,16}", bits) for bits in (a, b)):
                raise ValueError(f"invalid component: {key}")
            x, y = [mp.mpf(struct.unpack(">d", bytes.fromhex(bits.zfill(16)))[0]) for bits in (a, b)]
            if not mp.isfinite(x) or not mp.isfinite(y):
                raise ValueError(f"nonfinite component differs: {key}")
            if (x == 0 or y == 0) and a != b:
                raise ValueError(f"exact zero differs: {key}")
        differences.append({"key": key, "java": value, "rust": rust_value})
    return differences


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("outputs", nargs="+", type=Path)
    parser.add_argument("--compare", action="store_true",
                        help="require complete Java/Rust compatibility, allowing only independently audited finite spherical rounding; Java output first")
    args = parser.parse_args()
    if args.compare and len(args.outputs) != 2:
        parser.error("--compare requires Java output followed by Rust output")
    callers = Path(__file__).resolve().parent.parent / "tests/corpus/numbers"
    reports = {str(path): audit(path) for path in args.outputs}
    comparison = {}
    if args.compare:
        expected, actual = [path.read_text().splitlines() for path in args.outputs]
        if len(expected) != 4532:
            raise ValueError("expected the complete 4532-line production-call oracle")
        differences = compare_rows(expected, actual)
        comparison = {"output_lines": len(expected), "strict_difference_count": len(differences),
                      "strict_differences": differences,
                      "exceptions_and_noninterpolation_observations_exact": True}
    print(json.dumps({"policy": "quaternion-geometric-compatibility-v1" if args.compare else "geometric-diagnostic-v1", "mpmath_version": mp.__version__,
                      "decimal_precision": mp.mp.dps, "residual_limit": str(LIMIT),
                      "strict_comparator_changed": False,
                      "caller_sha256": {name: hashlib.sha256((callers / name).read_bytes()).hexdigest()
                                        for name in ("QuaternionOracle.java", "QuaternionOracle.rs")},
                      "results": reports, **comparison}, indent=2))


if __name__ == "__main__":
    main()
