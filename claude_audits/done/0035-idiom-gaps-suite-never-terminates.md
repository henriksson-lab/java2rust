# 0035 — The largest behavioural test suite has not completed a run in a long time

Status: **validated** (reproduced at HEAD with all working-tree changes stashed)
Severity: high — process; it silently stopped being a gate
Kind: test infrastructure

## Symptom

`cargo test --test idiom_gaps` never finishes. 11 of its 684 tests hang; the
other 673 complete in about three minutes. A single hanging test was left
running for over ten minutes with no output and no progress.

Because the run never returns, any script or agent that shells out to it and
waits sees a timeout, not a failure — and a timeout is easy to read as "slow"
rather than "broken".

## Reproduction

```sh
cargo test --release --quiet --test idiom_gaps \
  summary_context_translation_resolves_sibling_symbols_without_lowering_bodies
# no output; still running after 10 minutes
```

Confirmed at HEAD with every uncommitted source change stashed, so this is not
caused by any in-flight work.

## The eleven

```
corpus_picard_imported_constants_need_source_summary_facts
fully_qualified_jdk_static_constant_lowers_through_external_field_fact
inherited_external_field_setter_does_not_become_static_owner
inherited_source_summary_field_types_emit_opaque_support_shells
java_integer_division_and_remainder_by_zero_raise_catchable_arithmetic_exception
neutral_idiom_fixtures_generated_crate_cargo_checks
source_summary_method_surface_can_be_added_after_external_return_selects_type
source_summary_nullable_return_into_concrete_local_unwraps_once
source_summary_nullable_return_into_optional_local_does_not_double_wrap
source_summary_opaque_type_gets_used_inherited_method_surface
summary_context_translation_resolves_sibling_symbols_without_lowering_bodies
```

Nine of the eleven involve source-summary or context translation. Two
(`neutral_idiom_fixtures_generated_crate_cargo_checks`,
`java_integer_division_and_remainder_by_zero_raise_catchable_arithmetic_exception`)
invoke `cargo` on a generated crate, so those may be waiting on the build lock
held by the outer `cargo test` rather than looping — worth checking first,
because if that is the cause it is also the cheapest fix.

## Why this matters more than eleven hangs

`docs/corpus-fix-current-proceed-plan.md:16` records:

> `cargo test --test idiom_gaps --quiet`: passes, 297 tests

It is now 684 tests, 144 of which fail and 11 of which never return. The suite
grew 2.3× and stopped being runnable, and the plan documents still describe it as
a gate.

The consequence is concrete and was observed: **five behavioural regressions
reached the working tree across two rounds of changes and were not caught**,
because the suite that covers them could not produce a result. They were found
only by running it to a fixed truncation point and diffing failure *names*
against a stashed baseline — which is not something a normal workflow does.

This is [0018](../0018-the-compile-error-metric-cannot-see-silent-failures.md)'s
thesis applied to the test infrastructure rather than the translator: a suite
that never terminates is indistinguishable, to anything that just waits on it,
from a suite that passes.

## What to do

1. **Diagnose the eleven.** Start with the two that shell out to `cargo` — if
   they deadlock on the build lock, the fix is to not nest `cargo` inside
   `cargo test`, or to give them a separate target directory.
2. **Bound every test.** Whatever the cause, no test should be able to hang the
   suite. A watchdog that fails a test after N seconds converts an invisible
   hang into a visible failure.
3. **Record the real baseline.** 144 failures at HEAD is the number to diff
   against; publish it next to the suite so a change can be judged. A suite with
   144 known failures is still useful as a *differential* gate, but only if the
   expected set is written down.
4. **Fix the stale claim** in `docs/corpus-fix-current-proceed-plan.md`, or mark
   the file as historical. A plan that asserts a suite passes, when it has not
   completed in a long time, actively misleads the next reader.

## Note on the 144

Not investigated here. Some are genuine translator gaps and some may be stale
text assertions — two of them were found to **contradict each other** on the
overload-naming convention (`get_count_` vs no trailing underscore), which
suggests the set has accumulated without review. Triaging them is worth a pass
of its own.

---

## Update 2026-08-03 — fixed at the cause; the suite returns

Status: **fixed.** `cargo test --release --test idiom_gaps` completes in ~8
minutes. Baseline checked in as `tests/baselines/idiom-gaps-failures.txt`.

### It was not the nested `cargo`

The obvious suspect — ~290 tests each spawning `cargo check` on a generated
crate — was wrong. A single test finishes in 3.5 seconds. Profiling the stuck
process (`perf record --call-graph dwarf`, after `gdb` proved useless under
`ptrace_scope=1`) put 87% of samples inside
`lower_program_with_semantic_plans_and_support_context`, and phase probes
narrowed it to:

```rust
while support.add_source_summary_method_surfaces(…) {}
```

### The bug

`add_source_summary_method_surfaces` set `changed = true` from the **intent** to
add a facade method:

```rust
changed = true;
self.add_method(owner.clone(), method_body);
```

`add_method` has **six early returns** where it does nothing — AWT owners,
invalid identifiers, erased source-alias owners, already-emitted source methods,
and two idempotence guards. When one fired, the loop reported progress it had
not made and ran forever. All seven hanging tests were on the source-summary
path; the name was the clue.

Fixed by making `add_method` return whether the support state actually changed
and having the caller read it. A fixpoint's termination condition has to be
"the state stopped changing" — deriving it from anything else is the defect.

### Verified byte-identical, which mattered

Six of the seven tests fail now that they terminate, and it would be easy to
read that as the fix breaking them. It is not: running the **old,
over-reporting** loop under an iteration bound produces output that differs from
the new loop only by the budget diagnostic. The six failures are pre-existing
defects the hang was hiding, and they are recorded as such in the baseline file
rather than quietly absorbed.

### The watchdog, and why it is still worth having

`cargo_verify_generated_modules` now runs the nested cargo under a timeout
(`NEW_JAVA2RUST_GENERATED_CRATE_TIMEOUT_SECS`, default 300s), draining both
pipes on their own threads, and kills the child rather than blocking forever. It
also strips the inherited jobserver (`CARGO_MAKEFLAGS`) and gives each generated
crate its own `CARGO_TARGET_DIR` — a nested cargo that inherits either can block
indefinitely on a token or a build lock. Those are hangs, not failures, and this
issue exists because the difference is invisible from outside.
