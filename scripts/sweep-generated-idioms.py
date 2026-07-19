#!/usr/bin/env python3
"""Rank generated Rust idiom signals for fixture triage.

This script intentionally does not decide pass/fail. It scans generated Rust,
counts regex-defined signals, maps examples back to Java source when a
`status.tsv` from `translate-focused-package.sh` is available, and reports the
generalized fixture/test area that should own each pattern.
"""

from __future__ import annotations

import argparse
import csv
import re
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Signal:
    key: str
    label: str
    regex: str
    owner: str
    fixtures: str


SIGNALS = [
    Signal(
        "runtime_downcast",
        'Runtime `downcast::<T>().expect("invalid Java cast")`',
        r'downcast::<[^>]+>\(\)\.expect\("invalid Java cast"\)',
        "COERCE-1 / representation dispatch",
        "TypedGetterAvoidsDowncast.java; PrimitiveGetterDispatchCoercion.java",
    ),
    Signal(
        "concrete_none",
        "`None` likely flowing into concrete storage",
        r"=\s*None;|return\s+None;|\bNone\s*==|==\s*None\b",
        "NULL-2 / slot optionality",
        "NullableParameterAndLocal.java; NullableLocalReturn.java",
    ),
    Signal(
        "java_collection_api",
        "Java collection method spelling on Rust containers",
        r"\.(add|add_all|size|put|contains_key|entry_set)\(",
        "CALL-1 / native collection call planning",
        "CollectionAddAllAndSize.java; CollectionsUse.java; MapPutGetJavaApi.java",
    ),
    Signal(
        "stream_placeholder",
        "Stream/lambda/method-reference placeholder or Java collector chain",
        r"unsupported Java expression: (lambda|method reference)|\.collect\(collectors\.|\.stream\(",
        "STREAM-2 / stream planning",
        "MethodReferenceFlatMap.java; StreamFlatMapLambda.java; ForEachLambdaSideEffect.java",
    ),
    Signal(
        "try_placeholder",
        "Unsupported try/catch/finally placeholder",
        r"unsupported Java statement: try/catch/finally",
        "EXC-2 / EXC-3 exception planning",
        "SimpleTryCatchReturn.java; SimpleTryFinallyCleanup.java; TryWithResourcesCopy.java",
    ),
    Signal(
        "array_placeholder",
        "Java array creation placeholder",
        r"Java array creation|unsupported Java array creation",
        "ARRAY-1 / array shape planning",
        "PrimitiveArrayWithComputedLength.java; StringArrayLiteralArgument.java",
    ),
    Signal(
        "switch_state_machine",
        "Synthetic switch state-machine scaffolding",
        r"__java_switch_|__java_case_|__java_fallthrough",
        "CTRL-2 / switch planning",
        "SimpleReturnSwitches.java; SwitchReturnsEnumMatch.java; SwitchFallthrough.java",
    ),
    Signal(
        "super_constructor",
        "`super(...)` constructor placeholder",
        r"super\(\.\.\.\) requires inheritance lowering",
        "INIT-1 / object representation and inherited initialization",
        "SuperConstructorState.java; InheritanceAndSuper.java",
    ),
    Signal(
        "unsupported_constructor_delegation",
        "Unsupported constructor delegation placeholder",
        r"unsupported Java constructor: (unresolved constructor delegation|this\(\.\.\.\).*)",
        "INIT-1 / constructor initialization planning",
        "ConstructorDelegationWithExtraWork.java; UnresolvedConstructorDelegation.java",
    ),
    Signal(
        "acronym_identifier",
        "Acronym-split identifier names",
        r"\b[a-z](?:_[a-z]){3,}\b",
        "identifier hygiene",
        "AcronymIdentifierNames.java",
    ),
    Signal(
        "class_literal_placeholder",
        "Class literal placeholder or weak Java class token use",
        r"unsupported Java class literal|JavaClass::<\(\)>",
        "CALL-1 / class literal token planning",
        "ClassLiteralToken.java; ClassLiteralRegistration.java",
    ),
    Signal(
        "instanceof_placeholder",
        "Unsupported instanceof/cast shape",
        r"unsupported Java instanceof|unsupported Java cast",
        "representation dispatch and narrowing",
        "InstanceofGuardNarrowsCast.java",
    ),
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--generated-root", required=True, type=Path)
    parser.add_argument("--status", type=Path)
    parser.add_argument("--out-md", type=Path)
    parser.add_argument("--out-tsv", type=Path)
    parser.add_argument("--top", type=int, default=30)
    return parser.parse_args()


def status_source_map(path: Path | None) -> dict[Path, str]:
    if path is None or not path.exists():
        return {}
    out: dict[Path, str] = {}
    with path.open(newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        for row in reader:
            rust_path = row.get("rust_path")
            source = row.get("source")
            if rust_path and source:
                out[Path(rust_path).resolve()] = source
    return out


def rust_files(root: Path) -> list[Path]:
    return sorted(path for path in root.rglob("*.rs") if path.is_file())


def count_signal(signal: Signal, files: list[Path], source_map: dict[Path, str]) -> dict[str, object]:
    pattern = re.compile(signal.regex)
    occurrences = 0
    matched_files = 0
    examples: list[str] = []
    for path in files:
        text = path.read_text(errors="replace")
        matches = list(pattern.finditer(text))
        if not matches:
            continue
        occurrences += len(matches)
        matched_files += 1
        if len(examples) < 5:
            source = source_map.get(path.resolve(), str(path))
            examples.append(source)
    return {
        "signal": signal,
        "occurrences": occurrences,
        "files": matched_files,
        "examples": examples,
    }


def write_tsv(path: Path, rows: list[dict[str, object]]) -> None:
    with path.open("w", newline="") as handle:
        writer = csv.writer(handle, delimiter="\t")
        writer.writerow(["signal", "occurrences", "files", "owner", "fixtures", "examples"])
        for row in rows:
            signal = row["signal"]
            assert isinstance(signal, Signal)
            writer.writerow(
                [
                    signal.key,
                    row["occurrences"],
                    row["files"],
                    signal.owner,
                    signal.fixtures,
                    "; ".join(row["examples"]),
                ]
            )


def markdown(rows: list[dict[str, object]], generated_root: Path, status: Path | None) -> str:
    lines = [
        "# Generated Rust Idiom Sweep",
        "",
        "This is a triage report, not a pass/fail oracle. Counts rank patterns",
        "that should become or update generalized fixtures before implementation",
        "work is considered representative of the corpus.",
        "",
        f"Generated root: `{generated_root}`",
    ]
    if status is not None:
        lines.append(f"Status map: `{status}`")
    lines.extend(
        [
            "",
            "| Occurrences | Files | Signal | Owning Layer | Generalized Fixture Coverage | Examples |",
            "|---:|---:|---|---|---|---|",
        ]
    )
    for row in rows:
        signal = row["signal"]
        assert isinstance(signal, Signal)
        examples = "<br>".join(f"`{example}`" for example in row["examples"])
        lines.append(
            f"| {row['occurrences']} | {row['files']} | {signal.label} | "
            f"{signal.owner} | {signal.fixtures} | {examples} |"
        )
    lines.extend(
        [
            "",
            "## Use",
            "",
            "1. Pick the highest-count signal that still has weak fixture coverage.",
            "2. Add or adjust a small Java fixture that captures the generalized idiom.",
            "3. Implement the owning planner/lowering layer.",
            "4. Re-run this sweep and the focused compile gate; do not use a lower count",
            "   alone as proof of correctness.",
        ]
    )
    return "\n".join(lines) + "\n"


def main() -> int:
    args = parse_args()
    if not args.generated_root.is_dir():
        raise SystemExit(f"generated root does not exist: {args.generated_root}")
    files = rust_files(args.generated_root)
    source_map = status_source_map(args.status)
    rows = [count_signal(signal, files, source_map) for signal in SIGNALS]
    rows.sort(key=lambda row: (int(row["occurrences"]), int(row["files"])), reverse=True)
    if args.top > 0:
        rows = rows[: args.top]
    if args.out_tsv is not None:
        args.out_tsv.parent.mkdir(parents=True, exist_ok=True)
        write_tsv(args.out_tsv, rows)
    report = markdown(rows, args.generated_root, args.status)
    if args.out_md is not None:
        args.out_md.parent.mkdir(parents=True, exist_ok=True)
        args.out_md.write_text(report)
    else:
        print(report, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
