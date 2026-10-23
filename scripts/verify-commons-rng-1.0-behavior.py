#!/usr/bin/env python3
"""Compare untouched generated corpus code with independent pinned Java callers.

No generated source is edited: cargo builds the supplied crate and rustc links
the repository's test-only driver against its library artifact. Floating-point
values must have identical IEEE bits by default; decimal formatting alone may
differ. The explicit arithmetic NaN-sign portability mode retains strict
differences and permits only annotated same-payload NaN sign flips.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import subprocess

ORACLES = {
    "numbers-angle": {
        "suffix": "-numbers-angle", "driver": "AngleOracle.rs", "lines": 4642,
        "driver_directory": "numbers", "output_env": "NUMBERS_ORACLE_OUT",
        "java_runner": "run-commons-numbers-1.3-angle-java-oracle.sh",
        "java_directory": ".tmp/commons-numbers-1.3-angle-java-oracle",
        "timeout": 40, "report_all_differences": True,
    },
    "numbers-angle-contracts": {
        "suffix": "-numbers-angle-contracts", "driver": "AngleContractsOracle.rs", "lines": 2,
        "driver_directory": "numbers", "output_env": "NUMBERS_ORACLE_OUT",
        "java_runner": "run-commons-numbers-1.3-angle-contracts-java-oracle.sh",
        "java_directory": ".tmp/commons-numbers-1.3-angle-contracts-java-oracle",
        "timeout": 40,
    },
    "numbers-quaternion-contracts": {
        "suffix": "-numbers-quaternion-contracts", "driver": "QuaternionContractsOracle.rs", "lines": 2,
        "driver_directory": "numbers", "output_env": "NUMBERS_ORACLE_OUT",
        "java_runner": "run-commons-numbers-1.3-quaternion-contracts-java-oracle.sh",
        "java_directory": ".tmp/commons-numbers-1.3-quaternion-contracts-java-oracle",
        "timeout": 40,
    },
    "numbers-quaternion": {
        "suffix": "-numbers-quaternion", "driver": "QuaternionOracle.rs", "lines": 4532,
        "driver_directory": "numbers", "output_env": "NUMBERS_ORACLE_OUT",
        "java_runner": "run-commons-numbers-1.3-quaternion-java-oracle.sh",
        "java_directory": ".tmp/commons-numbers-1.3-quaternion-java-oracle",
        "timeout": 40, "report_all_differences": True,
    },
    "numbers-complex-reference": {
        "suffix": "-numbers-complex-reference", "driver": "ComplexReferenceOracle.rs", "lines": None,
        "driver_directory": "numbers", "output_env": "NUMBERS_ORACLE_OUT",
        "java_runner": "run-commons-numbers-1.3-complex-reference-java-oracle.sh",
        "java_directory": ".tmp/commons-numbers-1.3-complex-reference-java-oracle",
        "timeout": 40, "report_all_differences": True, "reference_accuracy": True,
    },
    "numbers-complex-contracts": {
        "suffix": "-numbers-complex-contracts", "driver": "ComplexContractsOracle.rs", "lines": 2,
        "driver_directory": "numbers", "output_env": "NUMBERS_ORACLE_OUT",
        "java_runner": "run-commons-numbers-1.3-complex-contracts-java-oracle.sh",
        "java_directory": ".tmp/commons-numbers-1.3-complex-contracts-java-oracle",
        "timeout": 40,
    },
    "numbers-complex": {
        "suffix": "-numbers-complex", "driver": "ComplexOracle.rs", "lines": 14617,
        "driver_directory": "numbers", "output_env": "NUMBERS_ORACLE_OUT",
        "java_runner": "run-commons-numbers-1.3-complex-java-oracle.sh",
        "java_directory": ".tmp/commons-numbers-1.3-complex-java-oracle",
        "timeout": 40, "report_all_differences": True,
    },
    "numbers-core-contracts": {
        "suffix": "-numbers-core-contracts", "driver": "CoreContractsOracle.rs", "lines": 2,
        "driver_directory": "numbers", "output_env": "NUMBERS_ORACLE_OUT",
        "java_runner": "run-commons-numbers-1.3-core-contracts-java-oracle.sh",
        "java_directory": ".tmp/commons-numbers-1.3-core-contracts-java-oracle",
        "rust_dependencies": ["java_math_runtime"], "timeout": 40,
    },
    "numbers-core": {
        "suffix": "-numbers-core", "driver": "CoreOracle.rs", "lines": 7888,
        "driver_directory": "numbers", "output_env": "NUMBERS_ORACLE_OUT",
        "java_runner": "run-commons-numbers-1.3-core-java-oracle.sh",
        "java_directory": ".tmp/commons-numbers-1.3-core-java-oracle",
        "rust_dependencies": ["java_math_runtime"], "timeout": 40,
        "report_all_differences": True,
    },
    "numbers-rootfinder": {
        "suffix": "-numbers-rootfinder", "driver": "RootFinderOracle.rs", "lines": 122,
        "driver_directory": "numbers", "output_env": "NUMBERS_ORACLE_OUT",
        "java_runner": "run-commons-numbers-1.3-rootfinder-java-oracle.sh",
        "java_directory": ".tmp/commons-numbers-1.3-rootfinder-java-oracle",
    },
    "numbers-primes": {
        "suffix": "-numbers-primes", "driver": "PrimesOracle.rs", "lines": 3594,
        "driver_directory": "numbers", "output_env": "NUMBERS_ORACLE_OUT",
        "java_runner": "run-commons-numbers-1.3-primes-java-oracle.sh",
        "java_directory": ".tmp/commons-numbers-1.3-primes-java-oracle",
    },
    "source64": {
        "suffix": "", "driver": "GeneratorOracle.rs", "lines": 2865,
        "java_runner": "run-commons-rng-1.0-java-oracle.sh",
        "java_directory": ".tmp/commons-rng-1.0-java-oracle",
    },
    "native": {
        "suffix": "-native", "driver": "NativeGeneratorOracle.rs", "lines": 12601,
        "java_runner": "run-commons-rng-1.0-native-java-oracle.sh",
        "java_directory": ".tmp/commons-rng-1.0-native-java-oracle",
    },
    "sampling-collections": {
        "suffix": "-sampling-collections", "driver": "SamplingCollectionsOracle.rs", "lines": 162,
        "java_runner": "run-commons-rng-1.0-sampling-collections-java-oracle.sh",
        "java_directory": ".tmp/commons-rng-1.0-sampling-collections-java-oracle",
    },
    "sampling-distributions": {
        "suffix": "-sampling-distributions", "driver": "SamplingDistributionsOracle.rs", "lines": 2172,
        "java_runner": "run-commons-rng-1.0-sampling-distributions-java-oracle.sh",
        "java_directory": ".tmp/commons-rng-1.0-sampling-distributions-java-oracle",
    },
}


ARITHMETIC_BITS = re.compile(r"f(32|64)-arith\(([0-9a-f]+)\)")
ARITHMETIC_PREFIXES = ("f32-arith(", "f64-arith(")


def arithmetic_components(value):
    """Decode explicit caller annotations, never infer floats from hex/key names."""
    components, surrounding, position = [], [], 0
    for match in ARITHMETIC_BITS.finditer(value):
        width, encoding = int(match[1]), match[2]
        if len(encoding) != width // 4:
            raise ValueError(f"noncanonical arithmetic bit annotation: {match[0]!r}")
        surrounding.append(value[position:match.start()])
        components.append((width, encoding))
        position = match.end()
    surrounding.append(value[position:])
    if any(prefix in chunk for chunk in surrounding for prefix in ARITHMETIC_PREFIXES):
        raise ValueError(f"malformed arithmetic bit annotation: {value!r}")
    return components, surrounding


def arithmetic_value_matches(java, rust):
    # Exceptions, displays and raw-bit observations are not annotation streams.
    if not java.startswith(ARITHMETIC_PREFIXES) or not rust.startswith(ARITHMETIC_PREFIXES):
        return java == rust, []
    expected, expected_text = arithmetic_components(java)
    actual, actual_text = arithmetic_components(rust)
    if expected_text != actual_text or len(expected) != len(actual):
        return False, []
    permitted = []
    for component, ((width, encoding), (rust_width, rust_encoding)) in enumerate(zip(expected, actual)):
        if width != rust_width:
            return False, []
        if encoding == rust_encoding:
            continue
        bits, rust_bits = int(encoding, 16), int(rust_encoding, 16)
        fraction_width = 23 if width == 32 else 52
        fraction_mask = (1 << fraction_width) - 1
        exponent_mask = ((1 << (width - fraction_width - 1)) - 1) << fraction_width
        is_nan = lambda value: value & exponent_mask == exponent_mask and value & fraction_mask != 0
        if not (is_nan(bits) and is_nan(rust_bits) and bits ^ rust_bits == 1 << (width - 1)):
            return False, []
        permitted.append({"component": component, "width": width,
                          "java_bits": encoding, "rust_bits": rust_encoding})
    return True, permitted


def compare_outputs(java_text, rust_text, differences=None, *, arithmetic_nan_sign=False, permitted=None):
    expected = java_text.splitlines()
    actual = rust_text.splitlines()
    if not expected or expected[-1] != "oracle=ok":
        raise ValueError("Java oracle did not complete")
    if len(expected) != len(actual):
        raise ValueError(f"output length mismatch: Java={len(expected)}, Rust={len(actual)}")
    for line, (java, rust) in enumerate(zip(expected, actual), 1):
        key, separator, value = java.partition("=")
        rust_key, rust_separator, rust_value = rust.partition("=")
        if (key, separator) != (rust_key, rust_separator):
            detail = f"line {line}: Java {java!r}, Rust {rust!r}"
            if differences is None:
                raise ValueError(detail)
            differences.append(detail)
            continue
        if arithmetic_nan_sign and (value.startswith(ARITHMETIC_PREFIXES) or rust_value.startswith(ARITHMETIC_PREFIXES)):
            matches, allowances = arithmetic_value_matches(value, rust_value)
            if matches and permitted is not None:
                permitted.extend({"line": line, "key": key, **allowance} for allowance in allowances)
        elif ".float." in key or ".double." in key:
            format_code = "!f" if ".float." in key else "!d"
            matches = struct.pack(format_code, float(value)) == struct.pack(format_code, float(rust_value))
        else:
            matches = value == rust_value
        if not matches:
            detail = f"line {line}: Java {java!r}, Rust {rust!r}"
            if differences is None:
                raise ValueError(detail)
            differences.append(detail)
    return len(expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("translation_root", type=Path)
    parser.add_argument("--output-dir", type=Path,
                        help="write a new behavior checkpoint without replacing the artifact's default observations")
    parser.add_argument("--arithmetic-nan-sign-portability", action="store_true",
                        help="explicitly permit only sign-only NaN differences in caller-annotated arithmetic bits; retain strict differences")
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--numbers-angle", action="store_const", const="numbers-angle", dest="oracle",
                       help="check complete Angle operations, exact IEEE values, normalization, identity and vector errors")
    modes.add_argument("--numbers-angle-contracts", action="store_const", const="numbers-angle-contracts", dest="oracle",
                       help="check translated Angle inheritance, identity, callbacks and vector client")
    modes.add_argument("--numbers-quaternion-contracts", action="store_const", const="numbers-quaternion-contracts", dest="oracle",
                       help="check translated Quaternion identity/null/cause and interpolation callback client")
    modes.add_argument("--numbers-quaternion", action="store_const", const="numbers-quaternion", dest="oracle",
                       help="check complete Quaternion operations, IEEE bits, parsing/cause chains and interpolation callbacks")
    modes.add_argument("--numbers-complex-reference", action="store_const", const="numbers-complex-reference", dest="oracle",
                       help="check pinned independent Complex reference data with the upstream operation-specific accuracy bounds")
    modes.add_argument("--numbers-complex-contracts", action="store_const", const="numbers-complex-contracts", dest="oracle",
                       help="check automatically translated Complex workload and null/identity client")
    modes.add_argument("--numbers-complex", action="store_const", const="numbers-complex", dest="oracle",
                       help="check complete Complex operations, IEEE bits, identity, roots and parsing")
    modes.add_argument("--numbers-core-contracts", action="store_const", const="numbers-core-contracts", dest="oracle",
                       help="check automatically translated Core interface-default and null-boundary client")
    modes.add_argument("--numbers-core", action="store_const", const="numbers-core", dest="oracle",
                       help="check Core production operations, exact bits, state and exception observations")
    modes.add_argument("--numbers-rootfinder", action="store_const", const="numbers-rootfinder", dest="oracle",
                       help="check complete Commons Numbers Root Finder, exact IEEE roots and callback traces")
    modes.add_argument("--numbers-primes", action="store_const", const="numbers-primes", dest="oracle",
                       help="check complete Commons Numbers Primes against its independent Java caller")
    modes.add_argument("--native", action="store_const", const="native", dest="oracle",
                       help="check all ten native source32 generators")
    modes.add_argument("--sampling-collections", action="store_const", const="sampling-collections",
                       dest="oracle", help="check permutation, list and collection sampling")
    modes.add_argument("--sampling-distributions", action="store_const", const="sampling-distributions",
                       dest="oracle", help="check all distribution samplers, callbacks and numerical helpers")
    parser.set_defaults(oracle="source64")
    args = parser.parse_args()
    oracle = ORACLES[args.oracle]
    if oracle.get("reference_accuracy") and args.arithmetic_nan_sign_portability:
        parser.error("reference accuracy and arithmetic NaN-sign portability are distinct policies")
    repository = Path(__file__).resolve().parent.parent
    root = args.translation_root.resolve()
    manifest = root / "generated-crate/Cargo.toml"
    if not manifest.is_file():
        parser.error(f"missing generated manifest: {manifest}")
    output = (args.output_dir.resolve() if args.output_dir is not None
              else root / ("behavior" + oracle["suffix"]))
    output.mkdir(parents=True, exist_ok=True)
    (output / "result.txt").unlink(missing_ok=True)
    for policy_artifact in ["arithmetic-nan-sign-allowances.json", "portable-differences.txt"]:
        (output / policy_artifact).unlink(missing_ok=True)
    driver_directory = oracle.get("driver_directory", "rng")
    driver_path = repository / "tests/corpus" / driver_directory / oracle["driver"]
    java_driver = driver_path.with_suffix(".java")
    caller_sources = [driver_path]
    if java_driver.is_file():
        caller_sources.append(java_driver)
    caller_hashes = {}
    for caller in caller_sources:
        source = caller.read_bytes()
        (output / caller.name).write_bytes(source)
        caller_hashes[str(caller.relative_to(repository))] = hashlib.sha256(source).hexdigest()
    (output / "caller-provenance.json").write_text(json.dumps({
        "oracle": args.oracle,
        "expected_observations": oracle["lines"],
        "comparison_policy": ("pinned-reference-ulp-v1" if oracle.get("reference_accuracy")
                              else "arithmetic-nan-sign-v1" if args.arithmetic_nan_sign_portability else "strict"),
        "translation_root": str(root),
        "caller_sha256": caller_hashes,
    }, indent=2) + "\n")
    with (output / "java-run.log").open("w") as log:
        java_runner = oracle["java_runner"]
        subprocess.run(["bash", str(repository / "scripts" / java_runner)],
                       cwd=repository, stdout=log, stderr=subprocess.STDOUT, check=True)
    java_directory = Path(os.environ.get(oracle.get("output_env", "RNG_ORACLE_OUT"), oracle["java_directory"]))
    if not java_directory.is_absolute():
        java_directory = repository / java_directory
    driver_arguments = []
    reference_requests = None
    expected_count = oracle["lines"]
    if oracle.get("reference_accuracy"):
        import complex_reference
        for artifact in ("requests.tsv", "reference-manifest.json"):
            (output / artifact).write_bytes((java_directory / artifact).read_bytes())
        request_path = output / "requests.tsv"
        reference_requests = complex_reference.load_requests(request_path, output / "reference-manifest.json")
        expected_count = len(reference_requests) + 1
        driver_arguments = [str(request_path)]
        provenance_path = output / "caller-provenance.json"
        provenance = json.loads(provenance_path.read_text())
        provenance["expected_observations"] = expected_count
        provenance["reference_inputs_sha256"] = {name: hashlib.sha256((output / name).read_bytes()).hexdigest()
                                                 for name in ("requests.tsv", "reference-manifest.json")}
        provenance["reference_policy_sha256"] = hashlib.sha256(Path(complex_reference.__file__).read_bytes()).hexdigest()
        provenance["verifier_sha256"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
        provenance_path.write_text(json.dumps(provenance, indent=2) + "\n")
    with (output / "cargo-stderr.log").open("w") as log:
        build = subprocess.run(["cargo", "build", "--offline", "--lib", "--manifest-path", str(manifest),
                                "--message-format=json"], cwd=repository,
                               stdout=subprocess.PIPE, stderr=log, text=True)
    (output / "cargo-artifacts.jsonl").write_text(build.stdout)
    build.check_returncode()
    libraries = []
    dependencies = {name: set() for name in oracle.get("rust_dependencies", [])}
    for line in build.stdout.splitlines():
        event = json.loads(line)
        if event.get("reason") == "compiler-artifact" and event["target"]["name"] == "new_java2rust_focused_output":
            libraries.extend(Path(name) for name in event["filenames"] if name.endswith(".rlib"))
        if event.get("reason") == "compiler-artifact" and event["target"]["name"] in dependencies:
            dependencies[event["target"]["name"]].update(
                name for name in event["filenames"] if name.endswith(".rlib"))
    if len(libraries) != 1:
        raise RuntimeError(f"expected one generated library artifact, got {libraries}")
    library = libraries[0]
    externs = []
    for name, artifacts in dependencies.items():
        if len(artifacts) != 1:
            raise RuntimeError(f"expected one compiled dependency artifact for {name}, got {artifacts}")
        externs.extend(["--extern", f"{name}={next(iter(artifacts))}"])
    with (output / "driver-build.log").open("w") as log:
        driver = oracle["driver"]
        driver_directory = oracle.get("driver_directory", "rng")
        subprocess.run(["rustc", "--edition=2021", str(repository / "tests/corpus" / driver_directory / driver),
                        "--extern", f"translated={library}", "-L", f"dependency={library.parent}",
                        "-L", f"dependency={library.parent / 'deps'}",
                        *externs,
                        "-o", str(output / "generator-oracle")],
                       cwd=repository, stdout=log, stderr=subprocess.STDOUT, check=True)
    with (output / "rust-output.txt").open("w") as stdout, (output / "rust-stderr.log").open("w") as stderr:
        subprocess.run([str(output / "generator-oracle"), *driver_arguments], stdout=stdout, stderr=stderr, check=True,
                       timeout=oracle.get("timeout"))
    java_output = repository / oracle["java_directory"] / "output.txt"
    # Honor the same configurable Java output directory as the Java runner.
    output_env = oracle.get("output_env", "RNG_ORACLE_OUT")
    if os.environ.get(output_env):
        java_output = Path(os.environ[output_env])
        if not java_output.is_absolute():
            java_output = repository / java_output
        java_output /= "output.txt"
    reference = java_output.read_text()
    (output / "java-output.txt").write_text(reference)
    rust_text = (output / "rust-output.txt").read_text()
    differences = [] if oracle.get("report_all_differences") or args.arithmetic_nan_sign_portability else None
    count = compare_outputs(reference, rust_text, differences)
    if count != expected_count:
        raise ValueError(f"unexpected reference scope: expected {expected_count} output lines, got {count}")
    if differences:
        (output / "differences.txt").write_text("\n".join(differences) + "\n")
    else:
        (output / "differences.txt").unlink(missing_ok=True)
    if reference_requests is not None:
        failures, deviations = complex_reference.validate_outputs(reference_requests, reference, rust_text)
        (output / "reference-failures.json").write_text(json.dumps(failures, indent=2) + "\n")
        (output / "reference-deviations.json").write_text(json.dumps(deviations, indent=2) + "\n")
        if failures:
            raise ValueError(f"{len(failures)} independent-reference component failures; see {output / 'reference-failures.json'}")
        result = (f"Java and generated Rust meet pinned independent-reference bounds: {count - 1} requests; "
                  f"{len(differences)} strict backend differences retained; signs, zeros and nonfinite classes preserved\n")
        (output / "result.txt").write_text(result)
        print(result, end="")
        return
    if args.arithmetic_nan_sign_portability:
        residual, permitted = [], []
        compare_outputs(reference, rust_text, residual, arithmetic_nan_sign=True, permitted=permitted)
        (output / "arithmetic-nan-sign-allowances.json").write_text(json.dumps(permitted, indent=2) + "\n")
        (output / "portable-differences.txt").write_text("\n".join(residual) + ("\n" if residual else ""))
        if residual:
            raise ValueError(f"{len(residual)} Java-contract mismatches; see {output / 'portable-differences.txt'}\n"
                             + "\n".join(residual[:10]))
        result = (f"Java/generated Rust contract comparison matches: {count} lines; "
                  f"arithmetic-nan-sign-v1 permits {len(permitted)} NaN sign components; "
                  f"{len(differences)} strict observation differences retained; all other observations exact\n")
        (output / "result.txt").write_text(result)
        print(result, end="")
        return
    if differences:
        raise ValueError(f"{len(differences)} exact observation mismatches; see {output / 'differences.txt'}\n"
                         + "\n".join(differences[:10]))
    detail = ("translated Quaternion identity/null/cause and interpolation callback contracts" if args.oracle == "numbers-quaternion-contracts"
              else "translated client identity/null contracts and finite numerical workload checks" if args.oracle == "numbers-complex-contracts"
              else "translated interface-default state and null-boundary contracts" if args.oracle == "numbers-core-contracts"
              else "exact IEEE roots, callback traces/failures and validation messages" if args.oracle == "numbers-rootfinder"
              else "exact discrete outputs, helper mutations and exception messages" if args.oracle == "numbers-primes"
              else "exact sampling outputs and RNG traces" if args.oracle == "sampling-collections"
              else "floating-point bits identical")
    result = f"Java/generated Rust behavior matches: {count} lines; {detail}\n"
    (output / "result.txt").write_text(result)
    print(result, end="")


if __name__ == "__main__":
    main()
