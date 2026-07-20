#!/usr/bin/env python3
import argparse
import re
from collections import defaultdict
from pathlib import Path


TYPE_RE = re.compile(r"^pub\s+(struct|enum)\s+([A-Za-z_][A-Za-z0-9_]*)")
ALIAS_RE = re.compile(r"^pub\s+type\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*([A-Za-z_][A-Za-z0-9_]*)\s*;")
DERIVE_RE = re.compile(r"^#\[derive")
INHERENT_IMPL_RE = re.compile(r"^impl(?:\s*<[^>{}]*>)?\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:\{|where\b)")
TRAIT_IMPL_RE = re.compile(
    r"^impl(?:\s*<[^>{}]*>)?\s+(.+?)\s+for\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:\{|where\b)"
)
FN_RE = re.compile(r"^\s*(?:pub\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\b")
CONST_RE = re.compile(r"^\s*pub\s+const\s+([A-Za-z_][A-Za-z0-9_]*)\b")


def brace_delta(line: str) -> int:
    delta = 0
    index = 0
    state = "code"
    while index < len(line):
        ch = line[index]
        next_ch = line[index + 1] if index + 1 < len(line) else ""
        if state == "code":
            if ch == "/" and next_ch == "/":
                break
            if ch == "/" and next_ch == "*":
                state = "block_comment"
                index += 2
                continue
            if ch == '"':
                state = "string"
                index += 1
                continue
            if ch == "'":
                state = "char"
                index += 1
                continue
            if ch == "{":
                delta += 1
            elif ch == "}":
                delta -= 1
            index += 1
            continue
        if state == "string":
            if ch == "\\":
                index += 2
                continue
            if ch == '"':
                state = "code"
            index += 1
            continue
        if state == "char":
            if ch == "\\":
                index += 2
                continue
            if ch == "'":
                state = "code"
            index += 1
            continue
        if state == "block_comment":
            if ch == "*" and next_ch == "/":
                state = "code"
                index += 2
                continue
            index += 1
    return delta


def source_type_for(path: Path) -> str:
    stem = path.stem
    match = re.match(r"^\d+_(.+)$", stem)
    return match.group(1) if match else stem


def item_end(lines: list[str], start: int) -> int:
    if "{" not in lines[start]:
        return start + 1
    depth = 0
    for index in range(start, len(lines)):
        depth += brace_delta(lines[index])
        if depth <= 0:
            return index + 1
    return len(lines)


def attached_start(lines: list[str], start: int) -> int:
    index = start
    while index > 0 and DERIVE_RE.match(lines[index - 1]):
        index -= 1
    return index


def prelude_support_types(path: Path) -> set[str]:
    source_type = source_type_for(path)
    lines = path.read_text().splitlines(keepends=True)
    names = set()
    depth = 0
    for line in lines:
        if depth == 0:
            match = TYPE_RE.match(line)
            if match:
                name = match.group(2)
                if name == source_type:
                    break
                names.add(name)
        depth += brace_delta(line)
    return names


def parse_items(path: Path, selected: set[str]) -> list[dict]:
    lines = path.read_text().splitlines(keepends=True)
    items = []
    depth = 0
    index = 0
    while index < len(lines):
        line = lines[index]
        if depth == 0:
            type_match = TYPE_RE.match(line)
            if type_match and type_match.group(2) in selected:
                start = attached_start(lines, index)
                end = item_end(lines, index)
                items.append(
                    {
                        "kind": "decl",
                        "name": type_match.group(2),
                        "start": start,
                        "end": end,
                        "text": "".join(lines[start:end]),
                    }
                )
                index = end
                continue

            alias_match = ALIAS_RE.match(line)
            if alias_match and (
                alias_match.group(1) in selected or alias_match.group(2) in selected
            ):
                items.append(
                    {
                        "kind": "alias",
                        "name": alias_match.group(1),
                        "target": alias_match.group(2),
                        "start": index,
                        "end": index + 1,
                        "text": line,
                    }
                )
                index += 1
                continue

            trait_match = TRAIT_IMPL_RE.match(line)
            if trait_match and trait_match.group(2) in selected:
                end = item_end(lines, index)
                items.append(
                    {
                        "kind": "trait_impl",
                        "name": trait_match.group(2),
                        "trait": trait_match.group(1).strip(),
                        "start": index,
                        "end": end,
                        "text": "".join(lines[index:end]),
                    }
                )
                index = end
                continue

            inherent_match = INHERENT_IMPL_RE.match(line)
            if inherent_match and inherent_match.group(1) in selected:
                end = item_end(lines, index)
                items.append(
                    {
                        "kind": "inherent_impl",
                        "name": inherent_match.group(1),
                        "start": index,
                        "end": end,
                        "text": "".join(lines[index:end]),
                    }
                )
                index = end
                continue

        depth += brace_delta(line)
        index += 1
    return items


def extract_methods(impl_text: str) -> list[tuple[str, str]]:
    lines = impl_text.splitlines(keepends=True)
    methods = []
    index = 1
    while index < max(1, len(lines) - 1):
        line = lines[index]
        match = FN_RE.match(line)
        if not match:
            index += 1
            continue
        start = index
        depth = brace_delta(line)
        if "{" not in line:
            index += 1
            continue
        index += 1
        while index < len(lines) - 1 and depth > 0:
            depth += brace_delta(lines[index])
            index += 1
        methods.append((match.group(1), "".join(lines[start:index]).strip()))
    return methods


def extract_consts(impl_text: str) -> list[tuple[str, str]]:
    lines = impl_text.splitlines(keepends=True)
    consts = []
    for line in lines[1 : max(1, len(lines) - 1)]:
        match = CONST_RE.match(line)
        if match:
            consts.append((match.group(1), line.strip()))
    return consts


def remove_items(path: Path, items: list[dict]) -> None:
    lines = path.read_text().splitlines(keepends=True)
    remove = [False] * len(lines)
    for item in items:
        for index in range(item["start"], item["end"]):
            remove[index] = True
    path.write_text("".join(line for index, line in enumerate(lines) if not remove[index]))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--generated-dir", required=True)
    parser.add_argument("--shared-support", required=True)
    parser.add_argument("--shared-types", required=True)
    parser.add_argument("--conflicts", required=True)
    args = parser.parse_args()

    generated_dir = Path(args.generated_dir)
    rust_paths = sorted(generated_dir.glob("*.rs"))
    primary_output_types = {source_type_for(path) for path in rust_paths}
    candidates_by_type = defaultdict(list)
    for path in rust_paths:
        for name in prelude_support_types(path):
            candidates_by_type[name].append(path)

    selected = {
        name
        for name, paths in candidates_by_type.items()
        if len(paths) > 1
        and name not in primary_output_types
        and name not in {"Box", "String", "Vec", "Option", "Result"}
    }

    conflict_lines = ["type\tkind\tdetail\n"]
    if not selected:
        Path(args.conflicts).write_text("".join(conflict_lines))
        return 0

    items_by_path = {}
    declarations = defaultdict(list)
    aliases = defaultdict(list)
    trait_impls = defaultdict(list)
    inherent_methods = defaultdict(list)
    inherent_consts = defaultdict(list)

    for path in rust_paths:
        items = parse_items(path, selected)
        items_by_path[path] = items
        for item in items:
            if item["kind"] == "decl":
                declarations[item["name"]].append((path, item["text"]))
            elif item["kind"] == "alias":
                aliases[item["name"]].append((path, item["target"], item["text"]))
            elif item["kind"] == "trait_impl":
                trait_impls[(item["name"], item["trait"])].append((path, item["text"]))
            elif item["kind"] == "inherent_impl":
                for method_name, method_text in extract_methods(item["text"]):
                    inherent_methods[(item["name"], method_name)].append((path, method_text))
                for const_name, const_text in extract_consts(item["text"]):
                    inherent_consts[(item["name"], const_name)].append((path, const_text))

    shared_chunks = []
    shared_aliases = set()
    for name in sorted(selected):
        decls = declarations.get(name, [])
        unique_decls = []
        seen_decls = set()
        for _, text in decls:
            if text not in seen_decls:
                unique_decls.append(text)
                seen_decls.add(text)
        if not unique_decls:
            continue
        if len(unique_decls) > 1:
            conflict_lines.append(f"{name}\tdeclaration\tkept first of {len(unique_decls)} variants\n")
        shared_chunks.append(unique_decls[0].rstrip() + "\n")

        methods_for_type = []
        consts_for_type = []
        for (type_name, const_name), variants in sorted(inherent_consts.items()):
            if type_name != name:
                continue
            unique_variants = []
            seen_consts = set()
            for _, text in variants:
                if text not in seen_consts:
                    unique_variants.append(text)
                    seen_consts.add(text)
            if len(unique_variants) > 1:
                conflict_lines.append(
                    f"{name}\tconst\t{const_name}: kept first of {len(unique_variants)} variants\n"
                )
            consts_for_type.append(unique_variants[0])
        for (type_name, method_name), variants in sorted(inherent_methods.items()):
            if type_name != name:
                continue
            unique_variants = []
            seen_methods = set()
            for _, text in variants:
                if text not in seen_methods:
                    unique_variants.append(text)
                    seen_methods.add(text)
            if len(unique_variants) > 1:
                conflict_lines.append(
                    f"{name}\tmethod\t{method_name}: kept first of {len(unique_variants)} variants\n"
                )
            methods_for_type.append(unique_variants[0])
        if consts_for_type or methods_for_type:
            shared_chunks.append(f"\nimpl {name} {{\n")
            for const in consts_for_type:
                shared_chunks.append(f"    {const}\n")
            for method in methods_for_type:
                shared_chunks.append("    " + method.replace("\n", "\n    ").rstrip() + "\n")
            shared_chunks.append("}\n")

        for (type_name, trait_name), variants in sorted(trait_impls.items()):
            if type_name != name:
                continue
            unique_variants = []
            seen_traits = set()
            for _, text in variants:
                if text not in seen_traits:
                    unique_variants.append(text)
                    seen_traits.add(text)
            if len(unique_variants) > 1:
                conflict_lines.append(
                    f"{name}\ttrait_impl\t{trait_name}: kept first of {len(unique_variants)} variants\n"
                )
            shared_chunks.append("\n" + unique_variants[0].rstrip() + "\n")
        shared_chunks.append("\n")

    for alias_name, variants in sorted(aliases.items()):
        if not any(target in selected for _, target, _ in variants):
            continue
        unique_variants = []
        seen_aliases = set()
        for _, _, text in variants:
            if text not in seen_aliases:
                unique_variants.append(text)
                seen_aliases.add(text)
        if len(unique_variants) > 1:
            conflict_lines.append(
                f"{alias_name}\talias\tkept first of {len(unique_variants)} variants\n"
            )
        shared_chunks.append(unique_variants[0])
        shared_aliases.add(alias_name)

    with Path(args.shared_support).open("a") as shared:
        if shared_chunks:
            shared.write("\n")
            shared.write("".join(shared_chunks))

    with Path(args.shared_types).open("a") as shared_types:
        for name in sorted(selected | shared_aliases):
            shared_types.write(f"{name}\n")

    for path, items in items_by_path.items():
        remove_items(path, items)

    Path(args.conflicts).write_text("".join(conflict_lines))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
