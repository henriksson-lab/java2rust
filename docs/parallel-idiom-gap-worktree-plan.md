# Parallel Idiom Gap Worktree Plan

Use separate git worktrees for implementation work on idiom-gap categories. Use
parallel workers in the same checkout only for read-only investigation, corpus
runs, and report generation.

## Why Worktrees

Separate worktrees avoid edit conflicts in shared files such as
`tests/idiom_gaps.rs`, generated `.tmp` outputs, and `target` artifacts while
still sharing one git object store. They are lighter and less divergent than
full cloned repositories.

## Setup

Create one branch and one worktree per category:

```bash
git worktree add -b idiom/source-summary ../new_java2rust-source-summary HEAD
git worktree add -b idiom/ffi-handover ../new_java2rust-ffi-handover HEAD
git worktree add -b idiom/builders ../new_java2rust-builders HEAD
git worktree add -b idiom/exceptions ../new_java2rust-exceptions HEAD
git worktree add -b idiom/spark-streams ../new_java2rust-spark-streams HEAD
```

If a branch already exists, omit `-b` and use the branch name:

```bash
git worktree add ../new_java2rust-source-summary idiom/source-summary
```

## Categories

| Branch | Owns | Corpus/fixture pressure |
|---|---|---|
| `idiom/source-summary` | Source-summary field/method facts, package-qualified constants, typed external/source fields, downcast removal. | `picard-programgroups`, `SourceSummaryConstantAcrossPackage.java` |
| `idiom/ffi-handover` | `ffi-boundary` manifests, FFI-owned handles, native handover call shape, cleanup/error metadata. | `gatk-genomicsdb`, `FfiNativeHandoverCall.java`, `FfiOwnedHandleTransfer.java` |
| `idiom/builders` | Nested type identity, protobuf-style builders, fluent mutators, `build()` return typing. | `gatk-genomicsdb`, `QualifiedNestedBuilderChain.java` |
| `idiom/exceptions` | Nested try/catch, catch-type selection, try/finally, try-with-resources, constructor delegation with throwing helpers. | `SampleNameMap.java`, `FileLoadConstructorDelegation.java` |
| `idiom/spark-streams` | Stream/RDD lambdas, method references, nullable external return slots, Spark/DISQ boundary diagnostics. | `gatk-disq-spark-datasources`, `SparkRddVariantPipeline.java`, `DisqExternalBoundaryChain.java` |

## Worker Rules

- Each worker edits only its branch/worktree.
- Each worker owns category-specific fixture assertions in `tests/idiom_gaps.rs`.
- Shared APIs may be edited, but avoid opportunistic refactors outside the
  category.
- Do not commit `.tmp` corpus outputs unless a specific report needs them.
- Track durable results in docs; keep large generated crates untracked.
- Re-run the category fixture filter before handing off:

```bash
cargo test --test idiom_gaps corpus_ -- --nocapture
```

Use narrower filters when available, such as:

```bash
cargo test --test idiom_gaps source_summary -- --nocapture
cargo test --test idiom_gaps ffi -- --nocapture
cargo test --test idiom_gaps builder -- --nocapture
cargo test --test idiom_gaps exception -- --nocapture
cargo test --test idiom_gaps spark -- --nocapture
```

## Merge Order

Merge smallest, least cross-cutting changes first:

1. `idiom/source-summary`
2. `idiom/builders`
3. `idiom/exceptions`
4. `idiom/ffi-handover`
5. `idiom/spark-streams`

After each merge, rebase the remaining branches onto the updated main branch
and rerun their category tests. If two branches both need the same shared
planner API, merge the smaller API-only patch first or split it into its own
short-lived branch.

## Done Criteria

A category branch is ready to merge when:

- its focused fixtures pass;
- relevant corpus slice improves or has a documented blocker;
- generated Rust does not gain new broad `Any`/downcast, unsupported lambda,
  or unsupported try/catch/finally regressions outside the category;
- any non-mechanical boundary is represented as explicit facts/diagnostics, not
  accidental opaque method guesses.
