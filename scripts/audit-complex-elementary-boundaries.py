#!/usr/bin/env python3
"""Diagnostic only: check two traced primitive boundaries against high precision.

Inputs are full independent Complex oracle outputs, not edited/generated code.
Requires mpmath for this development audit; the translator has no such dependency.
This report neither installs a tolerance nor declares corpus acceptance.
"""
import argparse
import json
import math
from pathlib import Path
import struct

import mpmath as mp


def observations(path):
    result = {}
    for line in path.read_text().splitlines():
        key, separator, value = line.partition("=")
        if not separator or key in result:
            raise ValueError(f"malformed or duplicate observation: {line!r}")
        result[key] = value
    if result.get("oracle") != "ok":
        raise ValueError("oracle did not complete")
    return result


def scalar(bits):
    return struct.unpack("!d", int(bits, 16).to_bytes(8, "big"))[0]


def audit(java, rust):
    if list(java) != list(rust):
        raise ValueError("different ordered observation domains")
    # These are actual ordinary production calls in the existing public oracle:
    # Complex.cosh(1,+0i) projects Math.cosh(1); Complex.arg(-2,-3i)
    # directly evaluates Math.atan2(-3,-2). No Complex algorithm is recreated.
    cases = [
        ("Math.cosh(1)", "z.4.cosh", lambda: mp.cosh(1)),
        ("Math.atan2(-3,-2)", "z.19.arg", lambda: mp.atan2(-3, -2)),
    ]
    report = {"audit_only": True, "mpmath_version": mp.__version__, "references": []}
    for precision in (100, 200):
        with mp.workdps(precision):
            for operation, key, reference in cases:
                exact = reference()
                nearest = struct.pack("!d", float(exact)).hex()
                results = {}
                for name, values in (("java", java), ("generated", rust)):
                    bits = values[key].split(",")[0]
                    value = scalar(bits)
                    results[name] = {
                        "bits": f"{int(bits, 16):016x}",
                        "error_in_ulps": mp.nstr(abs(mp.mpf(value) - exact) / mp.mpf(math.ulp(value)), 35),
                    }
                report["references"].append({
                    "operation": operation, "observation": key,
                    "decimal_precision": precision,
                    "reference": mp.nstr(exact, precision - 10),
                    "nearest_binary64": nearest, **results,
                })
    angle_delta = abs(scalar(java["z.19.arg"]) - scalar(rust["z.19.arg"]))
    report["existing_stress_sensitivity"] = {
        "observation": "z.19.scalar.9.pow", "exponent": 1e200,
        "primitive_angle_difference": angle_delta,
        "phase_perturbation_magnitude": angle_delta * 1e200,
        "java": java["z.19.scalar.9.pow"], "generated": rust["z.19.scalar.9.pow"],
        "classification": "ill-conditioned phase diagnostic, not a harmless final-value rounding claim",
    }
    report["strict_difference_count"] = sum(java[key] != rust[key] for key in java)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("java_output", type=Path)
    parser.add_argument("rust_output", type=Path)
    args = parser.parse_args()
    print(json.dumps(audit(observations(args.java_output), observations(args.rust_output)), indent=2))


if __name__ == "__main__":
    main()
