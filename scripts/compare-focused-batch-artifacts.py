#!/usr/bin/env python3
"""Compare focused batch outputs while ignoring output-directory path prefixes."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path


def read_tsv(path: Path) -> list[list[str]]:
    if not path.exists():
        raise FileNotFoundError(path)
    rows: list[list[str]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        rows.append(line.split("\t"))
    return rows


def normalize_manifest(path: Path) -> list[tuple[str, str, str, str, str]]:
    rows = read_tsv(path)
    if not rows:
        return []
    header = rows[0]
    expected = ["index", "source", "rust_path", "rust_hash", "diagnostics", "invalid_patterns"]
    if header != expected:
        raise ValueError(f"{path}: unexpected batch-manifest header: {header}")
    normalized = []
    for row in rows[1:]:
        if len(row) != 6:
            raise ValueError(f"{path}: malformed batch-manifest row: {row}")
        index, source, rust_path, rust_hash, diagnostics, invalid_patterns = row
        normalized.append(
            (index, source, Path(rust_path).name, rust_hash, diagnostics, invalid_patterns)
        )
    return normalized


def normalize_status(path: Path) -> list[tuple[str, str, str, str, str, str]]:
    rows = read_tsv(path)
    if not rows:
        return []
    header = rows[0]
    expected = [
        "source",
        "rust_path",
        "translate_status",
        "diagnostics",
        "invalid_patterns",
        "unsupported_members",
    ]
    if header != expected:
        raise ValueError(f"{path}: unexpected status header: {header}")
    normalized = []
    for row in rows[1:]:
        if len(row) != 6:
            raise ValueError(f"{path}: malformed status row: {row}")
        (
            source,
            rust_path,
            translate_status,
            diagnostics,
            invalid_patterns,
            unsupported_members,
        ) = row
        normalized.append(
            (
                source,
                Path(rust_path).name,
                translate_status,
                diagnostics,
                invalid_patterns,
                unsupported_members,
            )
        )
    return normalized


def normalize_raw_tsv(path: Path) -> list[tuple[str, ...]]:
    return [tuple(row) for row in read_tsv(path)]


def normalize_error_tsv(path: Path, root: Path) -> list[tuple[str, ...]]:
    root_text = str(root)
    normalized = []
    for row in read_tsv(path):
        normalized.append(tuple(field.replace(root_text, "<output-dir>") for field in row))
    return normalized


def summary_values(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    for row in read_tsv(path):
        if len(row) >= 2:
            values[row[0]] = row[1]
    return values


def normalize_analysis_profile(path: Path) -> list[tuple[str, str]]:
    values = summary_values(path)
    volatile_counter_keys = {
        "signature_component_sweeps",
        "signature_changed_component_sweeps",
        "signature_state_clones",
    }
    return sorted(
        (key, value)
        for key, value in values.items()
        if not key.endswith("_ms") and key not in volatile_counter_keys
        and not key.startswith("signature_propagation_arm_")
    )


def compare_value(
    label: str,
    left: object,
    right: object,
    differences: list[str],
) -> None:
    if left != right:
        differences.append(f"{label} differs")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serial-dir", required=True, type=Path)
    parser.add_argument("--parallel-dir", required=True, type=Path)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()

    differences: list[str] = []
    try:
        compare_value(
            "batch-manifest",
            normalize_manifest(args.serial_dir / "batch-manifest.tsv"),
            normalize_manifest(args.parallel_dir / "batch-manifest.tsv"),
            differences,
        )
        compare_value(
            "status",
            normalize_status(args.serial_dir / "status.tsv"),
            normalize_status(args.parallel_dir / "status.tsv"),
            differences,
        )
        compare_value(
            "invalid-patterns",
            normalize_raw_tsv(args.serial_dir / "invalid-patterns.tsv"),
            normalize_raw_tsv(args.parallel_dir / "invalid-patterns.tsv"),
            differences,
        )
        compare_value(
            "top-compile-errors",
            normalize_error_tsv(args.serial_dir / "top-compile-errors.tsv", args.serial_dir),
            normalize_error_tsv(args.parallel_dir / "top-compile-errors.tsv", args.parallel_dir),
            differences,
        )
        serial_profile = args.serial_dir / "analysis-profile.tsv"
        parallel_profile = args.parallel_dir / "analysis-profile.tsv"
        if serial_profile.exists() or parallel_profile.exists():
            compare_value(
                "analysis-profile counters",
                normalize_analysis_profile(serial_profile),
                normalize_analysis_profile(parallel_profile),
                differences,
            )
        serial_summary = summary_values(args.serial_dir / "summary.tsv")
        parallel_summary = summary_values(args.parallel_dir / "summary.tsv")
        for key in [
            "files",
            "translate_failed_files",
            "diagnostic_files",
            "parse_failures",
            "cargo_type_errors_valid",
            "unsupported_statements",
            "unsupported_members",
            "unsupported_expressions",
            "unmodelled_types",
            "unmodelled_call_sites",
            "noop_facade_methods",
            "invalid_pattern_files",
            "cargo_check",
            "cargo_check_errors",
            "cargo_failure_kind",
        ]:
            compare_value(
                f"summary.{key}",
                serial_summary.get(key),
                parallel_summary.get(key),
                differences,
            )
    except (FileNotFoundError, ValueError) as err:
        differences.append(str(err))

    status = "ok" if not differences else "failed"
    lines = [
        f"serial_dir\t{args.serial_dir}",
        f"parallel_dir\t{args.parallel_dir}",
        f"status\t{status}",
    ]
    lines.extend(f"difference\t{difference}" for difference in differences)
    report = "\n".join(lines) + "\n"
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(report, encoding="utf-8")
    print(report, end="")
    return 0 if not differences else 1


if __name__ == "__main__":
    sys.exit(main())
