#!/usr/bin/env python3
"""Map a jdeps class dependency closure back to Java source files.

The input jar must have been compiled from the supplied source root.  Root
classes are given by fully-qualified binary names.  Dependencies outside that
jar are deliberately ignored: they belong in translator context/fact inputs,
not in the emitted source set.
"""

from __future__ import annotations

import argparse
import collections
import pathlib
import re
import subprocess
import sys


DEPENDENCY_RE = re.compile(r"^\s+(\S+)\s+->\s+(\S+)\s+(\S+)\s*$")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--jar", required=True, type=pathlib.Path)
    parser.add_argument("--source-root", required=True, type=pathlib.Path)
    parser.add_argument("--root-class", action="append", required=True)
    parser.add_argument("--module-path")
    parser.add_argument("--multi-release")
    return parser.parse_args()


def source_index(source_root: pathlib.Path) -> dict[str, str]:
    result: dict[str, str] = {}
    for source in sorted(source_root.rglob("*.java")):
        relative = source.relative_to(source_root)
        if source.name in {"module-info.java", "package-info.java"}:
            continue
        result[".".join(relative.with_suffix("").parts)] = relative.as_posix()
    return result


def main() -> int:
    args = parse_args()
    command = ["jdeps"]
    if args.multi_release:
        command.extend(["--multi-release", args.multi_release])
    if args.module_path:
        command.extend(["--module-path", args.module_path])
    command.extend(["-verbose:class", "-filter:none", str(args.jar)])
    completed = subprocess.run(command, check=True, text=True, stdout=subprocess.PIPE)

    source_by_class = source_index(args.source_root)
    adjacency: dict[str, set[str]] = collections.defaultdict(set)
    for line in completed.stdout.splitlines():
        match = DEPENDENCY_RE.match(line)
        if not match:
            continue
        source_class, target_class, _module = match.groups()
        source_class = source_class.split("$", 1)[0]
        target_class = target_class.split("$", 1)[0]
        if source_class in source_by_class and target_class in source_by_class:
            adjacency[source_class].add(target_class)

    unknown = sorted(set(args.root_class) - source_by_class.keys())
    if unknown:
        print("root classes have no matching source file:", file=sys.stderr)
        for class_name in unknown:
            print(f"  {class_name}", file=sys.stderr)
        return 2

    pending = collections.deque(args.root_class)
    reached: set[str] = set()
    while pending:
        class_name = pending.popleft()
        if class_name in reached:
            continue
        reached.add(class_name)
        pending.extend(sorted(adjacency[class_name] - reached))

    for relative in sorted(source_by_class[class_name] for class_name in reached):
        print(relative)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
