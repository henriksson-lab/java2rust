#!/usr/bin/env python3
import sys
import json
from pathlib import Path


def tokens(path: Path):
    for raw in path.read_text().splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        yield line.split()


def rust_type_name(java_type: str) -> str:
    return java_type.rsplit(".", 1)[-1]


def main() -> int:
    type_targets: dict[str, str] = {}
    exception_types: set[str] = set()
    for arg in sys.argv[1:]:
        path = Path(arg)
        for parts in tokens(path):
            if len(parts) >= 5 and parts[0] == "external-pack":
                _, _pack_name, type_name, mode, *_ = parts
                if mode not in {"rust-crate-mapping", "rust_crate_mapping"}:
                    continue
                target = None
                for token in parts[5:]:
                    if token.startswith("target="):
                        target = token.split("=", 1)[1]
                        break
                if target:
                    type_targets[type_name] = target
            elif len(parts) >= 4 and parts[0] == "external-pack-exception":
                exception_types.add(parts[2])

    emitted = set()
    for java_type in sorted(exception_types):
        crate_name = type_targets.get(java_type)
        if not crate_name:
            continue
        rust_ty = f"{crate_name}::{rust_type_name(java_type)}"
        if rust_ty in emitted:
            continue
        emitted.add(rust_ty)
        print()
        print(f"impl From<{rust_ty}> for JavaException {{")
        print(f"    fn from(err: {rust_ty}) -> JavaException {{")
        print(f"        JavaException::from_display({json.dumps(java_type)}, err)")
        print("    }")
        print("}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
