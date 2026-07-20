#!/usr/bin/env python3
"""Emit Cargo dependency lines required by external-pack manifests."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys


def parse_attrs(tokens: list[str]) -> dict[str, str]:
    attrs: dict[str, str] = {}
    for token in tokens:
        if "=" not in token:
            continue
        key, value = token.split("=", 1)
        attrs[key] = value
    return attrs


def manifest_crate_root(manifest: Path) -> Path | None:
    if manifest.parent.name == "manifests":
        return manifest.parent.parent
    return None


def dependency_for_target(manifest: Path, target: str, attrs: dict[str, str]) -> tuple[str, str] | None:
    if not target:
        return None

    package = attrs.get("package") or target.replace("_", "-")
    if "path" in attrs:
        dep_path = (manifest.parent / attrs["path"]).resolve()
        return package, f'{package} = {{ path = "{dep_path}" }}'

    root = manifest_crate_root(manifest)
    if root is None:
        return None

    candidate = root / "crates" / package
    if not candidate.exists():
        return None

    return package, f'{package} = {{ path = "{candidate.resolve()}" }}'


def extract_dependency_lines(manifest_paths: list[Path]) -> tuple[list[str], list[str]]:
    deps: dict[str, str] = {}
    warnings: list[str] = []
    for manifest in manifest_paths:
        try:
            lines = manifest.read_text(encoding="utf-8").splitlines()
        except OSError as err:
            warnings.append(f"{manifest}: cannot read manifest: {err}")
            continue

        for lineno, raw_line in enumerate(lines, start=1):
            line = raw_line.split("#", 1)[0].strip()
            if not line:
                continue
            tokens = line.split()
            if len(tokens) < 5:
                continue
            if tokens[0] != "external-pack" or tokens[3] not in {
                "rust-crate-mapping",
                "rust_crate_mapping",
            }:
                continue

            attrs = parse_attrs(tokens[5:])
            target = attrs.get("target")
            if target is None:
                continue

            dep = dependency_for_target(manifest, target, attrs)
            if dep is None:
                warnings.append(
                    f"{manifest}:{lineno}: cannot infer Cargo dependency for target={target}"
                )
                continue

            package, line = dep
            existing = deps.get(package)
            if existing is not None and existing != line:
                warnings.append(
                    f"{manifest}:{lineno}: conflicting Cargo dependency for {package}"
                )
                continue
            deps[package] = line

    return [deps[package] for package in sorted(deps)], warnings


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("manifest", nargs="*", type=Path)
    args = parser.parse_args()

    lines, warnings = extract_dependency_lines(args.manifest)
    for warning in warnings:
        print(f"warning: {warning}", file=sys.stderr)
    for line in lines:
        print(line)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
