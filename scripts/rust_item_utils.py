"""Shared structural helpers for generated Rust item assembly.

Keep nominal-owner projection here rather than teaching each reconciliation
pass its own approximation of Rust generic syntax.
"""

import re
from functools import cache


_MACRO_DEFINITION_RE = re.compile(
    r"^\s*macro_rules!\s*([A-Za-z_][A-Za-z0-9_]*)\s*[({[]"
)
_MACRO_INVOCATION_RE = re.compile(
    r"^\s*([A-Za-z_][A-Za-z0-9_]*)!\s*[({[]"
)


def impl_target_name(raw_target: str) -> str:
    """Return the outer declared type name from an impl target.

    Generic arguments and leading reference/dynamic markers do not change the
    nominal owner: ``Comparator<T>`` and ``Comparator`` both belong to
    ``Comparator``.
    """
    target = raw_target.strip()
    target = target.removeprefix("&mut ").removeprefix("&")
    target = target.removeprefix("dyn ")
    outer_type = target.split("<", 1)[0].strip()
    return outer_type.rsplit("::", 1)[-1]


def impl_header_parts(line: str):
    """Return ``(trait, target)`` for a generated one-line impl header.

    ``trait`` is ``None`` for inherent impls.  Angle brackets are scanned
    structurally so nested bounds such as ``T: View<Box<dyn Any>>`` do not
    truncate the impl generics the way a ``[^>]`` regular expression does.
    """
    text = line.strip()
    if not text.startswith("impl"):
        return None
    index = len("impl")
    if index < len(text) and (text[index].isalnum() or text[index] == "_"):
        return None
    while index < len(text) and text[index].isspace():
        index += 1
    if index < len(text) and text[index] == "<":
        depth = 0
        while index < len(text):
            if text[index] == "<":
                depth += 1
            elif text[index] == ">":
                depth -= 1
                if depth == 0:
                    index += 1
                    break
            index += 1
        if depth != 0:
            return None
        while index < len(text) and text[index].isspace():
            index += 1

    start = index
    depth = 0
    trait_separator = None
    end = None
    while index < len(text):
        ch = text[index]
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
            if depth < 0:
                return None
        elif depth == 0:
            if text.startswith(" for ", index):
                trait_separator = index
                index += len(" for ") - 1
            elif ch == "{" or text.startswith(" where", index):
                end = index
                break
        index += 1
    if end is None or depth != 0:
        return None
    if trait_separator is None:
        target = text[start:end].strip()
        return (None, target) if target else None
    trait = text[start:trait_separator].strip()
    target = text[trait_separator + len(" for "):end].strip()
    return (trait, target) if trait and target else None


def inherent_impl_target_name(line: str):
    """Return the nominal target of an inherent generated impl header."""
    parts = impl_header_parts(line)
    if parts is None or parts[0] is not None:
        return None
    return impl_target_name(parts[1])


@cache
def _rust_delimiter_depths(line: str, depths: tuple[int, int, int]):
    """Advance Rust (), [] and {} depths while ignoring literals/comments."""
    paren, bracket, brace = depths
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
                # Lifetimes have no closing quote. Treat only a conventional
                # one-character/escaped character token as a char literal.
                if next_ch == "\\":
                    is_char = index + 3 < len(line) and line[index + 3] == "'"
                else:
                    is_char = index + 2 < len(line) and line[index + 2] == "'"
                if is_char:
                    state = "char"
                    index += 1
                    continue
            if ch == "(":
                paren += 1
            elif ch == ")":
                paren -= 1
            elif ch == "[":
                bracket += 1
            elif ch == "]":
                bracket -= 1
            elif ch == "{":
                brace += 1
            elif ch == "}":
                brace -= 1
            index += 1
            continue
        if state in {"string", "char"}:
            if ch == "\\":
                index += 2
                continue
            if (state == "string" and ch == '"') or (state == "char" and ch == "'"):
                state = "code"
            index += 1
            continue
        if ch == "*" and next_ch == "/":
            state = "code"
            index += 2
            continue
        index += 1
    return paren, bracket, brace


def top_level_macro_items(text: str) -> list[dict]:
    """Return top-level ``macro_rules!`` definitions and macro invocations.

    Assembly treats these as ordinary coherence-bearing Rust items: moving a
    trait out of duplicated module preludes must also move macros that generate
    its implementations. The returned start/end offsets are line indexes and
    use the same half-open convention as the merger's other parsed items.
    """
    lines = text.splitlines(keepends=True)
    items = []
    outer_depths = (0, 0, 0)
    index = 0
    while index < len(lines):
        line = lines[index]
        definition = _MACRO_DEFINITION_RE.match(line) if outer_depths == (0, 0, 0) else None
        invocation = _MACRO_INVOCATION_RE.match(line) if outer_depths == (0, 0, 0) else None
        match = definition or invocation
        if match:
            depths = (0, 0, 0)
            end = index
            while end < len(lines):
                depths = _rust_delimiter_depths(lines[end], depths)
                end += 1
                if depths == (0, 0, 0):
                    break
            items.append(
                {
                    "kind": "macro_definition" if definition else "macro_invocation",
                    "name": match.group(1),
                    "start": index,
                    "end": end,
                    "text": "".join(lines[index:end]),
                }
            )
            index = end
            continue
        outer_depths = _rust_delimiter_depths(line, outer_depths)
        index += 1
    return items
