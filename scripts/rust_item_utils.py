"""Shared structural helpers for generated Rust item assembly.

Keep nominal-owner projection here rather than teaching each reconciliation
pass its own approximation of Rust generic syntax.
"""


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
