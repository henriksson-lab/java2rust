# Parallel Idiom Gap Worktree Status

Setup date: 2026-07-21

Base revision: `e8f8371` (`master`, commit message shown by git as
`prep for parallel`).

## Worktrees Created

| Category | Branch | Path |
|---|---|---|
| Source summary | `idiom/source-summary` | `/data/henriksson/github/claude/new_java2rust-source-summary` |
| FFI handover | `idiom/ffi-handover` | `/data/henriksson/github/claude/new_java2rust-ffi-handover` |
| Builders | `idiom/builders` | `/data/henriksson/github/claude/new_java2rust-builders` |
| Exceptions | `idiom/exceptions` | `/data/henriksson/github/claude/new_java2rust-exceptions` |
| Spark/streams | `idiom/spark-streams` | `/data/henriksson/github/claude/new_java2rust-spark-streams` |

## Verification

Commands were run with per-worktree `CARGO_TARGET_DIR` values under `/tmp` so
verification does not dirty sibling worktrees with build artifacts.

| Branch | Command | Result |
|---|---|---|
| `idiom/source-summary` | `cargo test --test idiom_gaps corpus_picard -- --nocapture` | Passed: 1/1 |
| `idiom/ffi-handover` | `cargo test --test idiom_gaps ffi -- --nocapture` | Passed: 2/2 |
| `idiom/builders` | `cargo test --test idiom_gaps builder -- --nocapture` | Passed: 1/1 |
| `idiom/exceptions` | `cargo test --test idiom_gaps exception -- --nocapture` | Passed: 7/7 |
| `idiom/spark-streams` | `cargo test --test idiom_gaps spark -- --nocapture` | Passed: 1/1 |

## Deviation

The broad `source_summary` filter currently includes an older unrelated failing
test, `inherited_source_summary_instance_fields_are_flattened_into_primary_struct`.
The focused Picard corpus source-summary test passed, so the source-summary
worktree is usable for the planned Picard constant/downcast task. The broader
source-summary filter should be cleaned up or split when that branch starts
implementation.
