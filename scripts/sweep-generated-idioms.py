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

NOOP_FACADE_METHOD_CALL_SITE_PREFIX = "/* noop Java facade method call: "


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
        "Strong Java collection method spelling on Rust containers",
        r"\.(add_all|size|put|entry_set)\(",
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
        "unsupported_statements",
        "Unsupported Java statement diagnostics",
        r"unsupported Java statement:",
        "parser/lowering unsupported coverage",
        "AssertStatementUnsupported.java; YieldStatementUnsupported.java",
    ),
    Signal(
        "unsupported_members",
        "Unsupported Java member diagnostics",
        r"unsupported Java member:",
        "parser/member unsupported coverage",
        "StaticInitializerUnsupported.java",
    ),
    Signal(
        "unsupported_expressions",
        "Unsupported Java expression diagnostics",
        r"unsupported Java expression:",
        "parser/expression unsupported coverage",
        "SwitchExpressionUnsupported.java",
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
    Signal(
        "unmodelled_types",
        "Opaque Any fallback types",
        r"Box<dyn std::any::Any>",
        "external fallback shape",
        "OpaqueExternalShim.java",
    ),
    Signal(
        "unmodelled_call_sites",
        "Explicit unmodelled facade call sites",
        r"unimplemented!\([^)]*is not modelled",
        "javalibs facade coverage",
        "UnmodelledFacadeCall.java",
    ),
    Signal(
        # Must stay in step with `output_markers::count_noop_facade_methods`.
        # Two independent detectors for one named signal is the drift hazard
        # claude_audits/0018 is about, and these two did disagree: this one
        # matched every `Default::default()` anywhere, including the synthesised
        # upcast conversions below, which were 78 of 80 matches on a program
        # whose only Java API use was `StringBuilder`.
        # `tests/idiom_sweep_script.rs` asserts the two agree.
        "noop_facade_methods",
        "Facade methods returning default placeholders",
        r"^(?!\s*fn from\().*(?:Default::default\(\)|self\.clone\(\)) \}\s*$",
        "javalibs facade coverage",
        "StringBuilderFacade.java",
    ),
    Signal(
        "noop_facade_call_sites",
        "Call sites of no-op facade members",
        r"",
        "javalibs facade coverage",
        "StringBuilderFacade.java",
    ),
    Signal(
        "unmodelled_upcast_conversions",
        "Synthesised From impls for Java upcasts with no model",
        r"Unmodelled Java upcast: ",
        "javalibs upcast coverage",
        "StringBuilderFacade.java",
    ),
    Signal(
        "stubbed_constants",
        "External static constants without modeled values",
        r'compile_error!\("external static constant [^"]+ has no modeled value"\)',
        "javalibs facade constant coverage",
        "NumericLimitConstants.java; ExternalConstantValue.java",
    ),
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--generated-root", required=True, type=Path)
    parser.add_argument(
        "--extra-rust",
        action="append",
        default=[],
        type=Path,
        help="additional Rust file to include outside --generated-root",
    )
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


def scan_files(generated_root: Path, extra_rust: list[Path]) -> list[Path]:
    files = rust_files(generated_root)
    for path in extra_rust:
        if not path.is_file():
            raise SystemExit(f"extra Rust file does not exist: {path}")
        files.append(path)
    return sorted(dict.fromkeys(path.resolve() for path in files))


# Signals whose regex is anchored per line rather than across the whole file.
LINE_ANCHORED_SIGNALS = {"noop_facade_methods"}


def signal_matches(signal: Signal, text: str) -> list[re.Match[str]]:
    flags = re.MULTILINE if signal.key in LINE_ANCHORED_SIGNALS else 0
    pattern = re.compile(signal.regex, flags)
    matches = list(pattern.finditer(text))
    if signal.key == "concrete_none":
        return [match for match in matches if not concrete_none_match_is_valid_option(text, match)]
    return matches


def concrete_none_match_is_valid_option(text: str, match: re.Match[str]) -> bool:
    line_start = text.rfind("\n", 0, match.start()) + 1
    line_end = text.find("\n", match.end())
    if line_end == -1:
        line_end = len(text)
    line = text[line_start:line_end]

    if re.search(r"\blet\s+(?:mut\s+)?[A-Za-z_][A-Za-z0-9_]*\s*:\s*Option<[^;]+>\s*=\s*None;", line):
        return True

    if re.search(r"\breturn\s+None;", line):
        return enclosing_function_returns_option(text, match.start())

    return False


def enclosing_function_returns_option(text: str, offset: int) -> bool:
    prefix = text[:offset]
    fn_matches = list(re.finditer(r"\bfn\s+[A-Za-z_][A-Za-z0-9_]*[^{;]*\{", prefix, re.MULTILINE))
    if not fn_matches:
        return False
    signature = fn_matches[-1].group(0)
    return re.search(r"->\s*Option\s*<", signature) is not None


def count_signal(signal: Signal, files: list[Path], source_map: dict[Path, str]) -> dict[str, object]:
    if signal.key == "noop_facade_call_sites":
        return count_noop_facade_call_sites_signal(signal, files, source_map)
    occurrences = 0
    matched_files = 0
    examples: list[str] = []
    for path in files:
        text = path.read_text(errors="replace")
        matches = signal_matches(signal, text)
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


def count_noop_facade_call_sites_signal(
    signal: Signal, files: list[Path], source_map: dict[Path, str]
) -> dict[str, object]:
    occurrences = 0
    matched_files = 0
    examples: list[str] = []
    for path in files:
        text = path.read_text(errors="replace")
        matches = count_noop_facade_call_sites(text)
        if matches == 0:
            continue
        occurrences += matches
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


def count_noop_facade_call_sites(text: str) -> int:
    constructors = noop_facade_constructors(text)
    constructor_calls = sum(
        sum(count_noop_facade_path_calls(line, path) for path in constructors)
        for line in text.splitlines()
        if not line_defines_noop_facade_constructor(line)
    )
    return constructor_calls + text.count(NOOP_FACADE_METHOD_CALL_SITE_PREFIX)


def count_noop_facade_path_calls(line: str, path: str) -> int:
    count = 0
    cursor = 0
    while cursor < len(line):
        if line.startswith("//", cursor):
            break
        if line[cursor] == '"':
            cursor += 1
            escaped = False
            while cursor < len(line):
                char = line[cursor]
                cursor += 1
                if escaped:
                    escaped = False
                elif char == "\\":
                    escaped = True
                elif char == '"':
                    break
            continue
        if line.startswith(path, cursor) and starts_on_rust_path_boundary(line, cursor):
            count += 1
            cursor += len(path)
        else:
            cursor += 1
    return count


def starts_on_rust_path_boundary(line: str, start: int) -> bool:
    if start == 0:
        return True
    return not is_rust_ident_continue(line[start - 1])


def is_rust_ident_continue(char: str) -> bool:
    return char == "_" or char.isalnum()


def noop_facade_constructors(text: str) -> list[str]:
    owner: str | None = None
    impl_depth = 0
    paths: list[str] = []
    for line in text.splitlines():
        trimmed = line.strip()
        if owner is None:
            owner = inherent_impl_owner(trimmed)
            if owner is not None:
                impl_depth = 0
        if not line_defines_noop_facade_constructor(line):
            if owner is not None:
                impl_depth += line.count("{")
                impl_depth -= line.count("}")
                if impl_depth <= 0:
                    owner = None
                    impl_depth = 0
            continue
        if owner is None:
            continue
        name_match = re.search(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)", trimmed)
        if name_match is None:
            continue
        paths.append(f"{owner}::{name_match.group(1)}(")
        impl_depth += line.count("{")
        impl_depth -= line.count("}")
        if impl_depth <= 0:
            owner = None
            impl_depth = 0
    return paths


def line_defines_noop_facade_constructor(line: str) -> bool:
    stripped = line.rstrip()
    return " fn new" in stripped and stripped.endswith("{ Default::default() }")


def inherent_impl_owner(trimmed: str) -> str | None:
    if not trimmed.startswith("impl") or " for " in trimmed:
        return None
    rest = trimmed[len("impl") :]
    if not rest.startswith("<") and (not rest or not rest[0].isspace()):
        return None
    rest = rest.lstrip()
    if rest.startswith("<"):
        generic_end = matching_angle_end(rest)
        if generic_end is None:
            return None
        rest = rest[generic_end + 1 :].lstrip()
    parts = rest.split()
    if not parts:
        return None
    owner = parts[0].rstrip("{").strip().split("<", 1)[0].strip()
    return owner or None


def matching_angle_end(text: str) -> int | None:
    depth = 0
    for index, char in enumerate(text):
        if char == "<":
            depth += 1
        elif char == ">":
            depth -= 1
            if depth == 0:
                return index
            if depth < 0:
                return None
    return None


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
    files = scan_files(args.generated_root, args.extra_rust)
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
