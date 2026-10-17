#!/usr/bin/env python3
import argparse
import re
from collections import defaultdict
from functools import cache
from pathlib import Path

from rust_item_utils import impl_target_name, top_level_macro_items


# Private helper types can be representation dependencies of a public support
# carrier (for example an enum/state pair behind an Rc<RefCell<_>>).  They are
# top-level support items just like public facades and must participate in the
# same nominal dependency closure.  Visibility is widened to `pub(crate)` only
# when such an item is actually promoted to crate-root shared support.
TYPE_RE = re.compile(
    r"^\s*(?:(?:pub(?:\(crate\))?)\s+)?(struct|enum)\s+([A-Za-z_][A-Za-z0-9_]*)"
)
TRAIT_DECL_RE = re.compile(r"^\s*pub\s+trait\s+([A-Za-z_][A-Za-z0-9_]*)\b")
ALIAS_RE = re.compile(
    r"^\s*pub\s+type\s+([A-Za-z_][A-Za-z0-9_]*)"
    r"(?:\s*<[^=;]+>)?\s*=\s*(.+?)\s*;\s*$"
)
DERIVE_RE = re.compile(r"^\s*#\[derive")
INHERENT_IMPL_RE = re.compile(
    r"^\s*impl(?:\s*<.*>)?\s+(?:dyn\s+)?"
    r"([A-Za-z_][A-Za-z0-9_]*)(?:<[^>{}]*>)?\s*(?:\{|where\b)"
)
TRAIT_IMPL_RE = re.compile(
    r"^\s*impl(?:\s*<.*>)?\s+(.+?)\s+for\s+(.+?)\s*(?:\{|where\b)"
)
FROM_TRAIT_RE = re.compile(
    r"^From<([A-Za-z_][A-Za-z0-9_]*)(?:<.*>)?>$"
)
FN_RE = re.compile(
    r"^\s*(?:pub(?:\(crate\))?\s+)?(?:const\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\b"
)
TOP_LEVEL_VALUE_RE = re.compile(
    r"^\s*(?:pub(?:\(crate\))?\s+)?(?:const|static(?:\s+mut)?)\s+"
    r"([A-Za-z_][A-Za-z0-9_]*)\b"
)
FUNCTION_REFERENCE_RE = re.compile(
    r"\b([A-Za-z_][A-Za-z0-9_]*)\s*(?:<|\()"
)
IDENTIFIER_RE = re.compile(r"\b[A-Za-z_][A-Za-z0-9_]*\b")
# A `const fn` is a method, not an associated constant.  Keeping `fn` out of
# this match matters because constants are copied as a single line while
# methods are copied through their brace-balanced body.
CONST_RE = re.compile(
    r"^\s*pub\s+const\s+(?!fn\b)([A-Za-z_][A-Za-z0-9_]*)\b"
)
STATIC_METHOD_CALL_RE_TEMPLATE = r"\b{}\s*::\s*{}\s*(?:<|\()"
INSTANCE_METHOD_CALL_RE_TEMPLATE = r"\.\s*{}\s*(?:<|\()"


def impl_target_wraps_type(raw_target: str, type_name: str) -> bool:
    """Whether an Rc/Arc impl target is owned by the wrapped source type.

    The outer target name is useful for shared-facade coherence, but it loses
    ownership information for generated polymorphic adapters such as
    `impl Trait for Rc<ConcreteSource>`.  Those impls belong beside the
    concrete source declaration just as `impl Trait for ConcreteSource` does.
    """
    target = raw_target.strip()
    target = target.removeprefix("&mut ").removeprefix("&")
    target = target.removeprefix("dyn ")
    if "<" not in target or not target.endswith(">"):
        return False
    outer, inner = target.split("<", 1)
    if outer.strip().rsplit("::", 1)[-1] not in {"Rc", "Arc"}:
        return False
    inner = inner[:-1].strip()
    return impl_target_name(inner) == type_name and "<" not in inner


def trait_impl_name(raw_trait: str) -> str:
    trait = raw_trait.strip()
    if trait.startswith("From<"):
        return "From"
    trait = trait.split("<", 1)[0]
    return trait.rsplit("::", 1)[-1]


def boxed_dyn_trait_target_name(raw_target: str):
    target = raw_target.strip()
    match = re.search(
        r"(?:^|::)Box\s*<\s*dyn\s+([A-Za-z_][A-Za-z0-9_]*)",
        target,
    )
    return match.group(1) if match else None


def is_boxed_trait_conversion(raw_trait: str, raw_target: str) -> bool:
    source = re.fullmatch(r"(?:[A-Za-z_][A-Za-z0-9_]*::)*From<(.*)>", raw_trait.strip())
    return bool(
        source
        and boxed_dyn_trait_target_name(source.group(1))
        and boxed_dyn_trait_target_name(raw_target)
    )


@cache
def private_support_types(path: Path) -> set[str]:
    """Internal representation carriers can follow the primary declaration."""
    names = set()
    depth = 0
    for line in path.read_text().splitlines(keepends=True):
        if depth == 0 and not re.match(r"^\s*pub\s+", line):
            if match := TYPE_RE.match(line):
                names.add(match.group(2))
        depth += brace_delta(line)
    return names


def is_char_literal_start(line: str, index: int) -> bool:
    next_index = index + 1
    if next_index >= len(line):
        return False
    if line[next_index] == "\\":
        return index + 3 < len(line) and line[index + 3] == "'"
    return index + 2 < len(line) and line[index + 2] == "'"


@cache
def brace_delta(line: str) -> int:
    """Return the structural brace delta for one generated Rust line.

    Assembly deliberately makes several independent top-level-item passes over
    the raw modules. Those modules also share a large generated support
    prelude, so the same line was previously scanned character-by-character
    millions of times. The result depends only on immutable line text; caching
    it preserves every caller's parser state while reducing the repeated work
    to one scan per distinct generated line.
    """
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
            if ch == "'" and is_char_literal_start(line, index):
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
    if suffix.startswith("_"):
        suffix = suffix[1:]
    return bool(suffix) and suffix[0].isupper()


def remove_duplicate_top_level_aliases(path: Path) -> None:
    """Keep the last declaration of an identical local compatibility alias.

    Nested source lowering can emit an early facade alias and then repeat the
    same alias beside the authoritative nested declaration.  The latter owns
    the source identity; retaining both is always an invalid Rust namespace.
    """
    lines = path.read_text().splitlines(keepends=True)
    aliases = defaultdict(list)
    depth = 0
    for index, line in enumerate(lines):
        if depth == 0 and (match := ALIAS_RE.match(line)):
            aliases[(match.group(1), impl_target_name(match.group(2)))].append(index)
        depth += brace_delta(line)
    remove = {
        index
        for indices in aliases.values()
        for index in indices[:-1]
    }
    if remove:
        path.write_text("".join(line for index, line in enumerate(lines) if index not in remove))


def item_end(lines: list[str], start: int) -> int:
    depth = 0
    saw_body = False
    for index in range(start, len(lines)):
        if "{" in lines[index]:
            saw_body = True
        depth += brace_delta(lines[index])
        if saw_body and depth <= 0:
            return index + 1
        if not saw_body and ";" in lines[index]:
            return index + 1
    return len(lines)


def impl_header_text(lines: list[str], start: int) -> str:
    """Join a top-level impl header that rustfmt split before its body.

    Generated support contains both ``impl Trait for Type`` followed by a
    multiline ``where`` clause and trait/target splits where ``for Type`` is
    on the next line.  Reconciliation must classify the whole header before
    deciding ownership; treating only its first line silently drops coherent
    implementations from the assembled crate.
    """
    first = lines[start]
    if not first.lstrip().startswith("impl"):
        return first
    fragments = []
    for index in range(start, min(len(lines), start + 32)):
        fragment = lines[index].strip()
        if fragment:
            fragments.append(fragment)
        joined = " ".join(fragments)
        if "{" in fragment or ";" in fragment:
            return joined
        if re.search(r"\bwhere\b", fragment):
            return joined
    return " ".join(fragments)


def attached_start(lines: list[str], start: int) -> int:
    index = start
    while index > 0 and DERIVE_RE.match(lines[index - 1]):
        index -= 1
    return index


@cache
def prelude_support_types(path: Path) -> set[str]:
    source_type = source_type_for(path)
    source_owned_nested_types = prelude_source_owned_nested_types(path)
    lines = path.read_text().splitlines(keepends=True)
    names = set()
    depth = 0
    for index, line in enumerate(lines):
        if depth == 0:
            match = TYPE_RE.match(line)
            if match:
                name = match.group(2)
                if name == source_type:
                    break
                if name in source_owned_nested_types:
                    depth += brace_delta(line)
                    continue
                names.add(name)
            trait_match = TRAIT_DECL_RE.match(line)
            if trait_match:
                names.add(trait_match.group(1))
        depth += brace_delta(line)
    return names


@cache
def prelude_aliases(path: Path) -> list[tuple[str, str]]:
    source_type = source_type_for(path)
    lines = path.read_text().splitlines(keepends=True)
    aliases = []
    depth = 0
    for index, line in enumerate(lines):
        if depth == 0:
            type_match = TYPE_RE.match(line)
            if type_match and type_match.group(2) == source_type:
                break
            alias_match = ALIAS_RE.match(line)
            if alias_match:
                aliases.append(
                    (alias_match.group(1), impl_target_name(alias_match.group(2)))
                )
        depth += brace_delta(line)
    return aliases


@cache
def prelude_source_owned_nested_types(path: Path) -> set[str]:
    # A concatenated Java name alone is not proof of nesting: for example,
    # java.io.SequenceInputStream is unrelated to a source class named
    # Sequence.  Generated nested classes have both a prelude facade and the
    # concrete declaration before their enclosing primary type, which gives us
    # a structural signal instead of relying on a name prefix alone.
    nested = set(duplicate_source_owned_nested_last_decl_starts(path))
    source_type = source_type_for(path)
    lines = path.read_text().splitlines(keepends=True)
    declared = set()
    aliases = []
    depth = 0
    for line in lines:
        if depth == 0:
            type_match = TYPE_RE.match(line)
            if type_match:
                name = type_match.group(2)
                if name == source_type:
                    break
                declared.add(name)
            if alias_match := ALIAS_RE.match(line):
                aliases.append(
                    (alias_match.group(1), impl_target_name(alias_match.group(2)))
                )
        depth += brace_delta(line)
    # A flattened declaration plus its simple-name alias is the emitted form
    # of a Java nested source type. The prefix alone is insufficient evidence
    # (`SequenceInputStream` is not nested in `Sequence`), while this pair is
    # produced deliberately by nested-type lowering.
    nested.update(
        target
        for _alias, target in aliases
        if target in declared and target.startswith(f"{source_type}_")
    )
    # Java permits additional package-private top-level declarations in the
    # same file. The translator emits those concrete source declarations after
    # the filename's primary type, whereas synthesized support facades live in
    # the prelude. Treat every post-primary declaration as source-owned so a
    # duplicate facade in another module cannot promote only the declaration
    # while discarding wrapper coherence impls such as `Trait for Arc<Helper>`.
    after_primary = False
    depth = 0
    for line in lines:
        if depth == 0:
            type_match = TYPE_RE.match(line)
            trait_match = TRAIT_DECL_RE.match(line)
            if type_match and type_match.group(2) == source_type:
                after_primary = True
            elif after_primary and type_match:
                nested.add(type_match.group(2))
            elif after_primary and trait_match:
                nested.add(trait_match.group(1))
        depth += brace_delta(line)
    return nested


@cache
def prelude_from_impls(path: Path) -> list[tuple[str, str]]:
    source_type = source_type_for(path)
    source_owned_nested_types = prelude_source_owned_nested_types(path)
    lines = path.read_text().splitlines(keepends=True)
    impls = []
    depth = 0
    for index, line in enumerate(lines):
        if depth == 0:
            type_match = TYPE_RE.match(line)
            if type_match and type_match.group(2) == source_type:
                break
            trait_match = TRAIT_IMPL_RE.match(impl_header_text(lines, index))
            if trait_match:
                from_match = FROM_TRAIT_RE.match(trait_match.group(1).strip())
                if from_match:
                    from_name = from_match.group(1)
                    to_name = impl_target_name(trait_match.group(2))
                    if not (
                        from_name in source_owned_nested_types
                        or to_name in source_owned_nested_types
                    ):
                        impls.append((from_name, to_name))
        depth += brace_delta(line)
    return impls


@cache
def prelude_trait_impls(path: Path) -> list[tuple[str, str]]:
    source_type = source_type_for(path)
    lines = path.read_text().splitlines(keepends=True)
    impls = []
    declared_traits = set()
    depth = 0
    for index, line in enumerate(lines):
        if depth == 0:
            type_match = TYPE_RE.match(line)
            if type_match and type_match.group(2) == source_type:
                break
            trait_decl_match = TRAIT_DECL_RE.match(line)
            if trait_decl_match:
                declared_traits.add(trait_decl_match.group(1))
            trait_match = TRAIT_IMPL_RE.match(impl_header_text(lines, index))
            if trait_match and trait_impl_name(trait_match.group(1)) in declared_traits:
                impls.append(
                    (
                        trait_impl_name(trait_match.group(1)),
                        impl_target_name(trait_match.group(2)),
                    )
                )
        depth += brace_delta(line)
    return impls


@cache
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


@cache
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


def parse_items(
    path: Path,
    selected: set[str],
    known_source_owned_nested_types: set[str] | None = None,
) -> list[dict]:
    source_type = source_type_for(path)
    source_owned_nested_types = (
        prelude_source_owned_nested_types(path)
        if known_source_owned_nested_types is None
        else known_source_owned_nested_types
    )
    lines = path.read_text().splitlines(keepends=True)
    items = []
    depth = 0
    index = 0
    after_primary = False
    while index < len(lines):
        line = lines[index]
        if depth == 0:
            type_match = TYPE_RE.match(line)
            trait_decl_match = TRAIT_DECL_RE.match(line)
            if type_match and type_match.group(2) == source_type:
                after_primary = True
            if type_match and type_match.group(2) in selected:
                name = type_match.group(2)
                start = attached_start(lines, index)
                end = item_end(lines, index)
                items.append(
                    {
                        "kind": "decl",
                        "name": name,
                        "start": start,
                        "end": end,
                        "text": "".join(lines[start:end]),
                        "prelude": not after_primary,
                        "source_owned_nested": name in source_owned_nested_types,
                    }
                )
                index = end
                continue
            if trait_decl_match and trait_decl_match.group(1) in selected:
                start = attached_start(lines, index)
                end = item_end(lines, index)
                items.append(
                    {
                        "kind": "decl",
                        "name": trait_decl_match.group(1),
                        "start": start,
                        "end": end,
                        "text": "".join(lines[start:end]),
                        "prelude": not after_primary,
                    }
                )
                index = end
                continue

            alias_match = ALIAS_RE.match(line)
            alias_target = (
                impl_target_name(alias_match.group(2)) if alias_match else None
            )
            if alias_match and (
                alias_match.group(1) in selected or alias_target in selected
            ):
                items.append(
                    {
                        "kind": "alias",
                        "name": alias_match.group(1),
                        "target": alias_target,
                        "start": index,
                        "end": index + 1,
                        "text": line,
                        "prelude": not after_primary,
                        "source_owned_nested": alias_target
                        in source_owned_nested_types,
                    }
                )
                index += 1
                continue

            header = impl_header_text(lines, index)
            trait_match = TRAIT_IMPL_RE.match(header)
            if trait_match and (
                impl_target_name(trait_match.group(2)) in selected
                or boxed_dyn_trait_target_name(trait_match.group(2)) in selected
                or trait_impl_name(trait_match.group(1)) in selected
                or (
                    (from_match := FROM_TRAIT_RE.match(trait_match.group(1).strip()))
                    and impl_target_name(from_match.group(1)) in selected
                )
                or (
                    is_boxed_trait_conversion(trait_match.group(1), trait_match.group(2))
                    and referenced_nominal_names(
                        ["".join(lines[index:item_end(lines, index)])], selected
                    )
                )
            ):
                raw_trait = trait_match.group(1).strip()
                raw_target = trait_match.group(2).strip()
                target_name = impl_target_name(raw_target)
                from_match = FROM_TRAIT_RE.match(raw_trait)
                source_owned_wrapper = impl_target_wraps_type(
                    raw_target, source_type
                ) or any(
                    impl_target_wraps_type(raw_target, owned_type)
                    for owned_type in source_owned_nested_types
                )
                source_owned_nested = (
                    target_name in source_owned_nested_types
                    or trait_impl_name(raw_trait) in source_owned_nested_types
                    or (
                        from_match is not None
                        and impl_target_name(from_match.group(1))
                        in source_owned_nested_types
                    )
                )
                if from_match and (
                    impl_target_name(from_match.group(1)) in source_owned_nested_types
                    or target_name in source_owned_nested_types
                ):
                    depth += brace_delta(line)
                    index += 1
                    continue
                end = item_end(lines, index)
                items.append(
                    {
                        "kind": "trait_impl",
                        "name": target_name,
                        "target": raw_target,
                        "trait": trait_impl_name(raw_trait),
                        "trait_raw": raw_trait,
                        "start": index,
                        "end": end,
                        "text": "".join(lines[index:end]),
                        "prelude": not after_primary,
                        "source_owned_nested": source_owned_nested,
                        "source_owned_wrapper": source_owned_wrapper,
                    }
                )
                index = end
                continue

            inherent_match = INHERENT_IMPL_RE.match(header)
            if inherent_match and inherent_match.group(1) in selected:
                name = inherent_match.group(1)
                end = item_end(lines, index)
                items.append(
                    {
                        "kind": "inherent_impl",
                        "name": name,
                        "start": index,
                        "end": end,
                        "text": "".join(lines[index:end]),
                        "prelude": not after_primary,
                        "source_owned_nested": name in source_owned_nested_types,
                    }
                )
                index = end
                continue

        depth += brace_delta(line)
        index += 1
    return items


def select_catalog_items(items: list[dict], selected: set[str]) -> list[dict]:
    """Filter an all-name item catalog without reparsing the Rust module."""
    result = []
    for item in items:
        kind = item["kind"]
        include = item["name"] in selected
        if kind == "alias":
            include = include or item["target"] in selected
        elif kind == "trait_impl":
            include = include or item.get("trait") in selected
            include = include or boxed_dyn_trait_target_name(
                item.get("target", "")
            ) in selected
            from_match = FROM_TRAIT_RE.match(item.get("trait_raw", ""))
            include = include or (
                from_match is not None
                and impl_target_name(from_match.group(1)) in selected
            )
            # The private adapter and the conversion that constructs it form
            # one ownership group; splitting them also changes tuple-field
            # constructor privacy when the adapter moves to crate root.
            include = include or (
                is_boxed_trait_conversion(item.get("trait_raw", ""), item.get("target", ""))
                and bool(referenced_nominal_names([item["text"]], selected))
            )
        if include:
            result.append(item)
    return result


def parse_shared_items_from_text(text: str, selected: set[str]) -> list[dict]:
    lines = text.splitlines(keepends=True)
    items = []
    depth = 0
    index = 0
    while index < len(lines):
        line = lines[index]
        if depth == 0:
            type_match = TYPE_RE.match(line)
            trait_decl_match = TRAIT_DECL_RE.match(line)
            if type_match and type_match.group(2) in selected:
                name = type_match.group(2)
                start = attached_start(lines, index)
                end = item_end(lines, index)
                items.append(
                    {
                        "kind": "decl",
                        "name": name,
                        "start": start,
                        "end": end,
                        "text": "".join(lines[start:end]),
                        "prelude": True,
                    }
                )
                index = end
                continue
            if trait_decl_match and trait_decl_match.group(1) in selected:
                start = attached_start(lines, index)
                end = item_end(lines, index)
                items.append(
                    {
                        "kind": "decl",
                        "name": trait_decl_match.group(1),
                        "start": start,
                        "end": end,
                        "text": "".join(lines[start:end]),
                        "prelude": True,
                    }
                )
                index = end
                continue

            alias_match = ALIAS_RE.match(line)
            alias_target = (
                impl_target_name(alias_match.group(2)) if alias_match else None
            )
            if alias_match and (
                alias_match.group(1) in selected or alias_target in selected
            ):
                items.append(
                    {
                        "kind": "alias",
                        "name": alias_match.group(1),
                        "target": alias_target,
                        "start": index,
                        "end": index + 1,
                        "text": line,
                        "prelude": True,
                    }
                )
                index += 1
                continue

            header = impl_header_text(lines, index)
            trait_match = TRAIT_IMPL_RE.match(header)
            if trait_match and (
                impl_target_name(trait_match.group(2)) in selected
                or boxed_dyn_trait_target_name(trait_match.group(2)) in selected
                or trait_impl_name(trait_match.group(1)) in selected
                or (
                    (from_match := FROM_TRAIT_RE.match(trait_match.group(1).strip()))
                    and impl_target_name(from_match.group(1)) in selected
                )
            ):
                raw_trait = trait_match.group(1).strip()
                raw_target = trait_match.group(2).strip()
                target_name = impl_target_name(raw_target)
                end = item_end(lines, index)
                items.append(
                    {
                        "kind": "trait_impl",
                        "name": target_name,
                        "target": raw_target,
                        "trait": trait_impl_name(raw_trait),
                        "trait_raw": raw_trait,
                        "start": index,
                        "end": end,
                        "text": "".join(lines[index:end]),
                        "prelude": True,
                    }
                )
                index = end
                continue

            inherent_match = INHERENT_IMPL_RE.match(header)
            if inherent_match and inherent_match.group(1) in selected:
                name = inherent_match.group(1)
                end = item_end(lines, index)
                items.append(
                    {
                        "kind": "inherent_impl",
                        "name": name,
                        "start": index,
                        "end": end,
                        "text": "".join(lines[index:end]),
                        "prelude": True,
                    }
                )
                index = end
                continue

        depth += brace_delta(line)
        index += 1
    return items


def parse_top_level_functions_from_text(text: str) -> list[dict]:
    lines = text.splitlines(keepends=True)
    items = []
    depth = 0
    index = 0
    while index < len(lines):
        line = lines[index]
        if depth == 0:
            fn_match = FN_RE.match(line)
            if fn_match:
                end = function_item_end(lines, index)
                items.append(
                    {
                        "kind": "function",
                        "name": fn_match.group(1),
                        "start": index,
                        "end": end,
                        "text": "".join(lines[index:end]),
                        "prelude": True,
                    }
                )
                index = end
                continue
        depth += brace_delta(line)
        index += 1
    return items


def parse_top_level_values_from_text(text: str) -> list[dict]:
    """Parse crate-level constants/statics, including multiline initializers."""
    lines = text.splitlines(keepends=True)
    items = []
    depth = 0
    index = 0
    while index < len(lines):
        line = lines[index]
        if depth == 0 and (value_match := TOP_LEVEL_VALUE_RE.match(line)):
            end = value_item_end(lines, index)
            items.append(
                {
                    "kind": "value",
                    "name": value_match.group(1),
                    "start": index,
                    "end": end,
                    "text": "".join(lines[index:end]),
                    "prelude": True,
                }
            )
            index = end
            continue
        depth += brace_delta(line)
        index += 1
    return items


@cache
def parse_top_level_functions(path: Path) -> list[dict]:
    return parse_top_level_functions_from_text(path.read_text())


@cache
def parse_top_level_values(path: Path) -> list[dict]:
    return parse_top_level_values_from_text(path.read_text())


@cache
def text_without_top_level_functions(path: Path) -> str:
    lines = path.read_text().splitlines(keepends=True)
    remove = [False] * len(lines)
    text = "".join(lines)
    for item in (
        parse_top_level_functions_from_text(text)
        + parse_top_level_values_from_text(text)
    ):
        for index in range(item["start"], item["end"]):
            remove[index] = True
    return "".join(line for index, line in enumerate(lines) if not remove[index])


def function_item_end(lines: list[str], start: int) -> int:
    depth = 0
    saw_body = False
    for index in range(start, len(lines)):
        if "{" in lines[index]:
            saw_body = True
        depth += brace_delta(lines[index])
        if saw_body and depth <= 0:
            return index + 1
        if not saw_body and ";" in lines[index]:
            return index + 1
    return len(lines)


def value_item_end(lines: list[str], start: int) -> int:
    depth = 0
    for index in range(start, len(lines)):
        depth += brace_delta(lines[index])
        if depth <= 0 and ";" in lines[index]:
            return index + 1
    return len(lines)


def referenced_function_names(text: str, candidate_names: set[str]) -> set[str]:
    # Scan the source once.  Compiling and searching one regex per candidate
    # made package assembly O(source bytes * facade names) and took many
    # minutes once a real corpus exposed hundreds of support functions. A
    # helper can be called normally, called with turbofish syntax, or passed
    # as a function value, so identifier intersection is the complete
    # dependency relation here.
    return candidate_names.intersection(IDENTIFIER_RE.findall(text))


def referenced_nominal_names(texts: list[str], candidate_names: set[str]) -> set[str]:
    # Candidate nominal names are Rust identifiers, so tokenizing identifiers
    # is equivalent to the old per-name word-boundary searches while changing
    # the dependency-closure pass from O(bytes * candidates) to O(bytes).
    referenced = set()
    for text in texts:
        referenced.update(IDENTIFIER_RE.findall(text))
    return candidate_names.intersection(referenced)


def top_level_function_dependency_closure(
    seed_texts: list[str], functions_by_name: dict[str, list[dict]]
) -> set[str]:
    candidate_names = set(functions_by_name)
    selected = set()
    pending_texts = list(seed_texts)
    while pending_texts:
        text = pending_texts.pop()
        for name in referenced_function_names(text, candidate_names):
            if name in selected:
                continue
            selected.add(name)
            pending_texts.extend(item["text"] for item in functions_by_name[name])
    return selected


def top_level_item_dependency_closure(
    seed_texts: list[str],
    functions_by_name: dict[str, list[dict]],
    values_by_name: dict[str, list[dict]],
) -> tuple[set[str], set[str]]:
    """Close dependencies across crate-level functions, constants and statics."""
    function_names = set(functions_by_name)
    value_names = set(values_by_name)
    selected_functions = set()
    selected_values = set()
    pending_texts = list(seed_texts)
    while pending_texts:
        text = pending_texts.pop()
        for name in referenced_function_names(text, function_names):
            if name in selected_functions:
                continue
            selected_functions.add(name)
            pending_texts.extend(item["text"] for item in functions_by_name[name])
        for name in referenced_nominal_names([text], value_names):
            if name in selected_values:
                continue
            selected_values.add(name)
            pending_texts.extend(item["text"] for item in values_by_name[name])
    return selected_functions, selected_values


def crate_visible_top_level_function(text: str) -> str:
    """Expose a moved private helper to sibling generated modules.

    Facade helpers start life in each generated module's prelude. Once the
    facade merger moves one to crate-root shared support, callers reside in a
    different Rust module, so preserving private visibility changes its
    meaning. `pub(crate)` is the narrow visibility that preserves the
    original package-wide generated-code contract.
    """
    if re.match(r"^\s*pub(?:\(crate\))?\s+", text):
        return text
    return re.sub(
        r"^(\s*)((?:const\s+)?fn\b)",
        r"\1pub(crate) \2",
        text,
        count=1,
    )


def crate_visible_top_level_value(text: str) -> str:
    if re.match(r"^\s*pub(?:\(crate\))?\s+", text):
        return text
    return re.sub(
        r"^(\s*)((?:const|static(?:\s+mut)?)\b)",
        r"\1pub(crate) \2",
        text,
        count=1,
    )


def crate_visible_top_level_type(text: str) -> str:
    """Expose a promoted private nominal helper to generated child modules."""
    if re.search(r"(?m)^\s*pub(?:\(crate\))?\s+(?:struct|enum)\b", text):
        return text
    return re.sub(
        r"(?m)^(\s*)((?:struct|enum)\b)",
        r"\1pub(crate) \2",
        text,
        count=1,
    )


def promote_existing_top_level_functions(path: Path, names: set[str]) -> None:
    if not names or not path.exists():
        return
    text = path.read_text()
    lines = text.splitlines(keepends=True)
    replacements = {
        item["start"]: (item["end"], crate_visible_top_level_function(item["text"]))
        for item in parse_top_level_functions_from_text(text)
        if item["name"] in names
    }
    if not replacements:
        return
    output = []
    index = 0
    while index < len(lines):
        replacement = replacements.get(index)
        if replacement is None:
            output.append(lines[index])
            index += 1
            continue
        end, replacement_text = replacement
        output.append(replacement_text)
        index = end
    path.write_text("".join(output))


def remove_shared_items(path: Path, items: list[dict]) -> None:
    if not items or not path.exists():
        return
    lines = path.read_text().splitlines(keepends=True)
    remove = [False] * len(lines)
    for item in items:
        for index in range(item["start"], item["end"]):
            remove[index] = True
    path.write_text("".join(line for index, line in enumerate(lines) if not remove[index]))


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
                inherent_match = INHERENT_IMPL_RE.match(impl_header_text(lines, index))
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


def remove_cross_module_source_owned_stubs(
    path: Path,
    globally_source_owned_names: set[str],
    locally_source_owned_names: set[str],
) -> None:
    """Remove facade shadows of concrete source declarations owned elsewhere.

    A package-private top-level Java declaration is emitted after its file's
    primary declaration. Other translated files can still synthesize a
    prelude facade for that same nominal type. Leaving that facade module-local
    shadows the crate-visible concrete source type (and therefore hides its
    constructors and real methods) even though no Rust coherence conflict is
    reported. Source ownership is package-wide, so strip every prelude item
    for the identity outside the module that owns the concrete declaration.
    """
    names = globally_source_owned_names - locally_source_owned_names
    if not names or not path.exists():
        return
    lines = path.read_text().splitlines(keepends=True)
    remove = [False] * len(lines)
    for item in parse_items(path, names, locally_source_owned_names):
        if item["name"] not in names or not item.get("prelude", False):
            continue
        for index in range(item["start"], item["end"]):
            remove[index] = True
    path.write_text("".join(line for index, line in enumerate(lines) if not remove[index]))


def remove_duplicate_inherent_methods(path: Path) -> None:
    lines = path.read_text().splitlines(keepends=True)
    remove = [False] * len(lines)
    depth = 0
    index = 0
    while index < len(lines):
        line = lines[index]
        if depth == 0 and (
            impl_match := INHERENT_IMPL_RE.match(impl_header_text(lines, index))
        ):
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
        depth = 0
        saw_body = False
        while index < len(lines) - 1:
            depth += brace_delta(lines[index])
            index += 1
            if "{" in lines[index - 1]:
                saw_body = True
            if saw_body and depth <= 0:
                break
            if not saw_body and ";" in lines[index - 1]:
                break
        if saw_body:
            methods.append((match.group(1), "".join(lines[start:index]).strip()))
    return methods


def extract_trait_methods(trait_text: str) -> list[tuple[str, str]]:
    lines = trait_text.splitlines(keepends=True)
    methods = []
    index = 1
    while index < max(1, len(lines) - 1):
        line = lines[index]
        match = FN_RE.match(line)
        if not match:
            index += 1
            continue
        start = index
        if "{" in line:
            depth = brace_delta(line)
            index += 1
            while index < len(lines) - 1 and depth > 0:
                depth += brace_delta(lines[index])
                index += 1
            methods.append((match.group(1), "".join(lines[start:index]).strip()))
            continue
        while index < len(lines) - 1:
            if ";" in lines[index]:
                index += 1
                break
            index += 1
        methods.append((match.group(1), "".join(lines[start:index]).strip()))
    return methods


def merge_trait_declarations(name: str, declarations: list[str]) -> str:
    unique_decls = []
    seen_decls = set()
    for text in declarations:
        if text not in seen_decls:
            unique_decls.append(text)
            seen_decls.add(text)
    if len(unique_decls) <= 1:
        return unique_decls[0]

    methods_by_name = defaultdict(list)
    for text in unique_decls:
        for method_name, method_text in extract_trait_methods(text):
            methods_by_name[method_name].append(method_text)
    if not methods_by_name:
        return unique_decls[0]

    header = unique_decls[0].split("{", 1)[0].rstrip()
    lines = [f"{header} {{"]
    for method_name in sorted(methods_by_name):
        variants = []
        seen_methods = set()
        for text in methods_by_name[method_name]:
            if text not in seen_methods:
                variants.append(text)
                seen_methods.add(text)
        selected_method = sorted(variants)[0]
        lines.append("    " + selected_method.replace("\n", "\n    ").rstrip())
    lines.append("}")
    return "\n".join(lines)


def extract_consts(impl_text: str) -> list[tuple[str, str]]:
    lines = impl_text.splitlines(keepends=True)
    consts = []
    for line in lines[1 : max(1, len(lines) - 1)]:
        match = CONST_RE.match(line)
        if match:
            consts.append((match.group(1), line.strip()))
    return consts


def normalize_const_for_impl_target(
    impl_target: str, const_text: str, alias_targets: dict[str, str]
) -> str:
    match = re.match(
        r"^(pub\s+const\s+[A-Za-z_][A-Za-z0-9_]*\s*:\s*)"
        r"([A-Za-z_][A-Za-z0-9_]*)"
        r"(\s*=\s*)"
        r"([A-Za-z_][A-Za-z0-9_]*)"
        r"(\s*;)$",
        const_text,
    )
    if not match:
        return const_text
    prefix, const_type, equals, value, suffix = match.groups()
    if value == const_type and alias_targets.get(value) == impl_target:
        return f"{prefix}{const_type}{equals}{impl_target}{suffix}"
    return const_text


def builtin_inherent_consts(name: str) -> list[tuple[str, str]]:
    if name != "Level":
        return []
    return [
        ("DEBUG", "pub const DEBUG: Level = Level;"),
        ("ERROR", "pub const ERROR: Level = Level;"),
        ("INFO", "pub const INFO: Level = Level;"),
        ("SEVERE", "pub const SEVERE: Level = Level;"),
        ("WARNING", "pub const WARNING: Level = Level;"),
    ]


def builtin_from_impls(selected: set[str]) -> list[tuple[str, str, str]]:
    stream_sources_by_target = {
        "InputStream": [
            "BufferedInputStream",
            "ByteArrayInputStream",
            "DataInputStream",
            "FileInputStream",
            "FilterInputStream",
            "GZIPInputStream",
            "JavaIoInputStream",
            "ObjectInputStream",
            "PushbackInputStream",
            "SequenceInputStream",
        ],
        "OutputStream": [
            "BufferedOutputStream",
            "ByteArrayOutputStream",
            "DataOutputStream",
            "FileOutputStream",
            "FilterOutputStream",
            "GZIPOutputStream",
            "ObjectOutputStream",
            "PrintStream",
            "ZipOutputStream",
        ],
    }
    impls = []
    for to_name, from_names in stream_sources_by_target.items():
        if to_name not in selected:
            continue
        for from_name in from_names:
            if from_name in selected:
                impls.append(
                    (
                        from_name,
                        to_name,
                        f"impl From<{from_name}> for {to_name} {{\n"
                        f"    fn from(_value: {from_name}) -> Self {{ Default::default() }}\n"
                        f"}}",
                    )
                )
    return impls


def method_return_type(method_text: str) -> str:
    first_line = method_text.splitlines()[0] if method_text.splitlines() else method_text
    if " -> " not in first_line:
        return ""
    return first_line.split(" -> ", 1)[1].split(" {", 1)[0].strip()


def method_signature_text(method_text: str) -> str:
    before_body = method_text.split(" {", 1)[0]
    return " ".join(line.strip() for line in before_body.splitlines())


def method_abi_signature(method_text: str) -> str:
    """Canonical method declaration/implementation ABI without its body."""
    return method_signature_text(method_text).rstrip(";").strip()


def comma_separated_arity(text: str) -> int:
    if not text.strip():
        return 0
    depth = 0
    count = 1
    state = "code"
    index = 0
    while index < len(text):
        ch = text[index]
        next_ch = text[index + 1] if index + 1 < len(text) else ""
        if state == "string":
            if ch == "\\":
                index += 2
                continue
            if ch == '"':
                state = "code"
        elif state == "char":
            if ch == "\\":
                index += 2
                continue
            if ch == "'":
                state = "code"
        elif ch == '"':
            state = "string"
        elif ch == "'":
            state = "char"
        elif ch in "([{<":
            depth += 1
        elif ch in ")]}>":
            depth = max(0, depth - 1)
        elif ch == "," and depth == 0:
            count += 1
        index += 1
    return count


def method_parameter_count(method_text: str) -> int | None:
    signature = method_signature_text(method_text)
    open_paren = signature.find("(")
    if open_paren < 0:
        return None
    depth = 0
    for index in range(open_paren, len(signature)):
        if signature[index] == "(":
            depth += 1
        elif signature[index] == ")":
            depth -= 1
            if depth == 0:
                return comma_separated_arity(signature[open_paren + 1 : index])
    return None


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


def method_call_arity(method_text: str) -> int | None:
    count = method_parameter_count(method_text)
    if count is None:
        return None
    return count - 1 if method_receiver_kind(method_text) != "static" else count


def weak_method_variant(method_text: str) -> bool:
    return_type = method_return_type(method_text)
    first_line = method_text.splitlines()[0] if method_text.splitlines() else method_text
    defaulted_generic_returns = {
        match.group(1)
        for match in re.finditer(
            r"(?:<|,)\s*([A-Za-z_][A-Za-z0-9_]*)\s*:\s*Default\b",
            first_line,
        )
    }
    return (
        "<R: Default" in first_line
        or return_type in defaulted_generic_returns
        or return_type == "R"
        or return_type == "()"
        or return_type == "Box<dyn std::any::Any>"
    )


def method_generic_bound_count(method_text: str) -> int:
    """Count explicit generic bounds on the method itself.

    Duplicate generated facades can expose the same forwarding body with a
    summary-only bound set and with the complete source/inferred bound set.
    The forwarding method must retain the latter or its calls into concrete
    payloads are ill-typed. Colons introduce a first bound and plus signs add
    further bounds; Rust path separators are outside the method-generic head.
    """
    signature = method_signature_text(method_text)
    fn_match = re.search(r"\bfn\s+[A-Za-z_][A-Za-z0-9_]*\s*<", signature)
    if not fn_match:
        return 0
    start = signature.find("<", fn_match.start())
    depth = 0
    for index in range(start, len(signature)):
        ch = signature[index]
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
            if depth == 0:
                generic_head = signature[start + 1 : index]
                generic_head = generic_head.replace("::", "")
                return generic_head.count(":") + generic_head.count("+")
    return 0


def method_receiver_preference_rank(method_text: str, preferred_receiver_kinds: set[str]) -> int:
    receiver = method_receiver_kind(method_text)
    if receiver in preferred_receiver_kinds:
        return 0
    if preferred_receiver_kinds:
        return 1
    return 0


def default_receiver_rank(method_name: str, method_text: str) -> int:
    receiver = method_receiver_kind(method_text)
    if method_name == "to_string" and receiver == "static":
        return 0
    if method_name in {"new_builder", "get_instance"} and receiver == "static":
        return 0
    if receiver == "static":
        return 2
    return 1


def best_method_variant(
    method_name: str,
    variants: list[str],
    preferred_receiver_kinds: set[str],
    preferred_arities: set[int] | None = None,
    representation_family_variants: set[str] | None = None,
) -> str:
    preferred_arities = preferred_arities or set()
    representation_family_variants = representation_family_variants or set()
    return sorted(
        variants,
        key=lambda text: (
            method_call_arity(text) not in preferred_arities
            if preferred_arities
            else False,
            method_receiver_preference_rank(text, preferred_receiver_kinds),
            default_receiver_rank(method_name, text),
            text not in representation_family_variants
            if representation_family_variants
            else False,
            weak_method_variant(text),
            -method_generic_bound_count(text),
            method_return_type(text) == "",
            text,
        ),
    )[0]


def call_argument_count(text: str, open_paren: int) -> int | None:
    depth = 0
    state = "code"
    index = open_paren
    while index < len(text):
        ch = text[index]
        if state == "string":
            if ch == "\\":
                index += 2
                continue
            if ch == '"':
                state = "code"
        elif state == "char":
            if ch == "\\":
                index += 2
                continue
            if ch == "'":
                state = "code"
        elif ch == '"':
            state = "string"
        elif ch == "'":
            state = "char"
        elif ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
            if depth == 0:
                return comma_separated_arity(text[open_paren + 1 : index])
        index += 1
    return None


def method_call_arity_preferences(
    rust_paths: list[Path],
    selected: set[str],
    alias_targets: dict[str, str],
    item_catalogs: dict[Path, list[dict]] | None = None,
) -> dict[tuple[str, str], set[int]]:
    preferences = defaultdict(set)
    names_by_target = defaultdict(set)
    for name in selected:
        names_by_target[name].add(name)
    for alias, target in alias_targets.items():
        names_by_target[target].add(alias)
    path_texts = [path.read_text() for path in rust_paths]
    parsed_items = []
    for path in rust_paths:
        parsed_items.extend(
            select_catalog_items(item_catalogs[path], selected)
            if item_catalogs is not None
            else parse_items(path, selected)
        )
    method_names_by_target = defaultdict(set)
    for item in parsed_items:
        if item["kind"] == "inherent_impl":
            for method_name, _ in extract_methods(item["text"]):
                method_names_by_target[item["name"]].add(method_name)
    target_by_surface = {
        surface: target for target, surfaces in names_by_target.items() for surface in surfaces
    }
    static_call_re = re.compile(
        r"\b([A-Za-z_][A-Za-z0-9_]*)\s*::\s*([A-Za-z_][A-Za-z0-9_]*)\s*\("
    )
    for text in path_texts:
        for match in static_call_re.finditer(text):
            type_name, method_name = match.groups()
            target = target_by_surface.get(type_name)
            if target is None or method_name not in method_names_by_target.get(type_name, set()):
                continue
            arity = call_argument_count(text, match.end() - 1)
            if arity is not None:
                preferences[(target, method_name)].add(arity)
    return preferences


def method_call_receiver_preferences(
    rust_paths: list[Path],
    selected: set[str],
    alias_targets: dict[str, str],
    item_catalogs: dict[Path, list[dict]] | None = None,
) -> dict[tuple[str, str], set[str]]:
    preferences = defaultdict(set)
    names_by_target = defaultdict(set)
    for name in selected:
        names_by_target[name].add(name)
    for alias, target in alias_targets.items():
        names_by_target[target].add(alias)
    path_texts = [path.read_text() for path in rust_paths]
    parsed_items = []
    for path in rust_paths:
        parsed_items.extend(
            select_catalog_items(item_catalogs[path], selected)
            if item_catalogs is not None
            else parse_items(path, selected)
        )
    method_names_by_target = defaultdict(set)
    for item in parsed_items:
        if item["kind"] == "inherent_impl":
            for method_name, _ in extract_methods(item["text"]):
                method_names_by_target[item["name"]].add(method_name)
    target_by_surface = {
        surface: target for target, surfaces in names_by_target.items() for surface in surfaces
    }
    static_calls = set()
    instance_methods = set()
    static_call_re = re.compile(
        r"\b([A-Za-z_][A-Za-z0-9_]*)\s*::\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?:<|\()"
    )
    instance_call_re = re.compile(r"\.\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?:<|\()")
    for text in path_texts:
        static_calls.update(static_call_re.findall(text))
        instance_methods.update(instance_call_re.findall(text))
    for type_name, method_names in method_names_by_target.items():
        target = target_by_surface.get(type_name)
        if target is None:
            continue
        for method_name in method_names:
            if (type_name, method_name) in static_calls:
                preferences[(target, method_name)].add("static")
            if method_name in instance_methods:
                preferences[(target, method_name)].update(
                    {"self", "mut_self", "owned_self"}
                )
    return preferences


def remove_items(
    path: Path,
    items: list[dict],
    removal_only_names: set[str],
    keep_last_prelude_names: set[str],
    shared_identity_names: set[str],
) -> None:
    lines = path.read_text().splitlines(keepends=True)
    remove = [False] * len(lines)
    last_prelude_start_by_name = {
        item["name"]: item["start"]
        for item in items
        if item["name"] in keep_last_prelude_names and item.get("prelude", False)
    }
    for item in items:
        if item.get("source_owned_wrapper"):
            continue
        if item.get("source_owned_nested") and item["name"] not in shared_identity_names:
            continue
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


def restore_source_owned_wrapper_impls(
    generated_path: Path,
    merge_input_path: Path,
) -> None:
    """Restore source-owned Rc/Arc trait impls removed by an earlier pass.

    Package assembly deliberately keeps a raw pre-strip snapshot. An early
    support-deduplication pass can only see the filename's primary source type
    and may therefore remove `impl Trait for Rc<SecondarySource>` when `Trait`
    is shared. The merger has structural ownership information for every
    source declaration in the file, so recover those coherence impls here and
    leave ordinary shared-facade wrappers to the normal promotion logic.
    """
    if not merge_input_path.exists() or not generated_path.exists():
        return
    source_type = source_type_for(merge_input_path)
    source_owned = prelude_source_owned_nested_types(merge_input_path)
    lines = merge_input_path.read_text().splitlines(keepends=True)
    wrappers = []
    depth = 0
    index = 0
    while index < len(lines):
        line = lines[index]
        if depth == 0:
            trait_match = TRAIT_IMPL_RE.match(impl_header_text(lines, index))
            if trait_match:
                raw_target = trait_match.group(2).strip()
                if impl_target_wraps_type(raw_target, source_type) or any(
                    impl_target_wraps_type(raw_target, owned_type)
                    for owned_type in source_owned
                ):
                    end = item_end(lines, index)
                    wrappers.append("".join(lines[index:end]).strip())
                    index = end
                    continue
        depth += brace_delta(line)
        index += 1
    if not wrappers:
        return
    rendered = generated_path.read_text()
    missing = [wrapper for wrapper in wrappers if wrapper and wrapper not in rendered]
    if missing:
        generated_path.write_text(
            rendered.rstrip() + "\n\n" + "\n\n".join(missing) + "\n"
        )


def box_self_recursive_fields(name: str, text: str) -> str:
    field_ty = re.escape(name)
    direct_self_field = re.compile(
        rf"(?m)^(\s*pub\s+[A-Za-z_][A-Za-z0-9_]*\s*:\s*){field_ty}(\s*,\s*)$"
    )
    return direct_self_field.sub(rf"\1Box<{name}>\2", text)


def item_targets_name(item_name: str, target_name: str, alias_targets: dict[str, str]) -> bool:
    return item_name == target_name or alias_targets.get(item_name) == target_name


def derived_traits_for_decl(text: str) -> set[str]:
    traits = set()
    for match in re.finditer(r"#\[derive\(([^)]*)\)\]", text):
        for trait in match.group(1).split(","):
            trait = trait.strip()
            if trait:
                traits.add(trait)
    return traits


def trait_is_derived_by_decl(trait_name: str, derived_traits: set[str]) -> bool:
    short_name = trait_name.rsplit("::", 1)[-1]
    return trait_name in derived_traits or short_name in derived_traits


def preferred_decl_variant(variants: list[str]) -> tuple[str, str]:
    for text in variants:
        if re.search(r"(?m)^\s*pub\s+enum\s+", text):
            return text, "enum"
    for text in variants:
        if re.search(r"(?m)^\s*pub\s+trait\s+", text):
            # A source-summary interface is authoritative semantic evidence.
            # Other units that do not directly exercise the interface can
            # synthesize an opaque struct with the same name; selecting that
            # fallback makes every `dyn Interface` and generic bound invalid.
            return text, "source-interface-trait"
    scored = [(declaration_information_score(text), index, text) for index, text in enumerate(variants)]
    score, _, selected = max(scored, key=lambda item: (item[0], -item[1]))
    return selected, "most-informative" if score > 0 else "first"


def declaration_information_score(text: str) -> int:
    """Prefer declarations that retain representational state over opaque stubs."""
    struct = re.search(
        r"(?s)\bpub\s+struct\s+[A-Za-z_][A-Za-z0-9_]*(?:\s*<[^;{]+>)?\s*(.*)$",
        text.strip(),
    )
    if not struct:
        return 0
    tail = struct.group(1).lstrip()
    if tail.startswith(";"):
        return 1
    if tail.startswith("("):
        return 2 + tail.count(",")
    if tail.startswith("{"):
        body = tail[1 : tail.rfind("}") if "}" in tail else len(tail)]
        fields = len(re.findall(r"(?m)^\s*(?:pub\s+)?[A-Za-z_][A-Za-z0-9_]*\s*:", body))
        return 3 + fields
    return 0


def preferred_trait_impl_variant(variants: list[str]) -> tuple[str, str]:
    for text in variants:
        if 'write!(f, "")' not in text and "Default::default()" not in text:
            return text, "non-empty"
    return variants[0], "first"


def split_top_level_commas(text: str) -> list[str]:
    parts = []
    start = 0
    depth = 0
    for index, ch in enumerate(text):
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth = max(0, depth - 1)
        elif ch == "," and depth == 0:
            parts.append(text[start:index].strip())
            start = index + 1
    tail = text[start:].strip()
    if tail:
        parts.append(tail)
    return parts


def generic_arg_name(param: str) -> str:
    param = param.strip()
    if not param:
        return ""
    if param.startswith("const "):
        param = param[len("const ") :].strip()
    return re.split(r"\s*[:=]\s*", param, maxsplit=1)[0].strip()


def inherent_impl_header_for_decl(name: str, decl_text: str, selected_decl_is_trait: bool) -> str:
    if selected_decl_is_trait:
        return f"impl dyn {name}"
    generic_match = re.search(
        rf"(?m)^\s*pub\s+(?:struct|enum)\s+{re.escape(name)}\s*<([^>{{}}]*)>",
        decl_text,
    )
    if not generic_match:
        return f"impl {name}"
    impl_generics = generic_match.group(1).strip()
    generic_args = [
        arg
        for arg in (generic_arg_name(param) for param in split_top_level_commas(impl_generics))
        if arg
    ]
    if not generic_args:
        return f"impl {name}"
    # Defaults belong on a type declaration, never on an `impl` generic
    # parameter list.  Rebuilding an inherent impl from
    # `pub struct Comparator<T = Box<dyn Any>>` previously emitted the invalid
    # `impl<T = Box<dyn Any>> Comparator<T>`.
    return f"impl<{', '.join(generic_args)}> {name}<{', '.join(generic_args)}>"


def simple_rust_type_name(name: str) -> bool:
    return bool(re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", name))


def trait_impl_target_can_be_emitted_without_decl(
    target_name: str, impl_text: str
) -> bool:
    return (
        impl_text.lstrip().startswith("impl<")
        or target_name.startswith("Box<")
        or target_name.startswith("Option<")
        or target_name.startswith("Result<")
        or target_name.startswith("Vec<")
        or "::" in target_name
        or target_name.startswith("&")
        or target_name
        in {
            "bool",
            "char",
            "f32",
            "f64",
            "i8",
            "i16",
            "i32",
            "i64",
            "isize",
            "str",
            "String",
            "u8",
            "u16",
            "u32",
            "u64",
            "usize",
        }
    )


def read_existing_shared_types(path: Path) -> set[str]:
    if not path.exists():
        return set()
    return {
        line.strip()
        for line in path.read_text().splitlines()
        if line.strip()
    }


def declared_type_names_from_text(text: str) -> set[str]:
    names = set()
    depth = 0
    for line in text.splitlines():
        if depth == 0:
            if type_match := TYPE_RE.match(line):
                names.add(type_match.group(2))
            if trait_match := TRAIT_DECL_RE.match(line):
                names.add(trait_match.group(1))
        depth += brace_delta(line)
    return names


def from_impl_keys_from_text(text: str) -> set[tuple[str, str]]:
    keys = set()
    lines = text.splitlines(keepends=True)
    for index, line in enumerate(lines):
        trait_match = TRAIT_IMPL_RE.match(impl_header_text(lines, index))
        if not trait_match:
            continue
        from_match = FROM_TRAIT_RE.match(trait_match.group(1).strip())
        if from_match:
            keys.add((from_match.group(1), impl_target_name(trait_match.group(2))))
    return keys


def remove_protected_aliases_from_shared_support(path: Path, aliases: set[str]) -> None:
    if not aliases or not path.exists():
        return
    lines = path.read_text().splitlines(keepends=True)
    kept = []
    for line in lines:
        alias_match = ALIAS_RE.match(line)
        if alias_match and alias_match.group(1) in aliases:
            continue
        kept.append(line)
    path.write_text("".join(kept))


def remove_protected_names_from_shared_support(path: Path, names: set[str]) -> None:
    if not names or not path.exists():
        return
    lines = path.read_text().splitlines(keepends=True)
    remove = [False] * len(lines)
    index = 0
    while index < len(lines):
        line = lines[index]
        start = attached_start(lines, index)
        end = None
        type_match = TYPE_RE.match(line)
        trait_match = TRAIT_DECL_RE.match(line)
        header = impl_header_text(lines, index)
        inherent_match = INHERENT_IMPL_RE.match(header)
        impl_match = TRAIT_IMPL_RE.match(header)
        alias_match = ALIAS_RE.match(line)
        if type_match and type_match.group(2) in names:
            end = item_end(lines, index)
        elif trait_match and trait_match.group(1) in names:
            end = item_end(lines, index)
        elif inherent_match and inherent_match.group(1) in names:
            end = item_end(lines, index)
        elif impl_match:
            trait_name = impl_match.group(1).strip()
            target_name = impl_match.group(2)
            from_match = FROM_TRAIT_RE.match(trait_name)
            if target_name in names or (from_match and from_match.group(1) in names):
                end = item_end(lines, index)
        elif alias_match and (alias_match.group(1) in names or alias_match.group(2) in names):
            end = index + 1
        if end is not None:
            for remove_index in range(start, end):
                remove[remove_index] = True
            index = end
            continue
        index += 1
    path.write_text("".join(line for index, line in enumerate(lines) if not remove[index]))


def remove_protected_names_from_shared_types(path: Path, names: set[str]) -> None:
    if not names or not path.exists():
        return
    lines = path.read_text().splitlines(keepends=True)
    path.write_text("".join(line for line in lines if line.strip() not in names))


def append_missing_builtin_from_impls(
    shared_support_path: Path,
    shared_type_universe: set[str],
    primary_output_types: set[str],
    existing_from_impl_keys: set[tuple[str, str]],
) -> None:
    chunks = []
    for from_name, to_name, text in builtin_from_impls(shared_type_universe):
        if (
            from_name in primary_output_types
            or to_name in primary_output_types
            or (from_name, to_name) in existing_from_impl_keys
        ):
            continue
        chunks.append("\n" + text.rstrip() + "\n")
    if chunks:
        with shared_support_path.open("a") as shared:
            shared.write("".join(chunks))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cleanup-module")
    parser.add_argument("--generated-dir")
    parser.add_argument("--merge-input-dir")
    parser.add_argument("--shared-support")
    parser.add_argument("--shared-types")
    parser.add_argument("--conflicts")
    args = parser.parse_args()

    if args.cleanup_module:
        path = Path(args.cleanup_module)
        remove_duplicate_source_owned_nested_stubs(path)
        remove_duplicate_top_level_aliases(path)
        remove_duplicate_inherent_methods(path)
        return 0
    for required in ["generated_dir", "shared_support", "shared_types", "conflicts"]:
        if getattr(args, required) is None:
            parser.error(f"--{required.replace('_', '-')} is required")

    generated_dir = Path(args.generated_dir)
    rust_paths = sorted(generated_dir.glob("*.rs"))
    merge_input_dir = Path(args.merge_input_dir) if args.merge_input_dir else generated_dir
    merge_input_paths = sorted(merge_input_dir.glob("*.rs"))
    shared_support_path = Path(args.shared_support)
    existing_shared_text = (
        shared_support_path.read_text() if shared_support_path.exists() else ""
    )
    existing_shared_declared_type_names = declared_type_names_from_text(
        existing_shared_text
    )
    existing_shared_types = (
        read_existing_shared_types(Path(args.shared_types))
        | existing_shared_declared_type_names
    )
    existing_shared_from_impl_keys = from_impl_keys_from_text(existing_shared_text)
    existing_shared_alias_names = {
        match.group(1)
        for line in existing_shared_text.splitlines()
        if (match := ALIAS_RE.match(line))
    }
    existing_shared_function_names = {
        item["name"] for item in parse_top_level_functions_from_text(existing_shared_text)
    }
    existing_shared_value_names = {
        item["name"] for item in parse_top_level_values_from_text(existing_shared_text)
    }
    def promotable_shared_name(name: str) -> bool:
        # Dependency closure may mention qualified external targets such as
        # std::path::PathBuf.  Only promote identities for which this corpus
        # actually contains a declaration; external path segments must never
        # become synthetic crate imports.
        return name not in existing_shared_types and name in candidates_by_type

    removal_only_names = set(existing_shared_alias_names)
    primary_output_types = {source_type_for(path) for path in rust_paths}
    source_shadowing_aliases = set(existing_shared_alias_names) & primary_output_types
    source_shadowing_declared_types = (
        existing_shared_declared_type_names & primary_output_types
    )
    candidates_by_type = defaultdict(list)
    alias_candidates_by_name = defaultdict(list)
    top_level_functions_by_name = defaultdict(list)
    top_level_values_by_name = defaultdict(list)
    macro_definitions_by_name = defaultdict(list)
    macro_invocations_by_name = defaultdict(list)
    for path in merge_input_paths:
        for name in prelude_support_types(path) | private_support_types(path):
            candidates_by_type[name].append(path)
        for alias, target in prelude_aliases(path):
            alias_candidates_by_name[alias].append((path, target))
        source_shadowing_aliases.update(
            alias
            for alias, _ in prelude_aliases(path)
            if alias in primary_output_types
        )
        for item in parse_top_level_functions(path):
            item_with_path = dict(item)
            item_with_path["path"] = path
            top_level_functions_by_name[item["name"]].append(item_with_path)
        for item in parse_top_level_values(path):
            item_with_path = dict(item)
            item_with_path["path"] = path
            top_level_values_by_name[item["name"]].append(item_with_path)
        for item in top_level_macro_items(path.read_text()):
            item_with_path = dict(item)
            item_with_path["path"] = path
            destination = (
                macro_definitions_by_name
                if item["kind"] == "macro_definition"
                else macro_invocations_by_name
            )
            destination[item["name"]].append(item_with_path)

    initially_selected_functions = top_level_function_dependency_closure(
        [existing_shared_text]
        + [text_without_top_level_functions(path) for path in rust_paths],
        top_level_functions_by_name,
    )

    selected = {
        name
        for name, paths in candidates_by_type.items()
        if len(paths) > 1
        and name not in primary_output_types
        and promotable_shared_name(name)
        and name not in {"Box", "String", "Vec", "Option", "Result"}
    }
    # A support identity can already be declared by the package assembler while
    # its coherent trait implementations still live in every per-file prelude.
    # Select those identities for impl extraction without re-emitting their
    # declarations or inherent methods.
    selected.update(
        name
        for name in existing_shared_declared_type_names
        if len(candidates_by_type.get(name, ())) > 1
        and name not in primary_output_types
    )
    # The assembler can seed a shared-type manifest before it has copied the
    # corresponding declaration into shared support. Those names are stripped
    # from per-file modules before this merger runs, so treating only repeated
    # or already-declared names as promotable silently discards their impls.
    # A manifest entry is an explicit cross-module ownership decision: restore
    # its complete declaration/impl surface from any raw merge input, even in a
    # one-file focused package.
    selected.update(
        name
        for name in existing_shared_types
        if candidates_by_type.get(name) and name not in primary_output_types
    )
    keep_last_prelude_names = set()
    protected_source_owned_nested_names = set()
    structural_source_owned_nested_names = set()
    for path in merge_input_paths:
        removal_only_names.update(prelude_types_redeclared_after_primary(path))
        # Duplicate declarations within one prelude are a local restoration
        # artifact only when that identity occurs in no sibling module. If the
        # same support type is present across modules, crate-wide coherence
        # wins: merge one declaration/impl surface and remove every local copy.
        keep_last_prelude_names.update(
            name
            for name in duplicate_prelude_types_before_primary(path)
            if len(candidates_by_type.get(name, ())) <= 1
        )
        path_support_types = prelude_support_types(path)
        path_structural_nested_names = {
            name
            for name in prelude_source_owned_nested_types(path)
            if name not in primary_output_types
        }
        structural_source_owned_nested_names.update(path_structural_nested_names)
        protected_source_owned_nested_names.update(path_structural_nested_names)
        # A module-local compatibility alias must not let an already shared
        # identity absorb the alias target's richer source-owned declaration.
        # For example, `type OutputStream = Base64OutputStream` is a fallback
        # view of the local payload when `OutputStream` is already crate-wide;
        # it is not evidence that `Base64OutputStream` itself is shared facade
        # state. Keep the target in its source module and discard only the
        # colliding alias.
        protected_source_owned_nested_names.update(
            target
            for alias, target in prelude_aliases(path)
            if alias in existing_shared_types
            and target in path_support_types
            and target not in primary_output_types
        )
    removal_only_names.update(keep_last_prelude_names)
    removal_only_names.update(protected_source_owned_nested_names)
    source_owned_nested_alias_names = {
        alias
        for alias, variants in alias_candidates_by_name.items()
        if any(
            target in structural_source_owned_nested_names
            for _path, target in variants
        )
    }
    # A nested Java leaf alias is useful inside its declaring output module,
    # but it must not replace an already shared crate identity with the same
    # name.  `PackedCoordinateSequence.Double`, for example, may be spelled
    # `Double` lexically in that source file while unrelated modules still
    # require the shared `java.lang.Double` facade.
    source_owned_nested_alias_shared_collisions = (
        source_owned_nested_alias_names
        & (existing_shared_types | set(candidates_by_type))
    )
    # The structural generic alias belongs beside its nested declaration.  A
    # stale assembly seed with the same simple name must be removed from
    # shared support, but must not cause the authoritative module alias to be
    # stripped as shared facade state.
    removal_only_names.difference_update(source_owned_nested_alias_names)
    # A qualified-name compatibility alias is only a fallback while its Java
    # declaration is external.  Once that declaration is translated in this
    # package, the concrete source type is authoritative.  Keep the alias in
    # the removal set so it is stripped from every prelude, but never allow it
    # to replace or absorb the source declaration itself.
    selected.update(source_shadowing_aliases)
    removal_only_names.update(source_shadowing_aliases)
    selected.update(removal_only_names - primary_output_types)
    promoted_aliases = set()
    protected_shared_aliases = {
        alias
        for alias in existing_shared_alias_names
        if alias in existing_shared_types and alias not in primary_output_types
    }
    protected_shared_aliases.update(source_owned_nested_alias_shared_collisions)
    for path in merge_input_paths:
        for alias, target in prelude_aliases(path):
            if (
                alias in existing_shared_types
                or alias in candidates_by_type
            ) and alias not in primary_output_types:
                protected_shared_aliases.add(alias)
                continue
            if target in protected_source_owned_nested_names:
                continue
            alias_variants = alias_candidates_by_name[alias]
            if (
                len({path for path, _target in alias_variants}) > 1
                and len({candidate_target for _path, candidate_target in alias_variants}) == 1
                and alias not in primary_output_types
            ):
                # Repeated generic aliases (for example Properties =
                # HashMap<String, String>) are shared facade identities too.
                # They have no struct/enum declaration, so nominal candidate
                # discovery cannot otherwise make their extension-trait impls
                # coherent at crate root.
                promoted_aliases.add(alias)
                continue
            if (
                target in selected
                and target not in protected_source_owned_nested_names
                and alias not in primary_output_types
            ):
                promoted_aliases.add(alias)
    selected.update(promoted_aliases)
    removal_only_names.update(promoted_aliases)
    selected.update(protected_shared_aliases)
    removal_only_names.update(
        protected_shared_aliases - source_owned_nested_alias_shared_collisions
    )

    # Build each raw module's top-level catalog once. Dependency closure grows
    # `selected` monotonically; reparsing every byte for every round made this
    # phase quadratic in corpus size and dependency depth.
    catalog_names = set(candidates_by_type) | set(existing_shared_types)
    catalog_names.update(alias_candidates_by_name)
    catalog_names.update(
        target
        for variants in alias_candidates_by_name.values()
        for _path, target in variants
    )
    merge_item_catalogs = {
        path: parse_items(path, catalog_names) for path in merge_input_paths
    }

    # A boxed interface conversion may own a private adapter found in just
    # one source unit. Duplication is not evidence of whether it is required.
    # Seed its nominal dependencies, then reuse the ordinary support closure.
    for path, items in merge_item_catalogs.items():
        for item in items:
            if item["kind"] == "trait_impl" and is_boxed_trait_conversion(
                item.get("trait_raw", ""), item.get("target", "")
            ):
                selected.update(
                    referenced_nominal_names([item["text"]], private_support_types(path))
                    - primary_output_types
                    - protected_source_owned_nested_names
                )

    # Shared seed code and a promoted declaration form one Rust coherence
    # unit. Close nominal dependencies before extracting items: otherwise a
    # root helper can retain `T: SupportTrait` while the trait declaration is
    # left in a child module and its impls are stripped as duplicate prelude.
    # Repeating over selected item bodies also captures transitive field and
    # trait dependencies without any corpus-specific allowlist.
    changed = True
    while changed:
        selected_item_texts = [existing_shared_text]
        selected_item_texts.extend(
            item["text"]
            for name in initially_selected_functions
            for item in top_level_functions_by_name[name]
        )
        for path in merge_input_paths:
            selected_item_texts.extend(
                item["text"]
                for item in select_catalog_items(merge_item_catalogs[path], selected)
            )
        dependencies = {
            name
            for name in referenced_nominal_names(
                selected_item_texts, set(candidates_by_type)
            )
            if name not in primary_output_types and promotable_shared_name(name)
        }
        new_dependencies = dependencies - selected
        changed = bool(new_dependencies)
        selected.update(new_dependencies)

    from_impls = []
    for path in merge_input_paths:
        from_impls.extend(prelude_from_impls(path))
    for _, to_name in from_impls:
        if to_name in existing_shared_types and to_name not in primary_output_types:
            removal_only_names.add(to_name)
            selected.add(to_name)
    trait_impls_for_dependency = []
    for path in merge_input_paths:
        trait_impls_for_dependency.extend(prelude_trait_impls(path))
    changed = True
    while changed:
        changed = False
        for from_name, to_name in from_impls:
            if from_name in primary_output_types or to_name in primary_output_types:
                continue
            if from_name in selected and to_name not in selected and promotable_shared_name(to_name):
                selected.add(to_name)
                changed = True
            if to_name in selected and from_name not in selected and promotable_shared_name(from_name):
                selected.add(from_name)
                changed = True
        for trait_name, target_name in trait_impls_for_dependency:
            if target_name not in selected or target_name in primary_output_types:
                continue
            if not re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", trait_name):
                continue
            if trait_name in primary_output_types:
                continue
            if not promotable_shared_name(trait_name):
                continue
            if trait_name not in selected:
                selected.add(trait_name)
                changed = True

    # Structural source ownership is authoritative even when an earlier
    # assembly stage seeded a facade declaration but did not record the name
    # in the shared-type manifest. Remove those stale crate-wide identities
    # before the empty-selection fast path as well as before normal merging.
    remove_protected_names_from_shared_support(
        shared_support_path,
        protected_source_owned_nested_names | source_shadowing_declared_types,
    )
    remove_protected_names_from_shared_types(
        Path(args.shared_types),
        protected_source_owned_nested_names | source_shadowing_declared_types,
    )

    conflict_lines = ["type\tkind\tdetail\n"]
    if not selected:
        append_missing_builtin_from_impls(
            shared_support_path,
            existing_shared_types,
            primary_output_types,
            existing_shared_from_impl_keys,
        )
        for path in rust_paths:
            matching_input = next(
                (candidate for candidate in merge_input_paths if candidate.name == path.name),
                path,
            )
            restore_source_owned_wrapper_impls(path, matching_input)
            remove_cross_module_source_owned_stubs(
                path,
                protected_source_owned_nested_names,
                prelude_source_owned_nested_types(matching_input),
            )
            remove_duplicate_source_owned_nested_stubs(path)
            remove_duplicate_top_level_aliases(path)
            remove_duplicate_inherent_methods(path)
        Path(args.conflicts).write_text("".join(conflict_lines))
        return 0

    items_by_path = {}
    declarations = defaultdict(list)
    aliases = defaultdict(list)
    trait_impls = defaultdict(list)
    inherent_methods = defaultdict(list)
    inherent_consts = defaultdict(list)
    source_owned_nested_types_by_filename = {
        path.name: prelude_source_owned_nested_types(path)
        | {
            target
            for alias, target in prelude_aliases(path)
            if alias in existing_shared_types
            and target in prelude_support_types(path)
            and target not in primary_output_types
        }
        for path in merge_input_paths
    }

    for path in merge_input_paths:
        items = [
            item
            for item in select_catalog_items(merge_item_catalogs[path], selected)
            if not (
                item["name"] in source_shadowing_aliases
                and item["kind"] != "alias"
            )
        ]
        for item in items:
            if item.get("source_owned_nested") or item.get("source_owned_wrapper"):
                continue
            if item["kind"] == "decl":
                declarations[item["name"]].append((path, item["text"]))
            elif item["kind"] == "alias":
                aliases[item["name"]].append((path, item["target"], item["text"]))
            elif item["kind"] == "trait_impl":
                trait_impls[
                    (item.get("target", item["name"]), item.get("trait_raw", item["trait"]))
                ].append((path, item["text"]))
            elif item["kind"] == "inherent_impl":
                impl_header = item["text"].split("{", 1)[0].rstrip()
                for method_name, method_text in extract_methods(item["text"]):
                    inherent_methods[(item["name"], impl_header, method_name)].append(
                        (path, method_text)
                    )
                for const_name, const_text in extract_consts(item["text"]):
                    inherent_consts[(item["name"], const_name)].append((path, const_text))
    for path in rust_paths:
        items_by_path[path] = [
            item
            for item in parse_items(
                path,
                selected,
                source_owned_nested_types_by_filename.get(path.name),
            )
            if not (
                item["name"] in source_shadowing_aliases
                and item["kind"] != "alias"
            )
        ]
        for item in items_by_path[path]:
            if item["kind"] == "alias" and (
                item["name"] in protected_shared_aliases
                or item["name"] in source_shadowing_aliases
            ) and item["name"] not in source_owned_nested_alias_names:
                item["source_owned_nested"] = False

    existing_shared_items = parse_shared_items_from_text(existing_shared_text, selected)
    generated_trait_decls = {
        name
        for name, variants in declarations.items()
        if any(
            path != shared_support_path and re.match(r"^\s*pub\s+trait\b", text)
            for path, text in variants
        )
    }
    existing_shared_items_to_replace = []
    replaced_shared_decl_names = set()
    for item in existing_shared_items:
        if item["kind"] == "decl":
            name = item["name"]
            if name in generated_trait_decls:
                if not re.match(r"^\s*pub\s+trait\b", item["text"]):
                    continue
                declarations[name].append((shared_support_path, item["text"]))
                existing_shared_items_to_replace.append(item)
                replaced_shared_decl_names.add(name)
                continue
            generated_variants = [
                text for path, text in declarations.get(name, []) if path != shared_support_path
            ]
            if not generated_variants:
                continue
            selected_variant, _ = preferred_decl_variant(
                [item["text"], *generated_variants]
            )
            if selected_variant != item["text"]:
                declarations[name].append((shared_support_path, item["text"]))
                existing_shared_items_to_replace.append(item)
                replaced_shared_decl_names.add(name)
        elif item["kind"] == "trait_impl" and item["trait"] in generated_trait_decls:
            trait_impls[
                (item.get("target", item["name"]), item.get("trait_raw", item["trait"]))
            ].append((shared_support_path, item["text"]))
            existing_shared_items_to_replace.append(item)

    # A package seed owns the nominal identity, but the raw generated prelude
    # can still carry a wider inherent surface. Reconcile that surface instead
    # of treating an existing declaration as a reason to discard every raw
    # method. When a seed already has an impl, include its methods in variant
    # selection and replace that impl atomically; this preserves seed-only
    # methods while allowing raw-only methods to reach the assembled crate.
    generated_inherent_names = {
        type_name for type_name, _impl_header, _method_name in inherent_methods
    } | {type_name for type_name, _const_name in inherent_consts}
    for item in existing_shared_items:
        if item["kind"] != "inherent_impl" or item["name"] not in generated_inherent_names:
            continue
        impl_header = item["text"].split("{", 1)[0].rstrip()
        for method_name, method_text in extract_methods(item["text"]):
            inherent_methods[(item["name"], impl_header, method_name)].append(
                (shared_support_path, method_text)
            )
        for const_name, const_text in extract_consts(item["text"]):
            inherent_consts[(item["name"], const_name)].append(
                (shared_support_path, const_text)
            )
        existing_shared_items_to_replace.append(item)

    # A declaration and its inherent implementation form one representation
    # contract.  If a generated declaration carries more state than the
    # assembly seed (for example, JavaException gains `suppressed`), retaining
    # seed methods creates duplicate APIs and can preserve struct literals for
    # the obsolete representation.  The more informative declaration's own
    # implementations are authoritative, so remove the complete fallback impl.
    for item in existing_shared_items:
        if item["kind"] != "inherent_impl" or item["name"] not in replaced_shared_decl_names:
            continue
        existing_shared_items_to_replace.append(item)

    shared_chunks = []
    replaced_shared_item_ids = {id(item) for item in existing_shared_items_to_replace}
    emitted_trait_impl_keys = {
        (item.get("target", item["name"]), item.get("trait_raw", item["trait"]))
        for item in existing_shared_items
        if item["kind"] == "trait_impl" and id(item) not in replaced_shared_item_ids
    }
    emitted_trait_impl_texts = {
        item["text"].strip()
        for item in existing_shared_items
        if item["kind"] == "trait_impl" and id(item) not in replaced_shared_item_ids
    }
    shared_aliases = set()
    alias_targets = {}
    emitted_from_impl_keys = set(existing_shared_from_impl_keys)
    boxed_conversion_dependencies = {
        key: referenced_nominal_names([text for _path, text in variants], selected)
        for key, variants in trait_impls.items()
        if is_boxed_trait_conversion(key[1], key[0])
    }
    for alias_name, variants in aliases.items():
        if alias_name in protected_shared_aliases:
            continue
        for _, target, _ in variants:
            if target in selected:
                alias_targets[alias_name] = target
                break
    remove_shared_items(shared_support_path, existing_shared_items_to_replace)
    remove_protected_aliases_from_shared_support(
        shared_support_path,
        (protected_shared_aliases & existing_shared_declared_type_names)
        | source_shadowing_aliases
        | (
            source_owned_nested_alias_names
            - source_owned_nested_alias_shared_collisions
        ),
    )
    remove_protected_names_from_shared_support(
        shared_support_path,
        protected_source_owned_nested_names | source_shadowing_declared_types,
    )
    remove_protected_names_from_shared_types(
        Path(args.shared_types),
        protected_source_owned_nested_names | source_shadowing_declared_types,
    )
    remove_protected_names_from_shared_types(
        Path(args.shared_types), source_shadowing_aliases
    )
    remove_protected_names_from_shared_types(
        Path(args.shared_types),
        source_owned_nested_alias_names - source_owned_nested_alias_shared_collisions,
    )
    receiver_preferences = method_call_receiver_preferences(
        merge_input_paths, selected, alias_targets, merge_item_catalogs
    )
    arity_preferences = method_call_arity_preferences(
        merge_input_paths, selected, alias_targets, merge_item_catalogs
    )
    for name in sorted(selected):
        # A conversion target is normally removal-only because an authoritative
        # declaration was seeded into shared support before facade merging.
        # If a generated declaration is more informative, however, the seed
        # has just been removed above and the selected replacement must be
        # emitted.  Otherwise every module imports a type that no longer has a
        # declaration in the assembled crate.
        if (
            name in removal_only_names
            and name not in replaced_shared_decl_names
            and name not in existing_shared_declared_type_names
        ):
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
        selected_decl, selected_decl_reason = preferred_decl_variant(unique_decls)
        selected_decl_paths = {
            path for path, text in decls if text == selected_decl
        }
        selected_decl_is_trait = bool(re.match(r"^\s*pub\s+trait\b", selected_decl))
        declaration_is_already_shared = (
            name in existing_shared_declared_type_names
            and name not in generated_trait_decls
            and name not in replaced_shared_decl_names
        )
        if len(unique_decls) > 1:
            conflict_lines.append(
                f"{name}\tdeclaration\tkept {selected_decl_reason} of {len(unique_decls)} variants\n"
            )
        selected_decl = (
            merge_trait_declarations(
                name,
                [
                    declaration
                    for declaration in unique_decls
                    if re.match(r"^\s*pub\s+trait\b", declaration)
                ],
            )
            if selected_decl_is_trait
            else crate_visible_top_level_type(selected_decl)
        )
        derived_traits = derived_traits_for_decl(selected_decl)
        if not declaration_is_already_shared:
            shared_chunks.append(box_self_recursive_fields(name, selected_decl).rstrip() + "\n")

        method_variants_by_header_and_name = defaultdict(list)
        consts_for_type = []
        emitted_const_names = set()
        if not selected_decl_is_trait:
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
                if const_name in emitted_const_names:
                    continue
                consts_for_type.append(
                    normalize_const_for_impl_target(name, unique_variants[0], alias_targets)
                )
                emitted_const_names.add(const_name)
            for const_name, const_text in builtin_inherent_consts(name):
                if const_name not in emitted_const_names:
                    consts_for_type.append(const_text)
                    emitted_const_names.add(const_name)
        for (type_name, impl_header, method_name), variants in sorted(
            inherent_methods.items()
        ):
            if not item_targets_name(type_name, name, alias_targets):
                continue
            if selected_decl_is_trait and not re.search(
                rf"\bdyn\s+{re.escape(name)}(?:\s*<|\b)", impl_header
            ):
                # Inherent impls emitted beside an opaque fallback struct do
                # not belong to the source interface selected above.
                continue
            for path, text in variants:
                emitted_header = (
                    impl_header
                    if type_name == name
                    else inherent_impl_header_for_decl(
                        name, selected_decl, selected_decl_is_trait
                    )
                )
                method_variants_by_header_and_name[(
                    emitted_header,
                    method_name,
                )].append((path, text))
        methods_by_header = defaultdict(list)
        for (impl_header, method_name), variants in sorted(
            method_variants_by_header_and_name.items()
        ):
            unique_variants = []
            seen_methods = set()
            for path, text in variants:
                if text not in seen_methods:
                    unique_variants.append((path, text))
                    seen_methods.add(text)
            if len(unique_variants) > 1:
                conflict_lines.append(
                    f"{name}\tmethod\t{method_name}: kept preferred of {len(unique_variants)} variants\n"
                )
            preferred_receiver_kinds = receiver_preferences.get((name, method_name), set())
            # Struct literals in constructors and factories must describe the
            # declaration that survives the merge. Prefer methods emitted
            # beside that declaration after honoring observed call shape, so
            # a richer facade cannot inherit a narrower initializer.
            representation_family_variants = {
                text
                for path, text in unique_variants
                if path in selected_decl_paths
            }
            methods_by_header[impl_header].append(
                best_method_variant(
                    method_name,
                    [text for _, text in unique_variants],
                    preferred_receiver_kinds,
                    arity_preferences.get((name, method_name), set()),
                    representation_family_variants,
                )
            )
        if consts_for_type:
            impl_header = inherent_impl_header_for_decl(
                name, selected_decl, selected_decl_is_trait
            )
            shared_chunks.append(f"\n{impl_header} {{\n")
            for const in consts_for_type:
                shared_chunks.append(f"    {const}\n")
            shared_chunks.append("}\n")
        for impl_header, methods_for_impl in sorted(methods_by_header.items()):
            shared_chunks.append(f"\n{impl_header} {{\n")
            for method in methods_for_impl:
                shared_chunks.append("    " + method.replace("\n", "\n    ").rstrip() + "\n")
            shared_chunks.append("}\n")

        for (type_name, trait_raw), variants in sorted(trait_impls.items()):
            trait_name = trait_impl_name(trait_raw)
            from_match = FROM_TRAIT_RE.match(trait_raw)
            belongs_to_type = impl_target_name(type_name) == name
            belongs_to_from_source = from_match is not None and impl_target_name(
                from_match.group(1)
            ) == name
            belongs_to_boxed_dyn_trait = boxed_dyn_trait_target_name(type_name) == name
            belongs_to_adapter_dependency = name in boxed_conversion_dependencies.get(
                (type_name, trait_raw), ()
            )
            if selected_decl_is_trait and belongs_to_type:
                # Eq/Hash/Display impls synthesized for an opaque struct are
                # invalid once the authoritative declaration is a trait.
                continue
            if not (
                belongs_to_type
                or belongs_to_from_source
                or belongs_to_boxed_dyn_trait
                or belongs_to_adapter_dependency
            ):
                continue
            trait_impl_key = (type_name, trait_raw)
            if trait_impl_key in emitted_trait_impl_keys:
                continue
            unique_variants = []
            seen_traits = set()
            for _, text in variants:
                if text not in seen_traits:
                    unique_variants.append(text)
                    seen_traits.add(text)
            selected_trait_impl, selected_trait_reason = preferred_trait_impl_variant(
                unique_variants
            )
            if len(unique_variants) > 1:
                conflict_lines.append(
                    f"{name}\ttrait_impl\t{trait_name}: kept {selected_trait_reason} of {len(unique_variants)} variants\n"
                )
            if trait_is_derived_by_decl(trait_name, derived_traits):
                continue
            if from_match:
                emitted_from_impl_keys.add(
                    (from_match.group(1), impl_target_name(type_name))
                )
            emitted_trait_impl_keys.add(trait_impl_key)
            if selected_trait_impl.strip() not in emitted_trait_impl_texts:
                shared_chunks.append("\n" + selected_trait_impl.rstrip() + "\n")
                emitted_trait_impl_texts.add(selected_trait_impl.strip())
        if selected_decl_is_trait:
            selected_trait_method_abis = {
                method_name: method_abi_signature(method_text)
                for method_name, method_text in extract_trait_methods(selected_decl)
            }
            for (type_name, trait_raw), variants in sorted(trait_impls.items()):
                trait_name = trait_impl_name(trait_raw)
                if trait_name != name:
                    continue
                trait_impl_key = (type_name, trait_raw)
                if trait_impl_key in emitted_trait_impl_keys:
                    continue
                if not (
                    trait_impl_target_can_be_emitted_without_decl(
                        type_name, variants[0][1]
                    )
                    or type_name in promoted_aliases
                    or type_name in existing_shared_types
                ):
                    continue
                methods_by_name = defaultdict(list)
                for _, text in variants:
                    for method_name, method_text in extract_methods(text):
                        methods_by_name[method_name].append(method_text)
                if not methods_by_name:
                    continue
                impl_header = variants[0][1].split("{", 1)[0].rstrip()
                shared_chunks.append(f"\n{impl_header} {{\n")
                for method_name, method_variants in sorted(methods_by_name.items()):
                    unique_variants = []
                    seen_methods = set()
                    for text in method_variants:
                        if text not in seen_methods:
                            unique_variants.append(text)
                            seen_methods.add(text)
                    if len(unique_variants) > 1:
                        conflict_lines.append(
                            f"{type_name}\ttrait_impl\t{name}.{method_name}: kept preferred of {len(unique_variants)} variants\n"
                        )
                    # A merged trait declaration is the crate-wide ABI. A
                    # concrete-looking facade variant may be more informative
                    # in isolation but is not a valid implementation when the
                    # selected trait method returns an erased carrier. Prefer
                    # exact ABI matches before applying ordinary quality ranks.
                    required_abi = selected_trait_method_abis.get(method_name)
                    abi_variants = [
                        text
                        for text in unique_variants
                        if required_abi is not None
                        and method_abi_signature(text) == required_abi
                    ]
                    selected_method = best_method_variant(
                        method_name, abi_variants or unique_variants, set()
                    )
                    shared_chunks.append(
                        "    " + selected_method.replace("\n", "\n    ").rstrip() + "\n"
                    )
                shared_chunks.append("}\n")
                emitted_trait_impl_keys.add(trait_impl_key)
        shared_chunks.append("\n")

    shared_type_universe = selected | existing_shared_types
    existing_from_impl_keys = emitted_from_impl_keys
    for from_name, to_name, text in builtin_from_impls(shared_type_universe):
        if (
            from_name in primary_output_types
            or to_name in primary_output_types
            or (from_name, to_name) in existing_from_impl_keys
        ):
            continue
        shared_chunks.append("\n" + text.rstrip() + "\n")

    # Seed dependency discovery from the stripped output modules as well as
    # shared declarations.  Calls from translated source bodies otherwise lose
    # helpers that were present in every per-file prelude but were not reached
    # by another shared helper.
    selected_functions, selected_values = top_level_item_dependency_closure(
        [existing_shared_text]
        + shared_chunks
        + [text_without_top_level_functions(path) for path in rust_paths],
        top_level_functions_by_name,
        top_level_values_by_name,
    )
    promote_existing_top_level_functions(shared_support_path, selected_functions)
    for function_name in sorted(selected_functions):
        if function_name in existing_shared_function_names:
            continue
        variants = []
        seen_functions = set()
        for item in top_level_functions_by_name[function_name]:
            text = item["text"]
            if text in seen_functions:
                continue
            variants.append(text)
            seen_functions.add(text)
        if not variants:
            continue
        if len(variants) > 1:
            conflict_lines.append(
                f"{function_name}\tfunction\tkept first of {len(variants)} variants\n"
            )
        shared_chunks.append(
            "\n" + crate_visible_top_level_function(variants[0]).rstrip() + "\n"
        )

    for value_name in sorted(selected_values):
        if value_name in existing_shared_value_names:
            continue
        variants = []
        seen_values = set()
        for item in top_level_values_by_name[value_name]:
            text = item["text"]
            if text in seen_values:
                continue
            variants.append(text)
            seen_values.add(text)
        if not variants:
            continue
        if len(variants) > 1:
            conflict_lines.append(
                f"{value_name}\tvalue\tkept first of {len(variants)} variants\n"
            )
        shared_chunks.append(
            "\n" + crate_visible_top_level_value(variants[0]).rstrip() + "\n"
        )

    # Macro-generated impls are coherence-bearing support just like explicit
    # impl blocks. If a repeated prelude macro defines implementations for a
    # promoted trait, move one definition and its unique invocations alongside
    # that trait. Dropping these items leaves the trait visible but silently
    # removes implementations (for example String support for Arrays helpers).
    shared_dependency_names = (
        selected
        | existing_shared_types
        | selected_functions
        | selected_values
    )
    selected_macro_names = set()
    for name, definitions in macro_definitions_by_name.items():
        definition_paths = {item["path"] for item in definitions}
        if not macro_invocations_by_name.get(name):
            continue
        if len(definition_paths) <= 1 and name not in existing_shared_text:
            continue
        if any(
            referenced_nominal_names([item["text"]], shared_dependency_names)
            for item in definitions
        ):
            selected_macro_names.add(name)

    existing_shared_macro_items = top_level_macro_items(existing_shared_text)
    existing_shared_macros = {
        item["name"]
        for item in existing_shared_macro_items
        if item["kind"] == "macro_definition"
    }
    existing_macro_invocation_identifiers = set()
    for item in existing_shared_macro_items:
        if item["kind"] == "macro_invocation":
            existing_macro_invocation_identifiers.update(
                IDENTIFIER_RE.findall(item["text"])
            )
    for name in sorted(selected_macro_names):
        definitions = []
        seen_definitions = set()
        for item in macro_definitions_by_name[name]:
            if item["text"] in seen_definitions:
                continue
            definitions.append(item["text"])
            seen_definitions.add(item["text"])
        if len(definitions) > 1:
            conflict_lines.append(
                f"{name}\tmacro_definition\tkept first of {len(definitions)} variants\n"
            )
        # The package assembler may already provide the same coherence family
        # through a differently named shared macro. Its invocation names the
        # generated trait even when the macro body accepts that trait as a
        # metavariable. Do not invoke a second prelude macro whose body names
        # that already-covered trait; Rust would see duplicate impls after
        # expansion. Other macro families (whose generated traits are absent)
        # still move normally.
        generated_shared_names = (
            referenced_nominal_names(definitions, shared_dependency_names)
            if definitions
            else set()
        )
        if generated_shared_names & existing_macro_invocation_identifiers:
            continue
        if definitions and name not in existing_shared_macros:
            shared_chunks.append("\n" + definitions[0].rstrip() + "\n")

        invocations = []
        seen_invocations = set()
        for item in macro_invocations_by_name[name]:
            if item["text"] in seen_invocations:
                continue
            invocations.append(item["text"])
            seen_invocations.add(item["text"])
        for invocation in invocations:
            if invocation.strip() not in existing_shared_text:
                shared_chunks.append("\n" + invocation.rstrip() + "\n")

    for alias_name, variants in sorted(aliases.items()):
        if alias_name in protected_shared_aliases or alias_name in source_shadowing_aliases:
            continue
        if alias_name not in promoted_aliases and not any(
            target in selected for _, target, _ in variants
        ):
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
        for name in sorted(selected_functions):
            shared_types.write(f"{name}\n")
        for name in sorted(selected_values):
            shared_types.write(f"{name}\n")

    for path, items in items_by_path.items():
        matching_input = next(
            (candidate for candidate in merge_input_paths if candidate.name == path.name),
            path,
        )
        restore_source_owned_wrapper_impls(path, matching_input)
        if selected_functions:
            items = items + [
                item
                for item in parse_top_level_functions(path)
                if item["name"] in selected_functions
            ]
        if selected_values:
            items = items + [
                item
                for item in parse_top_level_values(path)
                if item["name"] in selected_values
            ]
        if selected_macro_names:
            items = items + [
                item
                for item in top_level_macro_items(path.read_text())
                if item["name"] in selected_macro_names
            ]
        remove_items(
            path,
            items,
            removal_only_names,
            keep_last_prelude_names,
            selected - removal_only_names,
        )
        remove_cross_module_source_owned_stubs(
            path,
            protected_source_owned_nested_names,
            source_owned_nested_types_by_filename.get(path.name, set()),
        )
        remove_duplicate_source_owned_nested_stubs(path)
        remove_duplicate_top_level_aliases(path)
        remove_duplicate_inherent_methods(path)

    Path(args.conflicts).write_text("".join(conflict_lines))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
