#!/usr/bin/env python3
"""Compare untouched generated RNG code with the independent pinned Java oracle.

No generated source is edited: cargo builds the supplied crate and rustc links
the repository's test-only driver against its library artifact. Floating-point
values must have identical IEEE bits; decimal formatting alone may differ.
"""
import argparse
import json
import os
from pathlib import Path
import struct
import subprocess


def compare_outputs(java_text, rust_text):
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
            raise ValueError(f"line {line}: Java {java!r}, Rust {rust!r}")
        if ".float." in key or ".double." in key:
            format_code = "!f" if ".float." in key else "!d"
            matches = struct.pack(format_code, float(value)) == struct.pack(format_code, float(rust_value))
        else:
            matches = value == rust_value
        if not matches:
            raise ValueError(f"line {line}: Java {java!r}, Rust {rust!r}")
    return len(expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("translation_root", type=Path)
    args = parser.parse_args()
    repository = Path(__file__).resolve().parent.parent
    root = args.translation_root.resolve()
    manifest = root / "generated-crate/Cargo.toml"
    if not manifest.is_file():
        parser.error(f"missing generated manifest: {manifest}")
    output = root / "behavior"
    output.mkdir(exist_ok=True)
    (output / "result.txt").unlink(missing_ok=True)
    with (output / "java-run.log").open("w") as log:
        subprocess.run(["bash", str(repository / "scripts/run-commons-rng-1.0-java-oracle.sh")],
                       cwd=repository, stdout=log, stderr=subprocess.STDOUT, check=True)
    with (output / "cargo-stderr.log").open("w") as log:
        build = subprocess.run(["cargo", "build", "--offline", "--lib", "--manifest-path", str(manifest),
                                "--message-format=json"], cwd=repository,
                               stdout=subprocess.PIPE, stderr=log, text=True)
    (output / "cargo-artifacts.jsonl").write_text(build.stdout)
    build.check_returncode()
    libraries = []
    for line in build.stdout.splitlines():
        event = json.loads(line)
        if event.get("reason") == "compiler-artifact" and event["target"]["name"] == "new_java2rust_focused_output":
            libraries.extend(Path(name) for name in event["filenames"] if name.endswith(".rlib"))
    if len(libraries) != 1:
        raise RuntimeError(f"expected one generated library artifact, got {libraries}")
    library = libraries[0]
    with (output / "driver-build.log").open("w") as log:
        subprocess.run(["rustc", "--edition=2021", str(repository / "tests/corpus/rng/GeneratorOracle.rs"),
                        "--extern", f"translated={library}", "-L", f"dependency={library.parent}",
                        "-o", str(output / "generator-oracle")],
                       cwd=repository, stdout=log, stderr=subprocess.STDOUT, check=True)
    with (output / "rust-output.txt").open("w") as stdout, (output / "rust-stderr.log").open("w") as stderr:
        subprocess.run([str(output / "generator-oracle")], stdout=stdout, stderr=stderr, check=True)
    java_output = repository / ".tmp/commons-rng-1.0-java-oracle/output.txt"
    # Honor the same configurable Java output directory as the Java runner.
    if os.environ.get("RNG_ORACLE_OUT"):
        java_output = Path(os.environ["RNG_ORACLE_OUT"])
        if not java_output.is_absolute():
            java_output = repository / java_output
        java_output /= "output.txt"
    reference = java_output.read_text()
    (output / "java-output.txt").write_text(reference)
    count = compare_outputs(reference, (output / "rust-output.txt").read_text())
    if count != 2865:
        raise ValueError(f"unexpected reference scope: expected 2865 output lines, got {count}")
    result = f"Java/generated Rust behavior matches: {count} lines; floating-point bits identical\n"
    (output / "result.txt").write_text(result)
    print(result, end="")


if __name__ == "__main__":
    main()
