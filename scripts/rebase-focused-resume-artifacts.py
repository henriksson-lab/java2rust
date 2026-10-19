#!/usr/bin/env python3
"""Keep a copied focused-output cache's rewrite targets inside its new root."""

import csv
import io
from pathlib import Path
import sys


def rebase(status: Path, manifest: Path, generated: Path, raw: Path) -> None:
    generated = generated.resolve(strict=True)
    raw = raw.resolve(strict=True)
    output = generated.parents[2]
    pending = []
    for metadata in (status, manifest):
        if metadata == manifest and not metadata.exists():
            continue
        if metadata.resolve(strict=True).parent != output:
            raise ValueError(f"resume metadata escapes output root: {metadata}")
        rows = list(csv.reader(io.StringIO(metadata.read_text()), delimiter="\t"))
        if not rows or "rust_path" not in rows[0]:
            raise ValueError(f"resume metadata has no rust_path column: {metadata}")
        column = rows[0].index("rust_path")
        seen = set()
        for row in rows[1:]:
            if len(row) <= column:
                raise ValueError(f"incomplete resume metadata row: {metadata}")
            name = Path(row[column]).name
            target = generated / name
            source = raw / name
            if (not name.endswith(".rs") or name in seen
                    or target.resolve().parent != generated
                    or source.resolve().parent != raw or not source.is_file()):
                raise ValueError(f"invalid resume Rust artifact: {row[column]}")
            seen.add(name)
            row[column] = str(target)
        buffer = io.StringIO()
        csv.writer(buffer, delimiter="\t", lineterminator="\n").writerows(rows)
        pending.append((metadata, buffer.getvalue()))
    # Validate every metadata file before changing any file or clearing the
    # generated directory. Atomic replacement also avoids partial TSV writes.
    for metadata, text in pending:
        temporary = metadata.with_name(metadata.name + ".resume-paths.tmp")
        temporary.write_text(text)
        temporary.replace(metadata)


if __name__ == "__main__":
    try:
        if len(sys.argv) != 5:
            raise ValueError("expected status, manifest, generated directory, raw directory")
        rebase(*(Path(value) for value in sys.argv[1:]))
    except (OSError, ValueError) as error:
        raise SystemExit(f"cannot resume focused assembly safely: {error}")
