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
    while target.startswith("&"):
        target = re.sub(r"^&\s*(?:'[A-Za-z_][A-Za-z0-9_]*\s+)?(?:mut\s+)?", "", target).lstrip()
    target = target.removeprefix("dyn ")
    outer_type = target.split("<", 1)[0].strip()
    return outer_type.rsplit("::", 1)[-1]


def _impl_header_layout(line: str):
    """Return ``(trait, target, target_start, target_end)`` for an impl header.

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
            elif text[index] == ">" and not (index > 0 and text[index - 1] == "-"):
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
        elif ch == ">" and not (index > 0 and text[index - 1] == "-"):
            depth -= 1
            if depth < 0:
                return None
        elif depth == 0:
            if text.startswith(" for ", index):
                trait_separator = index
                index += len(" for ") - 1
            elif ch == "{" or (ch.isspace() and re.match(r"\s+where\b", text[index:])):
                end = index
                break
        index += 1
    if end is None or depth != 0:
        return None
    if trait_separator is None:
        target = text[start:end].strip()
        return (None, target, start, end) if target else None
    trait = text[start:trait_separator].strip()
    target = text[trait_separator + len(" for "):end].strip()
    return (trait, target, trait_separator + len(" for "), end) if trait and target else None


def impl_header_parts(line: str):
    """Return the selected trait/target using the shared structural scan."""
    layout = _impl_header_layout(line)
    return layout[:2] if layout is not None else None


def split_rust_type_arguments(text: str) -> list[str]:
    """Split type arguments without splitting nested types, tuples or arrays."""
    parts, start = [], 0
    depths = {"<": 0, "(": 0, "[": 0, "{": 0}
    closing = {">": "<", ")": "(", "]": "[", "}": "{"}
    for index, ch in enumerate(text):
        if ch in depths:
            depths[ch] += 1
        elif ch in closing and not (ch == ">" and index > 0 and text[index - 1] == "-"):
            depths[closing[ch]] -= 1
        elif ch == "," and not any(depths.values()):
            parts.append(text[start:index].strip())
            start = index + 1
    tail = text[start:].strip()
    if tail:
        parts.append(tail)
    return parts


def expand_inherent_impl_alias(header: str, declaration: str) -> str | None:
    """Expand an impl's alias target, preserving its binders and where clause.

    Rebuilding from the underlying declaration loses fixed arguments: an
    impl on Alias<T> = Store<T, Local<T>> must not become impl<T, B> Store<T, B>.
    Only the target type changes. Unsupported alias syntax remains unexpanded
    rather than silently broadening its implementation's domain.
    """
    alias = re.fullmatch(
        r"\s*pub\s+type\s+([A-Za-z_][A-Za-z0-9_]*)"
        r"(?:\s*<([^=;]+)>)?\s*=\s*(.+?)\s*;\s*", declaration,
    )
    if alias is None:
        return None
    text = header.strip()
    layout = _impl_header_layout(text + " {")
    if layout is None or layout[0] is not None:
        return None
    _, target, start, end = layout
    name, parameters, template = alias.groups()
    applied = re.fullmatch(rf"{re.escape(name)}(?:\s*<(.*)>)?", target)
    if applied is None:
        return None
    params = split_rust_type_arguments(parameters or "")
    args = split_rust_type_arguments(applied.group(1) or "")
    names = [re.match(r"(?:const\s+)?('[A-Za-z_][A-Za-z0-9_]*|[A-Za-z_][A-Za-z0-9_]*)", param) for param in params]
    if len(args) != len(params) or any(match is None for match in names):
        return None
    substitutions = {match.group(1): arg for match, arg in zip(names, args)}
    # Type templates may contain lifetime/const parameters. Preserve quoted
    # literals while substituting tokens, never arbitrary identifier substrings.
    token = re.compile(r'''"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])'|'?[A-Za-z_][A-Za-z0-9_]*''')
    expanded = token.sub(lambda match: substitutions.get(match.group(), match.group()), template)
    return text[:start] + expanded + text[end:]


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


@cache
def has_associated_definitions(impl_text: str) -> bool:
    """Whether a generated impl has an associated type/constant contract.

    Such impls must be selected atomically: combining methods from different
    variants can disagree with their associated definitions. Ignore aliases
    inside method bodies and delimiter-like characters in literal strings.
    """
    if "{" not in impl_text:
        return False
    body = impl_text.split("{", 1)[1].rsplit("}", 1)[0]
    depths = (0, 0, 0)
    for line in body.splitlines():
        if depths == (0, 0, 0) and re.match(
            r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:type\s+|const\s+(?!fn\b))[A-Za-z_]",
            line,
        ):
            return True
        depths = _rust_delimiter_depths(line, depths)
    return False


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
