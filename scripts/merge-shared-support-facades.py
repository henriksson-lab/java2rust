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
FROM_TRAIT_RE = re.compile(r"^From<([A-Za-z_][A-Za-z0-9_]*)>$")
FN_RE = re.compile(r"^\s*(?:pub\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\b")
CONST_RE = re.compile(r"^\s*pub\s+const\s+([A-Za-z_][A-Za-z0-9_]*)\b")
STATIC_METHOD_CALL_RE_TEMPLATE = r"\b{}\s*::\s*{}\s*(?:<|\()"
INSTANCE_METHOD_CALL_RE_TEMPLATE = r"\.\s*{}\s*(?:<|\()"


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


def source_owned_nested_type_name(source_type: str, name: str) -> bool:
    if name == source_type or not name.startswith(source_type):
        return False
    suffix = name[len(source_type) :]
    return bool(suffix) and suffix[0].isupper()


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
                if source_owned_nested_type_name(source_type, name):
                    depth += brace_delta(line)
                    continue
                names.add(name)
        depth += brace_delta(line)
    return names


def prelude_aliases(path: Path) -> list[tuple[str, str]]:
    source_type = source_type_for(path)
    lines = path.read_text().splitlines(keepends=True)
    aliases = []
    depth = 0
    for line in lines:
        if depth == 0:
            type_match = TYPE_RE.match(line)
            if type_match and type_match.group(2) == source_type:
                break
            alias_match = ALIAS_RE.match(line)
            if alias_match:
                aliases.append((alias_match.group(1), alias_match.group(2)))
        depth += brace_delta(line)
    return aliases


def prelude_from_impls(path: Path) -> list[tuple[str, str]]:
    source_type = source_type_for(path)
    lines = path.read_text().splitlines(keepends=True)
    impls = []
    depth = 0
    for line in lines:
        if depth == 0:
            type_match = TYPE_RE.match(line)
            if type_match and type_match.group(2) == source_type:
                break
            trait_match = TRAIT_IMPL_RE.match(line)
            if trait_match:
                from_match = FROM_TRAIT_RE.match(trait_match.group(1).strip())
                if from_match:
                    impls.append((from_match.group(1), trait_match.group(2)))
        depth += brace_delta(line)
    return impls


def prelude_types_redeclared_after_primary(path: Path) -> set[str]:
    source_type = source_type_for(path)
    lines = path.read_text().splitlines(keepends=True)
    prelude = set()
    redeclared = set()
    depth = 0
    after_primary = False
    for line in lines:
        if depth == 0:
            type_match = TYPE_RE.match(line)
            if type_match:
                name = type_match.group(2)
                if after_primary:
                    if name in prelude and not source_owned_nested_type_name(source_type, name):
                        redeclared.add(name)
                elif name == source_type:
                    after_primary = True
                elif not source_owned_nested_type_name(source_type, name):
                    prelude.add(name)
        depth += brace_delta(line)
    return redeclared


def duplicate_prelude_types_before_primary(path: Path) -> set[str]:
    source_type = source_type_for(path)
    lines = path.read_text().splitlines(keepends=True)
    counts = defaultdict(int)
    depth = 0
    for line in lines:
        if depth == 0:
            type_match = TYPE_RE.match(line)
            if type_match:
                name = type_match.group(2)
                if name == source_type:
                    break
                if source_owned_nested_type_name(source_type, name):
                    depth += brace_delta(line)
                    continue
                counts[name] += 1
        depth += brace_delta(line)
    return {name for name, count in counts.items() if count > 1}


def duplicate_source_owned_nested_last_decl_starts(path: Path) -> dict[str, int]:
    source_type = source_type_for(path)
    lines = path.read_text().splitlines(keepends=True)
    starts_by_name = defaultdict(list)
    depth = 0
    index = 0
    while index < len(lines):
        line = lines[index]
        if depth == 0:
            type_match = TYPE_RE.match(line)
            if type_match:
                name = type_match.group(2)
                if name == source_type:
                    break
                if source_owned_nested_type_name(source_type, name):
                    starts_by_name[name].append(attached_start(lines, index))
        depth += brace_delta(line)
        index += 1
    return {
        name: starts[-1]
        for name, starts in starts_by_name.items()
        if len(starts) > 1
    }


def parse_items(path: Path, selected: set[str]) -> list[dict]:
    source_type = source_type_for(path)
    lines = path.read_text().splitlines(keepends=True)
    items = []
    depth = 0
    index = 0
    after_primary = False
    while index < len(lines):
        line = lines[index]
        if depth == 0:
            type_match = TYPE_RE.match(line)
            if type_match and type_match.group(2) == source_type:
                after_primary = True
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
                        "prelude": not after_primary,
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
                        "prelude": not after_primary,
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
                        "prelude": not after_primary,
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
                        "prelude": not after_primary,
                    }
                )
                index = end
                continue

        depth += brace_delta(line)
        index += 1
    return items


def remove_duplicate_source_owned_nested_stubs(path: Path) -> None:
    last_decl_start_by_name = duplicate_source_owned_nested_last_decl_starts(path)
    if not last_decl_start_by_name:
        return

    source_type = source_type_for(path)
    lines = path.read_text().splitlines(keepends=True)
    remove = [False] * len(lines)
    depth = 0
    index = 0
    while index < len(lines):
        line = lines[index]
        if depth == 0:
            type_match = TYPE_RE.match(line)
            if type_match and type_match.group(2) == source_type:
                break
            name = None
            start = index
            if type_match:
                name = type_match.group(2)
                start = attached_start(lines, index)
            else:
                inherent_match = INHERENT_IMPL_RE.match(line)
                if inherent_match:
                    name = inherent_match.group(1)
            if name in last_decl_start_by_name and start < last_decl_start_by_name[name]:
                end = item_end(lines, index)
                for remove_index in range(start, end):
                    remove[remove_index] = True
                index = end
                continue
        depth += brace_delta(line)
        index += 1

    path.write_text("".join(line for line_index, line in enumerate(lines) if not remove[line_index]))


def remove_duplicate_inherent_methods(path: Path) -> None:
    lines = path.read_text().splitlines(keepends=True)
    remove = [False] * len(lines)
    depth = 0
    index = 0
    while index < len(lines):
        line = lines[index]
        if depth == 0 and (impl_match := INHERENT_IMPL_RE.match(line)):
            impl_end = item_end(lines, index)
            methods_by_name = defaultdict(list)
            inner = index + 1
            while inner < max(index + 1, impl_end - 1):
                fn_match = FN_RE.match(lines[inner])
                if not fn_match:
                    inner += 1
                    continue
                method_start = inner
                method_end = item_end(lines, inner)
                methods_by_name[fn_match.group(1)].append(
                    {
                        "name": fn_match.group(1),
                        "start": method_start,
                        "end": method_end,
                        "text": "".join(lines[method_start:method_end]).strip(),
                    }
                )
                inner = method_end
            for methods in methods_by_name.values():
                unique_texts = []
                seen = set()
                for method in methods:
                    if method["text"] not in seen:
                        unique_texts.append(method["text"])
                        seen.add(method["text"])
                if len(unique_texts) <= 1:
                    continue
                keep_text = best_method_variant(methods[0]["name"], unique_texts, set())
                kept = False
                for method in methods:
                    if method["text"] == keep_text and not kept:
                        kept = True
                        continue
                    for remove_index in range(method["start"], method["end"]):
                        remove[remove_index] = True
            index = impl_end
            continue
        depth += brace_delta(line)
        index += 1
    path.write_text("".join(line for line_index, line in enumerate(lines) if not remove[line_index]))


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


def method_return_type(method_text: str) -> str:
    first_line = method_text.splitlines()[0] if method_text.splitlines() else method_text
    if " -> " not in first_line:
        return ""
    return first_line.split(" -> ", 1)[1].split(" {", 1)[0].strip()


def method_signature_text(method_text: str) -> str:
    before_body = method_text.split(" {", 1)[0]
    return " ".join(line.strip() for line in before_body.splitlines())


def method_receiver_kind(method_text: str) -> str:
    signature = method_signature_text(method_text)
    open_paren = signature.find("(")
    close_paren = signature.find(")", open_paren + 1)
    if open_paren < 0 or close_paren < 0:
        return "unknown"
    first_arg = signature[open_paren + 1 : close_paren].split(",", 1)[0].strip()
    if first_arg == "&mut self":
        return "mut_self"
    if first_arg == "&self":
        return "self"
    if first_arg == "self":
        return "owned_self"
    return "static"


def weak_method_variant(method_text: str) -> bool:
    return_type = method_return_type(method_text)
    first_line = method_text.splitlines()[0] if method_text.splitlines() else method_text
    return (
        "<R: Default" in first_line
        or return_type == "R"
        or return_type == "()"
        or return_type == "Box<dyn std::any::Any>"
    )


def method_receiver_preference_rank(method_text: str, preferred_receiver_kinds: set[str]) -> int:
    receiver = method_receiver_kind(method_text)
    if receiver in preferred_receiver_kinds:
        return 0
    if preferred_receiver_kinds:
        return 1
    return 0


def default_receiver_rank(method_name: str, method_text: str) -> int:
    receiver = method_receiver_kind(method_text)
    if method_name in {"new_builder", "get_instance"} and receiver == "static":
        return 0
    if receiver == "static":
        return 2
    return 1


def best_method_variant(
    method_name: str, variants: list[str], preferred_receiver_kinds: set[str]
) -> str:
    return sorted(
        variants,
        key=lambda text: (
            method_receiver_preference_rank(text, preferred_receiver_kinds),
            default_receiver_rank(method_name, text),
            weak_method_variant(text),
            method_return_type(text) == "",
            text,
        ),
    )[0]


def method_call_receiver_preferences(
    rust_paths: list[Path], selected: set[str], alias_targets: dict[str, str]
) -> dict[tuple[str, str], set[str]]:
    preferences = defaultdict(set)
    names_by_target = defaultdict(set)
    for name in selected:
        names_by_target[name].add(name)
    for alias, target in alias_targets.items():
        names_by_target[target].add(alias)
    path_texts = [path.read_text() for path in rust_paths]
    for target, names in names_by_target.items():
        for type_name in names:
            type_pattern = re.escape(type_name)
            for method_name in method_names_for_target(type_name, rust_paths, selected):
                method_pattern = re.escape(method_name)
                static_re = re.compile(
                    STATIC_METHOD_CALL_RE_TEMPLATE.format(type_pattern, method_pattern)
                )
                instance_re = re.compile(
                    INSTANCE_METHOD_CALL_RE_TEMPLATE.format(method_pattern)
                )
                for text in path_texts:
                    if static_re.search(text):
                        preferences[(target, method_name)].add("static")
                    if instance_re.search(text):
                        preferences[(target, method_name)].update(
                            {"self", "mut_self", "owned_self"}
                        )
    return preferences


def method_names_for_target(type_name: str, rust_paths: list[Path], selected: set[str]) -> set[str]:
    names = set()
    for path in rust_paths:
        for item in parse_items(path, selected):
            if item["kind"] == "inherent_impl" and item["name"] == type_name:
                for method_name, _ in extract_methods(item["text"]):
                    names.add(method_name)
    return names


def remove_items(
    path: Path,
    items: list[dict],
    removal_only_names: set[str],
    keep_last_prelude_names: set[str],
) -> None:
    lines = path.read_text().splitlines(keepends=True)
    remove = [False] * len(lines)
    last_prelude_start_by_name = {
        item["name"]: item["start"]
        for item in items
        if item["name"] in keep_last_prelude_names and item.get("prelude", False)
    }
    for item in items:
        if item["name"] in removal_only_names and not item.get("prelude", False):
            continue
        if (
            item["name"] in keep_last_prelude_names
            and item.get("prelude", False)
            and item["start"] == last_prelude_start_by_name.get(item["name"])
        ):
            continue
        for index in range(item["start"], item["end"]):
            remove[index] = True
    path.write_text("".join(line for index, line in enumerate(lines) if not remove[index]))


def box_self_recursive_fields(name: str, text: str) -> str:
    field_ty = re.escape(name)
    direct_self_field = re.compile(
        rf"(?m)^(\s*pub\s+[A-Za-z_][A-Za-z0-9_]*\s*:\s*){field_ty}(\s*,\s*)$"
    )
    return direct_self_field.sub(rf"\1Box<{name}>\2", text)


def item_targets_name(item_name: str, target_name: str, alias_targets: dict[str, str]) -> bool:
    return item_name == target_name or alias_targets.get(item_name) == target_name


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--generated-dir", required=True)
    parser.add_argument("--shared-support", required=True)
    parser.add_argument("--shared-types", required=True)
    parser.add_argument("--conflicts", required=True)
    args = parser.parse_args()

    generated_dir = Path(args.generated_dir)
    rust_paths = sorted(generated_dir.glob("*.rs"))
    shared_support_path = Path(args.shared_support)
    existing_shared_text = (
        shared_support_path.read_text() if shared_support_path.exists() else ""
    )
    removal_only_names = {
        match.group(1)
        for line in existing_shared_text.splitlines()
        if (match := ALIAS_RE.match(line))
    }
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
    keep_last_prelude_names = set()
    for path in rust_paths:
        removal_only_names.update(prelude_types_redeclared_after_primary(path))
        keep_last_prelude_names.update(duplicate_prelude_types_before_primary(path))
    removal_only_names.update(keep_last_prelude_names)
    selected.update(removal_only_names - primary_output_types)
    promoted_aliases = set()
    for path in rust_paths:
        for alias, target in prelude_aliases(path):
            if target in selected and alias not in primary_output_types:
                promoted_aliases.add(alias)
    selected.update(promoted_aliases)
    removal_only_names.update(promoted_aliases)

    from_impls = []
    for path in rust_paths:
        from_impls.extend(prelude_from_impls(path))
    changed = True
    while changed:
        changed = False
        for from_name, to_name in from_impls:
            if from_name in primary_output_types or to_name in primary_output_types:
                continue
            if from_name in selected and to_name not in selected:
                selected.add(to_name)
                changed = True
            if to_name in selected and from_name not in selected:
                selected.add(from_name)
                changed = True

    conflict_lines = ["type\tkind\tdetail\n"]
    if not selected:
        for path in rust_paths:
            remove_duplicate_source_owned_nested_stubs(path)
            remove_duplicate_inherent_methods(path)
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
    alias_targets = {}
    for alias_name, variants in aliases.items():
        for _, target, _ in variants:
            if target in selected:
                alias_targets[alias_name] = target
                break
    receiver_preferences = method_call_receiver_preferences(rust_paths, selected, alias_targets)
    for name in sorted(selected):
        if name in removal_only_names:
            continue
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
        shared_chunks.append(box_self_recursive_fields(name, unique_decls[0]).rstrip() + "\n")

        method_variants_by_name = defaultdict(list)
        methods_for_type = []
        consts_for_type = []
        for (type_name, const_name), variants in sorted(inherent_consts.items()):
            if not item_targets_name(type_name, name, alias_targets):
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
            if not item_targets_name(type_name, name, alias_targets):
                continue
            for _, text in variants:
                method_variants_by_name[method_name].append(text)
        for method_name, variants in sorted(method_variants_by_name.items()):
            unique_variants = []
            seen_methods = set()
            for text in variants:
                if text not in seen_methods:
                    unique_variants.append(text)
                    seen_methods.add(text)
            if len(unique_variants) > 1:
                conflict_lines.append(
                    f"{name}\tmethod\t{method_name}: kept preferred of {len(unique_variants)} variants\n"
                )
            preferred_receiver_kinds = receiver_preferences.get((name, method_name), set())
            methods_for_type.append(
                best_method_variant(method_name, unique_variants, preferred_receiver_kinds)
            )
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
        for name in sorted((selected - removal_only_names) | shared_aliases):
            shared_types.write(f"{name}\n")

    for path, items in items_by_path.items():
        remove_items(path, items, removal_only_names, keep_last_prelude_names)
        remove_duplicate_source_owned_nested_stubs(path)
        remove_duplicate_inherent_methods(path)

    Path(args.conflicts).write_text("".join(conflict_lines))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
