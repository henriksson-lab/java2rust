# Current Corpus Idiom Fix Plan

Date: 2026-07-23

This plan starts from the dirty checkout after the latest FastQC exception
subset work. The immediate objective is to get the current idiom tests and
focused corpuses back into a measured state, then choose the next implementation
batch from real corpus failures rather than guesses.

## Current Evidence

Latest focused subset evidence:

- output:
  `.tmp/corpus-fastqc-exception-subset-after-stringbuilder-append-borrowed-arg`
- translation: 9/9 files translated
- invalid-pattern scan: 0 files
- generated Rust: cargo check still fails with 6 underlying errors

The most important remaining compile families are:

- nullable borrowed argument drift, visible as temporary-borrow errors such as
  `Some(reference.get_raw_input_string().as_str())` flowing into
  `Option<&str>`;
- erased Java object recovery, including `Box<dyn Any>`/`Vec<Box<dyn Any>>`
  clone/display/deserialization handoff;
- source trait/object handoff, including erased object to `Localizable`;
- map key/value borrowedness, visible as `String: Borrow<&String>`;
- source lifetime drift when borrowed slots are stored or returned without a
  stable lifetime model.

## Guardrails

- Keep non-JDK package behavior out of `src/stdlib.rs`.
- Prefer callable/member/fact/slot plans over rendered Rust string heuristics.
- Keep GenomicsDB native behavior and DISQ/Spark distributed behavior as
  explicit facade or FFI handoff boundaries.
- Do not make algorithm changes that block the later global semantic analysis
  model.
- Use 10 jobs by default for corpus reruns.
- Add one minimized idiom fixture for every recurring corpus failure family
  before or alongside the fix.

## Phase 1: Restore Nullable Argument Semantics

Status: next.

Problem:

External borrowed argument facts currently apply borrowing even after the
parameter has been wrapped as `Option<T>`. That is useful for a few handoff APIs
such as `ObjectOutputStream.writeObject`, but it is wrong as a general rule
because temporary owned expressions can become invalid `Option<&T>` values.

Actions:

1. In `src/call_plan.rs`, make `argument_shape_from_external_metadata` apply
   alias borrowing to nullable parameters only when an explicit allow flag says
   the API supports nullable borrowed handoff.
2. Thread that allow flag from `argument_shape_from_external_effect`.
3. Allow nullable borrowed slots only for precise JDK handoff APIs where it is
   needed, initially `ObjectOutputStream.writeObject/write_object`.
4. In `src/stdlib.rs`, make facade argument shape rendering follow the same
   policy so generated support signatures and call planning agree.
5. In `src/external_packs.rs`, mark `StringBuilder.append` and
   `StringBuffer.append` argument facts as definitely non-null borrowed string
   arguments, not nullable borrowed optional strings.
6. Update idiom test expectations after rerunning the affected tests.

Acceptance:

- `referenceSourcePath(if missing { null } else { temporaryString })` no longer
  borrows a temporary through `Option<&str>`.
- `SamReaderFactory.referenceSequence(if missing { null } else { reference })`
  keeps a compile-safe optional shape.
- `ObjectOutputStream.writeObject(args[j])` still avoids moving out of indexed
  vectors.
- Direct `StringBuilder.append(value)` keeps borrowed string behavior without
  generating `Some(...)` wrappers.

## Phase 2: Revalidate Local Gates

Status: after Phase 1.

Run:

```text
cargo fmt
cargo check --quiet
cargo test --test idiom_gaps --quiet
cargo test --test type_coercion --quiet
```

If the idiom test suite fails, fix only the regressions caused by the nullable
argument policy change before moving to corpus reruns.

Acceptance:

- formatting passes;
- core check passes;
- idiom and type-coercion tests pass;
- all updated assertions describe the desired slot policy, not incidental
  temporary syntax.

## Phase 3: Rerun The Focused FastQC Exception Subset

Status: after local gates.

Run:

```text
cargo build --release --quiet --bin new-java2rust
scripts/translate-focused-package.sh --skip-resolver-bench --jobs 10 --primary /tmp/new_java2rust_fastqc_exception_subset --out .tmp/corpus-fastqc-exception-subset-after-nullable-borrow-policy
```

Record:

- translated file count;
- invalid-pattern count;
- generated Rust cargo error count;
- top error families;
- whether `write_object(Some(&args[j as usize]))` or an equivalent safe handoff
  remains present.

Acceptance:

- no invalid Java-pattern output;
- no broad nullable borrowed temporary regressions;
- the remaining error list is smaller or at least more precise than the current
  6-error focused subset.

## Phase 4: Fix Remaining FastQC Exception Families

Status: pending Phase 3 evidence.

Likely next fixtures and fixes:

- erased object display helper for string concat and `StringBuilder.append`;
- erased object clone helper for stored `Object...` and map values;
- erased deserialization handoff for `ObjectInputStream.readObject()` assigned
  to `String`, `Box<dyn Any>`, or trait-object targets;
- source interface/object coercion for erased object to `Localizable`;
- map key iteration normalization so `HashMap<String, _>::get` receives `&str`
  or `&String`, not `&&String`;
- lifetime diagnostics for source fields/signatures that currently store
  borrowed data without a lifetime-bearing representation.

Acceptance:

- every surviving FastQC exception subset compile error has a minimized fixture
  in `tests/idiom_gaps.rs` or a clearly marked unsupported handoff fixture;
- generated code uses explicit helpers for erased Java object behavior instead
  of accidental `Any` clone/display methods.

## Phase 5: Broaden Corpus Reruns

Status: after the focused subset is materially improved.

Rerun tracked corpuses with `--jobs 10`, including:

- Picard corpus;
- GenomicsDB corpus;
- DISQ corpus;
- FastQC broader corpus;
- HTSJDK-focused corpuses;
- GATK non-facade Java utility corpuses.

For each corpus, capture:

- translated count;
- invalid-pattern count;
- generated Rust cargo error count;
- top recurring idiom families;
- whether the failure is mechanically translatable Java, facade/FFI handoff, or
  intentionally out of scope runtime behavior.

Acceptance:

- `docs/current-corpus-issue-audit.md` has a current row for every rerun;
- any new recurring idiom has either a fixture or a documented reason it should
  become a facade/FFI boundary.

## Phase 6: Choose The Next Batch

Status: after Phase 5.

Prioritize in this order:

1. high-frequency mechanical Java idioms that affect multiple corpuses;
2. math/numeric correctness and 2D matrix/array handling;
3. explicit FFI handoff shapes for native libraries, where the future C/C++
   transpiler supplies signatures first;
4. cleanup and inlining delegated to
   `/data/henriksson/github/claude/rust_refactor` only after generated Rust is
   working.

Avoid spending translator effort on true GenomicsDB native behavior or
DISQ/Spark distributed execution. Those should translate Java call sites into
handoff/facade states, not mechanically reproduce the runtime.

## Verification Checklist

- `cargo fmt --check`
- `cargo check --quiet`
- `cargo test --test idiom_gaps --quiet`
- `cargo test --test parser_gaps --quiet`
- `cargo test --test batch_translation --quiet`
- `cargo test --test type_coercion --quiet`
- `cargo build --release --quiet --bin new-java2rust`
- focused and broad corpus reruns recorded in
  `docs/current-corpus-issue-audit.md`
