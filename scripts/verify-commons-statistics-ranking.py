#!/usr/bin/env python3
"""Compare independent Java/Rust callers of the complete Ranking core closure."""

import argparse
import csv
import difflib
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--translation", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parent.parent
    translation = args.translation.resolve()
    out = args.out.resolve()
    if out.exists():
        parser.error("output already exists; select a fresh evidence directory")
    scope = json.loads((translation / "scope/production-scope.json").read_text())
    assert scope["selected"] == ["commons-statistics-ranking"], scope["selected"]
    summary = dict(csv.reader((translation / "summary.tsv").open(), delimiter="\t"))
    for key in (
        "translate_failed_files", "parse_failures", "cargo_check_errors",
        "unsupported_statements", "unsupported_members", "unsupported_expressions",
        "stubbed_constants", "noop_facade_call_sites", "invalid_pattern_files",
    ):
        assert summary[key] == "0", (key, summary[key])
    rows = [row for row in csv.DictReader(
        (translation / "scope/production-sources.tsv").open(), delimiter="\t"
    ) if row["module"] in scope["selected"]]
    assert len(rows) == scope["closures"]["ranking"]["java_files"] == int(summary["files"])
    for row in rows:
        assert digest(row["source"]) == row["sha256"], row["source"]
    emitted = list(csv.DictReader((translation / "batch-manifest.tsv").open(), delimiter="\t"))
    assert sorted(digest(row["source"]) for row in emitted) == sorted(row["sha256"] for row in rows)
    out.mkdir(parents=True)
    (out / "callers").mkdir()
    fixture = repo / "tests/corpus/statistics/ranking-acceptance"
    for name in ("RankingOracle.java", "ranking_oracle.rs"):
        shutil.copyfile(fixture / name, out / "callers" / name)
    crate = translation / "generated-crate"
    inputs = {row["source"]: row["sha256"] for row in rows}
    inputs.update({row["source"]: digest(row["source"]) for row in emitted})
    for path in [*crate.joinpath("src").rglob("*.rs"), crate / "Cargo.toml", crate / "Cargo.lock",
                 *out.joinpath("callers").iterdir(), Path(__file__).resolve()]:
        inputs[str(path)] = digest(path)
    evidence = {"scope": "Ranking core behavior", "translation": str(translation),
                "producer_sha256": summary["translation_producer_sha256"], "pins": scope["pins"],
                "inputs": inputs, "commands": [], "status": "running"}
    provenance = out / "input-provenance.json"
    provenance.write_text(json.dumps(evidence, indent=2) + "\n")
    env = os.environ.copy()
    env.pop("CARGO_MAKEFLAGS", None)
    env.pop("MAKEFLAGS", None)
    env.update(CARGO_INCREMENTAL="0", CARGO_NET_OFFLINE="true")
    env["CARGO_TARGET_DIR"] = str((args.target_dir or repo / ".tmp/statistics-behavior-target").resolve())

    def run(command, name, cwd=repo):
        evidence["commands"].append({"args": list(map(str, command)), "cwd": str(cwd)})
        result = subprocess.run(list(map(str, command)), cwd=cwd, env=env, capture_output=True, timeout=180)
        (out / f"{name}.stdout").write_bytes(result.stdout)
        (out / f"{name}.stderr").write_bytes(result.stderr)
        if result.returncode:
            raise RuntimeError(f"{name} failed ({result.returncode}); see {out / (name + '.stderr')}")
        return result.stdout

    try:
        for program in ("java", "javac", "rustc"):
            run([program, "-version" if program != "rustc" else "--version"], program + "-version")
        classes = out / "classes"
        classes.mkdir()
        run(["javac", "-d", classes, *[row["source"] for row in rows], out / "callers/RankingOracle.java"], "javac")
        java = run(["java", "-cp", classes, "RankingOracle"], "java")
        probe = out / "probe"
        (probe / "src").mkdir(parents=True)
        shutil.copyfile(out / "callers/ranking_oracle.rs", probe / "src/main.rs")
        (probe / "Cargo.toml").write_text(
            '[package]\nname="statistics-ranking-core-probe"\nversion="0.0.0"\nedition="2021"\n'
            '[dependencies]\ntranslated={package="new-java2rust-focused-output",path='
            + json.dumps(str(crate)) + '}\n'
        )
        rust = run(["cargo", "run", "--quiet", "--offline"], "rust", probe)
        evidence["probe_inputs"] = {
            str(path): digest(path) for path in
            (probe / "Cargo.toml", probe / "Cargo.lock", probe / "src/main.rs")
        }
        evidence["cargo_target_dir"] = env["CARGO_TARGET_DIR"]
        (out / "java.tsv").write_bytes(java)
        (out / "rust.tsv").write_bytes(rust)
        diff = "".join(difflib.unified_diff(java.decode().splitlines(True), rust.decode().splitlines(True), "Java", "Rust"))
        (out / "diff.txt").write_text(diff)
        assert java == rust and len(java.splitlines()) == 207, "Ranking behavior differs; inspect diff.txt"
        # Inputs are immutable throughout the run, including staged production.
        for name, expected in inputs.items():
            assert digest(name) == expected, name
        evidence.update(status="passed", exact_rows=207)
        print("207 exact Ranking core behavior rows passed")
    except Exception:
        evidence["status"] = "failed"
        raise
    finally:
        provenance.write_text(json.dumps(evidence, indent=2) + "\n")


if __name__ == "__main__":
    main()
