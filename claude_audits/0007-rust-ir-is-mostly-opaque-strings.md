# 0007 — The Rust IR is mostly opaque strings, so decisions are made by substring search

Status: **validated by inspection** (structural)
Severity: high — this is the mechanism the convergence plan is fighting
Kind: IR modelling gap

Current status 2026-08-29 (Codex audit pass):

- Still open, but no longer in the broad producer state described by the
  original counts below. A production-only scan before test modules finds
  `RustExpr::Raw` matching concentrated in the named compatibility/recovery
  paths in `coercion.rs`, `item_assembly.rs`, `lower.rs`, and `slot_render.rs`,
  plus the emitter and the central constructor in `rust_ir.rs`.
- The optional string-view and optional-construction cluster in `coercion.rs`
  is already guarded as intentional compatibility: structured `Some(...)`
  normalization avoids whole-expression rendering, while raw string-view
  recovery is fenced behind
  `recover_raw_optional_constructed_string_view_element`.
- The next useful work is not another bulk `Raw(format!(...))` producer sweep.
  It is deleting or narrowing one named compatibility consumer at a time, with
  each pass proving that parseable legacy raw leaves are recovered at a boundary
  before semantic predicates run.
- Follow-up in this pass: `ensure_borrow_projection_rendered` and
  `render_string_view_argument` are now `#[cfg(test)]`. Production already used
  the structured borrow projection helper, so this removes a rendered-string
  bridge from the production API surface without changing generated output.
- Follow-up in this pass: direct `Coercion::AsSlice` application now respects a
  recorded `RustExprShape::SliceView` instead of appending another `.as_slice()`
  projection. This removes the generated `source.as_slice().as_slice()` shape
  that made `tests/differential/LambdaMutation.java` fail on stable Rust with
  `E0658` from slice-view reapplication. Regression:
  `coercion::tests::as_slice_coercion_reuses_recorded_slice_view`.

Partial progress 2026-08-24:

- Borrow projection now recovers more raw operands into structured `RustExpr`
  before deciding whether explicit grouping is needed. Function/method-call
  operands such as `java_string_substring(...)` and `custom_key_view(...)` stay
  prefix-safe under `&`, string-literal receivers with suffixes still group as
  `&("literal".to_string())`, and mutable borrowed compound operands preserve
  the structured binary/index shape expected by the idempotence tests.
- The five previously baselined `--lib` failures are gone:
  `cargo test --lib -- --nocapture` passes 327/0. This is a local reduction of
  raw-string borrow handling debt, not closure of the broader 0007 audit.
- A later cleanup pass removed the top-level raw-string recovery branch from
  optional string-view construction normalization, stopped `lower.rs` from
  post-processing return expressions through that normalizer, and reused the
  central char-literal projection helper from `call_lowering.rs` instead of
  duplicating `strip_prefix('&')` parsing there. The source guard
  `rendered_string_heuristics_have_centralization_guardrails` is green again
  after updating stale assertions to distinguish raw consumers from raw
  constructors.
- Another cleanup pass taught the central rendered-expression recovery helper
  to rebuild top-level Rust ranges as `RustExpr::Range`, including inclusive
  ranges inside grouping. This removes a small but recurring `Raw("(0..limit)")`
  dependency from stream/coercion paths while keeping the parser conservative:
  it ignores range tokens inside strings and nested delimiters, rejects
  ambiguous `...`, and requires exactly one top-level range operator.
  Regressions: `coercion::tests::rendered_projection_recovers_borrowed_leaf_expressions`,
  `stream_plan_lowers_int_stream_range_source`, and
  `translation_lowers_int_stream_range_source_without_static_facade`.
- Stream lambda `Expr::Raw` ingress now runs through the same conservative
  rendered-expression recovery helper instead of always constructing an opaque
  `RustExpr::Raw` leaf. The stream path uses the central emitter precedence for
  the recovered expression, so raw fragments recovered as binaries are grouped
  correctly under stronger parent operators. Regression:
  `stream_raw_expr_recovery_preserves_binary_precedence`.
- Current-tree audit 2026-08-25: the direct production `RustExpr::Raw(format!(...))`
  population described below is now largely stale. A source scan for
  `RustExpr::Raw(format!` finds only `coercion.rs` regression fixtures, while
  `tests/layering.rs::rendered_string_heuristics_have_centralization_guardrails`
  and the larger `slot_shape_design` guard suite now fence many producer paths
  against reintroducing raw formatted Rust expressions. The audit remains open
  because `Raw` still exists as a compatibility carrier and because consumers
  still recover structure from rendered text in bounded helpers; the next work
  is deleting those consumer round trips or narrowing them behind explicit
  compatibility names, not chasing the historical broad set of direct
  `Raw(format!(...))` producers.
- Follow-up 2026-08-25: the central rendered-expression recovery helper now
  rebuilds simple expression-bodied closures such as `|v| v.trim()` and
  `move |item| item.iter()` as `RustExpr::Closure` when parameters are untyped
  and the body itself recovers structurally. It deliberately leaves typed
  parameters and explicit-return-type closures as raw compatibility leaves.
  Regression: `coercion::tests::rendered_projection_recovers_borrowed_leaf_expressions`.
- Follow-up 2026-08-25: the same recovery helper now rebuilds single-tail block
  expressions such as `{ value.trim() }` as `RustExpr::Block`, which also lets
  block-bodied expression closures like `|v| { v.trim() }` keep their closure
  and method-call structure. Blocks with statements/semicolons still remain raw.
  Regression: `coercion::tests::rendered_projection_recovers_borrowed_leaf_expressions`.
- Current-tree measurement 2026-08-26: the broad producer pattern is now much
  smaller than the historical counts below. Excluding test modules, the guarded
  core lowering files no longer mint direct production `RustExpr::Raw(...)`
  constructors outside named leaf/recovery boundaries, and a scan for
  `RustExpr::Raw(format!(...))` still finds only regression fixtures. The
  central raw constructors are `RustExpr::raw_leaf` / `RustStmt::raw_leaf`; the
  remaining 0007 work should therefore target named compatibility consumers,
  recovery helpers, and places that still infer semantics from raw text rather
  than the old bulk `Raw(format!(...))` producer class.
- Follow-up 2026-08-26: optional mutable reborrow detection no longer asks raw
  text first. `expr_is_optional_mut_reborrow` now accepts structured
  zero-argument `as_deref_mut` calls directly, recovers parseable raw renderings
  through the central rendered-expression helper, and keeps any remaining raw
  compatibility behind a named helper rather than inline suffix checks.
  Regression was updated in the 2026-08-28 follow-up below.
- Follow-up 2026-08-28: the last optional mutable reborrow suffix fallback is
  gone. The raw compatibility helper now only accepts expressions recovered by
  the central rendered-expression helper as structured zero-argument
  `as_deref_mut` method calls; malformed opaque raw text ending in that suffix
  is rejected instead of being treated as semantic evidence. Regression:
  `coercion::tests::optional_mut_reborrow_recovers_parseable_raw_projection_without_suffix_fallback`.
- Follow-up 2026-08-26: optional-construction detection now keeps the structured
  and Raw paths aligned on `Some` arity. Structured calls must be single-segment
  `Some` with exactly one argument, and the rendered Raw fallback rejects
  `Some()` and top-level multi-argument `Some(left, right)` instead of treating
  the envelope alone as proof of an optional value. Regression:
  `type_coercion::optional_construction_detection_requires_some_arity_one`.
- Follow-up 2026-08-26: erased-object downcast protection for unresolved static
  owner placeholders no longer renders every expression and searches for the
  `unimplemented!("unresolved Java static call ...")` text. The guard recognizes
  structured `RustExpr::MacroCall` placeholders through the IR, recurses through
  wrapper nodes, and keeps the substring check only for Raw compatibility
  leaves. Literal text mentioning the diagnostic no longer suppresses a
  downcast. Regression:
  `coercion::tests::unresolved_static_owner_detection_is_structural_before_raw_fallback`.
- Follow-up 2026-08-26: the central rendered-expression recovery helper now
  rebuilds Rust array literals as `RustExpr::ArrayLiteral`, using the same
  top-level comma splitter that already handles tuple and `vec![]` recovery.
  This keeps expressions such as `[left + right, helper("a, b")]` structured,
  and also lets surrounding binary expressions like `[left, right] + tail`
  recover without preserving the array side as a raw leaf. Regression:
  `coercion::tests::rendered_projection_recovers_borrowed_leaf_expressions`.
- Follow-up 2026-08-28: stream-lambda index-to-`usize` projection no longer
  treats unrecovered raw text as a simple cast operand merely because it has no
  spaces. Raw operands still get one conservative recovery pass first, so
  parseable identifiers, calls, fields, indexes, and binaries keep their
  structured behavior; only opaque raw leaves now fall back to explicit
  grouping. Regression:
  `coercion::tests::cast_operand_parenthesization_prefers_expression_structure`.
- Follow-up 2026-08-28: mutable-alias detection no longer has a final
  prefix/suffix parser for unrecovered raw text. Rendered compatibility inputs
  get one pass through the central rendered-expression recovery helper and must
  then match the structured alias shapes; malformed raw text such as
  `values.field.as_mut_slice() trailing` now fails closed instead of proving a
  mutable alias from a suffix. Regression:
  `slot_shape_design::rendered_mutable_alias_projection_names_same_call_alias_shapes`.
- Follow-up 2026-08-28: optional-carrier detection no longer treats an
  unrecovered raw `if ... { Some(...) } ...` substring as semantic evidence.
  The rendered-expression recovery helper now rebuilds complete expression-form
  `if <expr> { <expr> } else { <expr> }` leaves as `RustExpr::If` when the
  condition and both arms recover structurally; malformed raw text with trailing
  content now remains opaque and fails closed. Regressions:
  `coercion::tests::rendered_if_projection_requires_complete_structured_expression`
  and
  `type_coercion::raw_if_optional_construction_detection_ignores_literal_text`.
- Follow-up 2026-08-28: text-only string-view detection no longer accepts any
  raw fragment merely because it ends with `.as_str()`. The public rendered
  predicate now routes through central expression recovery and requires the
  whole fragment to recover as a zero-arg `as_str()` projection, a narrow
  `*<string-view>` compatibility spelling, or a complete Rust string literal.
  Malformed suffix text such as `name.as_str() trailing` and compound text such
  as `prefix + name.as_str()` now fail closed. Regressions:
  `coercion::tests::an_untagged_string_view_still_falls_back_to_the_text` and
  `type_coercion::borrowed_string_view_projection_owns_rendered_view_compatibility`.
- Follow-up 2026-08-28: borrowed map-key detection now shares that string-view
  rendered predicate instead of carrying its own `.as_str()` suffix check.
  Structured map-key helper calls still answer through the structural call
  predicate, and complete string literals remain borrowed keys; malformed raw
  suffix text no longer proves the key is already borrowed. Regression:
  `coercion::tests::borrowed_map_key_literal_fallback_requires_complete_literal`.
- Follow-up 2026-08-28: owned-string recovery no longer preserves an
  unrecovered raw leaf merely because it ends with `.to_string()`. The legacy
  helper now requires whole-fragment structural recovery to a zero-argument
  `to_string` call, and malformed raw receivers with whitespace are grouped
  before a new method suffix is attached. Regression:
  `coercion::tests::owned_string_recovery_idempotence_prefers_structure_and_shape`.
- Follow-up 2026-08-28: clone-recovery idempotence no longer trusts raw text
  just because it ends with `.clone()`, `.clone_box()`, `.cloned()`, or a
  nullable `.cloned().expect(...)` suffix. Rendered compatibility now strips
  block comments, requires whole-fragment structural recovery to one of the
  clone-recovery projections, and groups malformed raw receivers before
  appending a new clone. Regression:
  `coercion::tests::clone_recovery_requires_complete_structural_projection`.
- Follow-up 2026-08-28: result panic-recovery idempotence no longer uses the
  raw `.unwrap_or_else(|ex| panic!(...))` suffix as proof that a result has
  already been adapted. The raw compatibility branch now requires
  whole-fragment recovery to the structured panic-adapter method call before it
  suppresses a second adapter; malformed suffix text and literal text mentioning
  the adapter are treated as ordinary receivers. Regression:
  `coercion::tests::jdk_math_and_objects_projections_own_expression_shapes`.
- Follow-up 2026-08-28: optional-carrier detection for `Map::get`/sequence
  `get` followed by `.cloned()` or `.copied()` no longer has a raw suffix
  parser. Raw compatibility first recovers the whole expression, then accepts
  only structured zero-argument `cloned`/`copied` calls whose receiver is a
  one-argument `get` call; `Some(...)` remains the only text fallback for
  unrecovered raw leaves. Regressions:
  `coercion::tests::option_wrap_records_constructed_optional_shape` and
  `type_coercion::rust_get_projection_counts_as_existing_optional_carrier`.
- Follow-up 2026-08-28: erased-object clone idempotence no longer treats a raw
  expression that merely mentions `java_clone_erased_object_ref(...)` as already
  recovered. Raw compatibility now recovers the whole expression first and only
  accepts a structured clone-helper call; nested helper mentions are wrapped
  again instead of being returned with an `OwnedRecovery` tag. Regressions:
  `coercion::tests::erased_object_clone_recovery_records_recovery_shape` and
  `slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

## Observation

At filing, `src/rust_ir.rs` defined a structured `RustExpr`, but 58% of all
constructed expressions were the escape hatch:

| variant | construction sites across `src/` |
|---|---|
| `RustExpr::Raw` | **542** |
| `RustExpr::Var` | 147 |
| `RustExpr::Literal` | 59 |
| `RustExpr::Call` | 44 |
| `RustExpr::MethodCall` | 24 |
| `RustExpr::Binary` | 22 |
| `RustExpr::Cast` | 21 |
| all other variants | 70 |

`RustType::Raw` (37) likewise outnumbers `RustType::Named` (19). `RustStmt::Raw`
is 27 of 111.

Once an expression is `Raw(String)`, its structure is gone. Downstream code that
needs to know what it is has only one option: look at the text.

## Consequence: render-then-reparse round trips

`src/coercion.rs` calls `emit::emit_expr` **296** times, 37 of which are the
explicit `let rendered = emit::emit_expr(&expr);` round trip — lower to a
string, then inspect the string to decide what to do next.

It matches against **55 distinct Rust syntax fragments**. A representative
sample:

```rust
rendered.ends_with(".clone()")
rendered.ends_with(".clone_box()")
rendered.ends_with(".cloned()")
rendered.ends_with(".as_deref_mut()")
rendered.ends_with(".as_mut_slice()")
rendered.ends_with(".as_ref().map(|value| value.as_str())")
rendered.starts_with('&')
rendered.contains(".downcast::<")
rendered.contains("__java_self")
rendered.contains(" + ")        // detecting a binary operator
rendered.contains(" as ")
rendered.contains(">::anonymous()")
rendered.contains(".clone()/* TODO(translation): validate added clone */.into()")
```

Two of these deserve to be read twice.

`rendered.contains(" + ")` is **parsing Rust with substring search**. It is used
in `strip_arithmetic_display_conversion` and
`parenthesize_compound_cast_operand` (`src/coercion.rs`) to decide whether an
expression is an arithmetic expression. A string literal containing `" + "`, or
an argument to a nested call, is indistinguishable from a top-level binary
operator.

`rendered.contains(".clone()/* TODO(translation): validate added clone */.into()")`
matches against an emitted **comment**. The comment is now load-bearing syntax:
changing its wording changes behaviour.

`src/lower.rs` goes further and re-parses emitted item text:

```rust
fn raw_inherent_impl_owner(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("impl ")?;
    if rest.contains(" for ") { return None; }
    // ... hand-rolled generic-parameter skipping via rest.find('>')
}

fn raw_impl_method_name(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("pub fn ").or_else(|| trimmed.strip_prefix("fn "))?;
    // ...
}
```

This recovers the owner type and method name of an `impl` block by string
surgery on Rust source that this same program produced moments earlier.

## Why this matters beyond aesthetics

The convergence plan (`docs/rule-generalization-convergence-plan.md`) correctly
identifies "facts beat rendered-string checks" as the principle. But the plan
frames the string checks as a *habit* to be replaced site by site. Historically,
they were not a habit — they were a **forced consequence** of `RustExpr::Raw`
being the dominant constructor. As long as most expressions carry no structure, any new
decision that needs to know what an expression is will reach for the text again,
and the count will grow back.

This also caps how good the output can get. Cleanups such as removing redundant
`.clone()`, collapsing `&*x`, or fixing parenthesisation cannot be done reliably
on strings, which is presumably part of why the project delegates cleanup to the
external `rust_refactor` tool.

## Proposed direction

The goal is not "delete `Raw`" — that is not achievable in one step and not
required. The goal is that **decisions never read text**.

1. **Tag, don't parse.** Attach a structural tag to lowered expressions so
   consumers ask the tag rather than the string. The convergence plan already
   proposes `RustExprShape` (`Borrowed`, `MutBorrowed`, `OwnedRecovery`,
   `OptionalProjected`, `SliceView`, `MutableSliceView`, `StringView`,
   `AnonymousDynTrait`, `ResultPropagated`). Make
   this the *return type* of lowering — e.g. `LoweredExpr { expr: RustExpr,
   shape: RustExprShape, actual: RustSlotShape }` — so a `Raw` payload still
   carries what downstream needs.
2. **Make the tag mandatory.** If `Raw` can only be constructed through a
   helper that requires a shape, the escape hatch stops losing information even
   while it stays in use. This is the single highest-leverage change here.
3. **Retire the round trips by inversion.** Each `rendered.ends_with(…)` check
   corresponds to a shape the *producer* already knew. Fix them at the
   producer, not the consumer: whoever emitted `.as_deref_mut()` knew it was a
   `MutBorrowed` optional projection.
4. **Add the structured variants that `Raw` is standing in for.** The 542 sites
   are not uniformly exotic; expect a small number of missing node kinds
   (closures, `?`, method chains with turbofish, struct-update, `match`) to
   account for a large share. Measure before designing.
5. **Ban text inspection with a test.** Once the shape tag exists, add a test
   that greps `src/coercion.rs` and `src/lower.rs` for `emit_expr` followed by a
   string predicate and fails on new occurrences. Otherwise the count regrows.

## Measurement to redo after any change

```
grep -oh -E 'RustExpr::[A-Za-z]+' src/*.rs | sort | uniq -c | sort -rn
grep -c 'let rendered = emit' src/*.rs
grep -oh -E '(ends_with|starts_with|contains)\("[^"]+"\)' src/coercion.rs | sort -u | wc -l
```

Original baseline: 542 `Raw` / 929 total, 37 round trips, 55 distinct fragments.

---

## Update 2026-08-03 — measured, and the tag now exists

Status: **still open**, but the mechanism from step 2 is in place and one family
is migrated end to end. The counts have not moved; that is expected, and the
section below explains why they are the wrong thing to watch first.

### Measurement (step 4: "measure before designing")

`Raw` sites by file — the distribution is far more concentrated than the totals
suggest:

| file | `RustExpr::Raw` sites |
|---|---|
| `src/coercion.rs` | **357** |
| `src/lower.rs` | 92 |
| `src/call_lowering.rs` | 23 |
| everything else (12 files) | ~114 |

61% of all `Raw` construction is in one file, which is also where 39 of the 52
render-then-reparse round trips are. So this is not a diffuse habit across the
codebase — it is one module's adapter layer.

Breaking down what those 357 actually build (252 `Raw(format!(…))`, 62
`Raw("literal")`, ~43 `Raw(<variable>)`), the format strings cluster hard:

```
8  {}.as_str()          4  &{}              3  {}.as_slice()
5  {}.expect(…)         3  {}::{}           3  {}.as_mut_slice()
3  {}.as_ref()          3  {}.as_deref_mut()   2  {}.iter().cloned()
```

These are `MethodCall`, `Unary` and `Call` — variants that **already exist**.
That revises the audit's own hypothesis: for `coercion.rs` the dominant `Raw`
population is not exotic node kinds the IR lacks, it is existing node kinds
spelled as strings.

More importantly, reading the consumers shows what the text checks are actually
asking. Almost all of them are one question:

```rust
let rendered = emit_expr(&expr);
if rendered.ends_with(".as_str()") { expr }              // already a view?
else { RustExpr::Raw(format!("{rendered}.as_str()")) }   // make one
```

**Idempotence** — "have I already applied this adapter?" The producer always
knew. That is what the shape tag records.

### What landed

`RustExpr::Shaped { shape, expr }` plus `RustExprShape`
(`StringView | Borrowed | MutBorrowed | OwnedRecovery | OptionalProjected |
SliceView | MutableSliceView | AnonymousDynTrait | ResultPropagated`) in `src/rust_ir.rs`, with `shaped()`,
`recorded_shape()`, `unshaped()`, `into_unshaped()`.

Three properties make the migration incremental rather than a flag day, each
covered by a test:

1. **The tag is invisible to the emitter.** `Shaped` renders and takes
   precedence as its inner expression, so tagging a producer cannot change the
   generated Rust.
2. **Structural consumers see through it.** Every exhaustive `match` on
   `RustExpr` (7 sites) delegates through the tag, so tagging cannot silently
   break a downstream match.
3. **`recorded_shape()` returning `None` means *unknown*, never *no*.** The
   query is tag-first with the old text check as fallback:

   ```rust
   pub fn expr_is_string_view(expr: &RustExpr) -> bool {
       if let Some(shape) = expr.recorded_shape() {
           return shape == RustExprShape::StringView;
       }
       rendered_expr_is_already_string_view(&emit::emit_expr(expr))
   }
   ```

   This is the audit's own advice — "keep the old predicate and the new query
   both computed during migration" — as the permanent structure of the query
   rather than as scaffolding. A tagged producer can only turn a `false` into a
   `true`, never the reverse, so no migration step can regress behaviour.

Four families are migrated:

| shape | producer | what the text check was |
|---|---|---|
| `StringView` | 8 `.as_str()` sites, `string_helper_receiver_projection` | `ends_with(".as_str()")` + 3 literal-prefix tests |
| `SliceView` | 3 `.as_slice()` sites | `ends_with(".as_slice()")` |
| `ResultPropagated` | `propagate_result_projection` | `matches!(expr, Raw(raw) if raw.trim_end().ends_with('?'))` |
| `OwnedRecovery` | `apply_clone_recovery` | an **eight-way** alternation, one arm matching an emitted comment |

The last two are the ones that were actually dangerous:

- The `?` check matched only `RustExpr::Raw`, so a propagated expression carried
  by any other variant was propagated again. It also cannot tell a try operator
  from a question mark inside a string literal — the confusion that shipped as a
  `?` spliced into a literal.
- `rendered_expr_has_clone_recovery` includes
  `rendered.contains(".clone()/* TODO(translation): validate added clone */.into()")`.
  Every clone this producer makes now carries `OwnedRecovery` and is answered
  before that alternation is consulted. When the remaining clone producers are
  tagged, the comment stops being syntax and the arm can be deleted — that is
  the concrete done-when for step 3 of this file.

All queries are **monotone**: `tag == expected || <old text check>`. A tag can
only turn a `false` into a `true`, so no migration step can regress behaviour,
and a producer can be tagged without auditing every consumer first.

### Why the counts did not move

`Raw` went 592 → 598 and round trips 40 → 39. Tagging *wraps* `Raw`, it does not
remove it, and that is the design: removing `Raw` requires rewriting each
producer into structured variants, which is a much larger and riskier change
that this makes unnecessary as a precondition.

The number to watch instead is **how many decisions are made from text**, which
is what step 5's proposed lint would count. The right sequence from here:

1. `OptionalProjected` / `MutBorrowed` — the `.as_ref()` / `.as_mut()` /
   `.as_deref()` / `.as_deref_mut()` adapters, the last big untagged family.
2. Tag the remaining clone producers, then **delete** the comment-matching arm
   of `rendered_expr_has_clone_recovery`. That is a visible, checkable end state.
3. Retire the round trips by inversion (step 3 above): each surviving
   `rendered.ends_with(…)` is a shape its producer knew.
4. Only then add the lint from step 5, once there is a tag for each question it
   would forbid asking textually. Adding it earlier would just force the checks
   underground.

Two text checks are **not** tag candidates and should stay: 
`first_rust_try_operator_index` finds the *position* of a `?` inside a rendered
string in order to splice recovery around it, and
`optional_erased_string_downcast_ref_view_projection` extracts a local's name
from a rendering. A tag answers "what is this"; neither of those is that
question.

### Re-measure

```
grep -oh -E 'RustExpr::[A-Za-z]+' src/*.rs | sort | uniq -c | sort -rn
grep -c 'let rendered = emit' src/*.rs
grep -oh -E '(ends_with|starts_with|contains)\("[^"]+"\)' src/coercion.rs | sort -u | wc -l
```

2026-08-03: 598 `Raw` / ~1060 total, 52 round trips (39 in `coercion.rs`), 56
distinct fragments.

---

## Update 2026-08-03 (second pass) — the comment is no longer syntax

Status: **still open**, but the concrete done-when this file set for step 3 is
met: `rendered_expr_has_clone_recovery` no longer matches an emitted comment.

### What went

```rust
// before — eight arms, three of them matching a comment, one with `contains`
rendered.ends_with(".clone()/* TODO(translation): validate added clone */")
rendered.ends_with(".cloned()/* TODO(translation): validate added clone */")
rendered.contains(".clone()/* TODO(translation): validate added clone */.into()")
```

All three are deleted, along with the equivalent arm in
`rendered_expr_has_direct_clone_recovery`. The wording of
`/* TODO(translation): validate added clone */` is now inert: changing it can no
longer change behaviour.

### It took two things, and the order mattered

1. **Invert the consumers.** Three call sites asked
   `rendered_expr_has_clone_recovery(&emit_expr(expr))` — a round trip that
   cannot see a tag. They now ask `rust_expr_has_clone_recovery(expr)`, which is
   tag-first with the text check as fallback.
2. **Tag the remaining producers.** `apply_clone_recovery` already recorded
   `OwnedRecovery`; four adapters did not —
   `apply_borrowed_option_clone_adapter`, `apply_owned_option_clone_adapter`,
   and the list and set collect-fusing adapters. Those four are why the comment
   arms were still load-bearing.

**Deleting the arms before tagging the producers produced a double clone** —
`self.query.as_ref().cloned()/* … */.clone()/* … */` — caught by
`optional_self_field_read_into_owned_local_borrows_then_clones_element`. That is
the third time in this project that verify-then-delete has caught a wrong
subsumption hypothesis, after `catch_future_use_names` and the `finally`
override. The rule that keeps paying: **delete the fallback last, and only after
the thing meant to replace it is measured to cover every producer.**

### Test fallout, and why it was a test change and not a regression

Four `type_coercion` assertions compared `RustExpr` values structurally, so
wrapping a producer's result in `Shaped` failed them even though the emitted
Rust was byte-identical — the tag is invisible to the emitter, by design. They
now compare through `into_unshaped()`, which is what they were always asserting.

### Counts

| | filed | first pass | now |
|---|---|---|---|
| `RustExpr::Raw` | 542 | 600 | 604 |
| `Shaped` construction sites | — | 12 | **27** |
| distinct `coercion.rs` syntax fragments | 55 | 56 | **52** |

`Raw` went *up* again, and that remains the wrong number to watch: tagging wraps
`Raw`, it does not remove it. The fragment count moving 56 → 52 is the real
signal, and it moved because arms were deleted rather than added.

### Next

`OptionalProjected` / `MutBorrowed` — the `.as_ref()` / `.as_mut()` /
`.as_deref()` / `.as_deref_mut()` family — is the last big untagged group, and
the one Tier 5 (optional carriers) will need. The anti-regression lint stays
last, as this file has said twice: adding it before every question has a tag
would only force the checks underground.

---

## Update 2026-08-05 (third pass) — the optional-projection family needs `actual`, not another tag variant

Status: **still open.** The family this file named as "next" was attempted and
**deliberately not migrated**, for a reason that is a correction to the design
rather than a lack of effort.

### What the family actually asks

`apply_optional_borrowed_element_target_view` (`src/coercion.rs`) is five arms
that all ask the same idempotence question, each about a *different* adapter:

| target (ownership, view) | adapter | text check |
|---|---|---|
| `SharedBorrow, Slice` | `.as_deref()` | `ends_with(".as_deref()")` |
| `MutableBorrow, Slice` | `.as_deref_mut()` | `ends_with(".as_deref_mut()")` |
| `SharedBorrow, Str` | `.as_ref().map(\|v\| v.as_str())` | `ends_with(…)` |
| `SharedBorrow, Native` | `.as_ref()` | `ends_with(".as_ref()")` |
| `MutableBorrow, Native` | `.as_mut()` | `ends_with(".as_mut()")` |

The question is **not** "what kind of thing is this expression" — it is "which
of these five adapters has already been applied". `RustExprShape` has one name
for all of them, `OptionalProjected`.

### Why tagging them anyway would emit wrong code

The monotone rule that made the earlier migrations safe —
`tag == expected || <old text check>`, so a tag can only turn `false` into
`true` — does **not** make this family safe, and it is worth being explicit
about why, because the rule is stated in this file as though it were universal.

In these arms `true` means *skip the adapter*. So a tag that is right about the
family and wrong about the member suppresses an adapter that was needed:
tag `.as_deref()`'s result `OptionalProjected`, and the `SharedBorrow, Native`
arm will decline to add `.as_ref()` because it sees the family tag. The result
is a missing adapter — a type error at best, a wrong borrow at worst.

Where `true` means "already correct" (`expr_is_string_view`) monotonicity is
genuinely safe. Where `true` means "do nothing", it is not. **The rule is about
the *consumer's* interpretation of `true`, not about tagging.** That distinction
was not stated before and is the reason this pass stopped.

### The constraint that blocks it

`RustExprShape` is `#[derive(Copy)]` and `RustSlotShape` is not, so the shape
cannot simply carry the slot it produced. Adding five more coarse variants would
work mechanically and would encode one call site's arm list into a shared enum.

This file's own step 1 already proposes the right answer, and it is the part not
yet built:

> `LoweredExpr { expr: RustExpr, shape: RustExprShape, actual: RustSlotShape }`

`actual` is what this family needs — the *resulting slot shape*, which the arm
computes anyway (`target_element`) and then throws away. With it, every arm's
question becomes `actual == target_element` and all five text checks go at once,
along with the three in `apply_required_borrowed_element_target_view`.

So the ordering in the previous update's "Next" was wrong: this family is not
the next tagging step, it is the first consumer of `actual`, and `actual` should
be built first.

### Re-measurement

```
2026-08-03: 598 Raw, 52 round trips, 56 fragments, 12 Shaped sites
2026-08-03: 604 Raw, 52 round trips, 52 fragments, 27 Shaped sites
2026-08-05: 609 Raw, 49 round trips, 52 fragments, 15 `RustExpr::shaped(` sites
```

## Update 2026-08-06 (Codex follow-up) — the emitter's unit-expression helper also had to see through tags

`emit_expr` and precedence handling already treated `RustExpr::Shaped` as
transparent, but `is_unit_expr` did not. Statement emission uses that helper to
elide `();` and to render `return ();` as `return;`, so a tagged unit expression
could still leak the migration wrapper into emitter decisions even though the
wrapper was invisible to the main expression renderer.

`is_unit_expr` now recurses through `Shaped`, and the existing emitter unit tests
cover both statement-position `Shaped(())` and return-position `Shaped(())`.

Round trips 52 → 49 (38 of them still in `coercion.rs`). Fragments unchanged at
52, which is correct: nothing was deleted this pass because nothing was
migrated.

`Raw` continues to rise and continues to be the wrong number to watch, for the
reason given twice above.

### Revised next

1. Add `actual: RustSlotShape` alongside the coarse tag — the audit's step 1,
   and the prerequisite this pass discovered.
2. Then migrate the optional-projection family and
   `apply_required_borrowed_element_target_view` together: eight text checks,
   one question, one mechanism.
3. The anti-regression lint stays last, unchanged.

---

## Update 2026-08-06 (fourth pass) — `actual` is built, and the family it blocked is migrated

The previous pass stopped at the optional-projection family and said the
prerequisite was this file's own step 1, `actual: RustSlotShape`. That is now
built and the family is migrated.

### What landed

`RustExpr::Shaped` gained `actual: Option<Box<RustSlotShape>>`, with
`shaped_as(shape, actual, expr)` and `recorded_slot_shape()`. `shaped()` is
unchanged and records `None`, so every existing producer keeps working.

`apply_optional_borrowed_element_target_view` — the five-arm idempotence check
that could not be expressed by the coarse tag — now asks
`recorded_slot_shape() == target_element` before rendering anything, and records
the shape each arm produces.

### The direction of danger, restated because it is the reason this took two passes

In the earlier migrations (`expr_is_string_view` and friends) `true` means
*already correct*, so a tag that fires wrongly is harmless. Here `true` means
*skip the adapter*, so a tag right about the family and wrong about the member
emits a **missing** adapter. That asymmetry is why the coarse tag could not be
used, and why the check is `actual == target_element` — an equality on the
precise shape — rather than a family test.

The text checks stay as the fallback for producers that have not recorded an
`actual` yet, so an untagged expression is handled exactly as before. The
migration is therefore still incremental: a producer can be given an `actual`
without auditing its consumers.

### `scripts/gate.sh --all`: zero deltas

One `type_coercion` assertion moved and it is the same shape as the four the
second pass recorded: it compared `RustExpr` structurally, and the tag wraps the
value while the *emitted Rust is byte-identical* (`values.as_deref()`). It now
compares through `into_unshaped()`, which is what it was always asserting.

### A defect the `actual` made visible

The failing assertion's dump showed the recorded shape:

```
java_ty: Array(Primitive(Int)), ownership: SharedBorrow, view: Slice,
type_identity: { declared_java_ty: Optional<int[]>, erasure: Some(Optional), … }
```

`java_ty` is `int[]` while `type_identity.erasure` still says `Optional`. That is
a stale `type_identity` — the two fields must agree and nothing enforces it,
which a survey of representation-level invariants independently identified as
the highest-value finding in `slot_shape.rs`: `java_ty` and `type_identity` are
both `pub`, `TypeIdentity::from_java_type` derives the second from the first, and
of ~20 assignments to `.java_ty` only a handful refresh it. Recorded here
because printing `actual` is what made it visible; the fix belongs with that
survey, not with this file.

### Re-measure

```
2026-08-05: 609 Raw, 49 round trips, 52 fragments, 15 `RustExpr::shaped(` sites
2026-08-06: 609 Raw, 49 round trips, 52 fragments, `actual` recorded by 1 family
```

The fragment count has not moved yet, and that is correct: the text checks are
still present as the fallback. They can be deleted for this family once every
producer feeding it records an `actual` — the same *delete the fallback last*
rule that the clone-recovery migration proved, where deleting early produced a
double clone.

### Next

1. Give the remaining optional-projection producers an `actual`, then delete the
   five text checks — that is the first fragment-count movement this file will
   see from `actual`.
2. `apply_required_borrowed_element_target_view`'s three checks are the same
   question and go the same way.
3. The anti-regression lint stays last, unchanged.

## Update 2026-08-06 (Codex follow-up) — optional projection idempotence was too weak

The fourth pass made `apply_optional_borrowed_element_target_view` tag-aware, but
the idempotence branch returned `None` when the recorded shape already matched
the target element. In this API `None` does not mean "already correct"; it means
"this helper did not handle the expression", so callers could still fall through
to text-based or broader adapters.

That branch now returns `Some(expr.clone())`. The shape comparison also ignores
`type_identity`, because this same pass recorded that `type_identity` can be
stale after changing `java_ty`; optional projection should be idempotent on the
semantic slot axes, not on that cached identity field.

One wording correction: the current migration is not purely tag-first with text
fallback. Some predicates deliberately OR the recorded tag result with rendered
text checks while producers are still mixed. That is an incremental bridge, not
the final shape.

## Update 2026-08-11 (Codex follow-up) — required borrowed-element projections now record `actual`

The previous pass left `apply_required_borrowed_element_target_view` as the same
problem in smaller form: three required-value adapters (`.as_slice()`,
`.as_mut_slice()`, `.as_str()`) still produced only text even though the target
slot shape was available at the call site.

Those producers now use `RustExpr::shaped_as(..., Some(policy.target.clone()), ...)`.
The borrowed-element adapter also checks a recorded produced slot shape before it
unwraps an optional carrier, because a second pass over an already-projected
required value otherwise rendered another `.as_ref().expect(...).as_slice()` in
front of the idempotence check.

The fallback suffix checks remain for untagged inputs; this pass is deliberately
the producer/idempotence half of the migration, not the delete-the-fallback step.
`optional_array_carrier_to_required_slice_target_borrows_before_unwrap` now pins
both the byte-identical rendered Rust and the recorded required target shape.

## Update 2026-08-11 (Codex follow-up) — more producers now carry the produced shape

Two more projection producers had the target shape in scope but still returned
only rendered text.

`apply_shape_coercion_plan`'s specialized optional string-view mapping now
records the optional element shape it produced (`OptionalProjected` with
`String`/`SharedBorrow`/`Str`) and checks that recorded shape before remapping an
already-projected value. The regression
`optional_string_carrier_to_optional_string_view_records_projection_shape` pins
both the byte-identical Rust and the idempotence path.

`apply_borrowed_set_slice_view` now records the borrowed slice target shape for
the temporary `Vec` view it creates, and checks that shape before falling back to
the rendered `.as_slice()` suffix. The existing
`set_to_borrowed_slice_collects_temporary_vec_view` test now asserts the produced
shape and second-pass idempotence.

The suffix fallbacks remain deliberately: there are still untagged producers,
and deleting text checks before all producers record `actual` would repeat the
earlier double-adapter failure mode.

## Update 2026-08-11 (Codex follow-up) — optional carrier borrow can keep shape

`project_optional_element_borrow_carrier` still has an ownership-only form for
callers that do not know the element slot, but the lowering paths that already
computed `LoweredType::OptionalElementBorrow` now call
`project_optional_element_borrow_carrier_with_shape`. That records the precise
element shape for `.as_ref()` / `.as_mut()` carrier projections instead of
returning raw text after throwing the shape away.

Pinned by `optional_element_borrow_carrier_projection_records_element_shape`.
This is another producer-side step; the text fallbacks stay until the remaining
optional projection producers are covered.

## Update 2026-08-11 (Codex follow-up) — optional borrowed unwrap records shape too

`apply_borrowed_element_borrow_adapter` had the same producer-side gap one level
later: optional borrowed-element unwraps rendered
`.expect("Java null receiver")`, `.as_ref().expect(...)`, or
`.as_mut().expect(...)` as raw text even though `adapter.element_shape` says what
the unwrap produced.

Those optional unwrap arms now record `OptionalProjected` with the produced
element shape. The direct expect arms keep the adapter's element ownership; the
`.as_ref()` and `.as_mut()` arms adjust the produced ownership to
`SharedBorrow`/`MutableBorrow` respectively. The regression
`optional_borrowed_element_unwrap_records_same_target_shape` pins the no-followup
case where the shaped projection reaches the caller unchanged.

## Update 2026-08-11 (Codex follow-up) — generic optional map projections record shape

The generic `OptionalShapeCoercionPolicy::MapElement` renderer still turned
known element projections into raw optional projections:

- `&value` -> `.as_ref()`
- `value.as_slice()` -> `.as_deref()`
- `value.as_mut_slice()` -> `.as_deref_mut()`
- `value.as_str()` -> `.as_ref().map(|value| value.as_str())`

That renderer now receives `element_plan.target`, which is the exact optional
element shape built by `shape_coercion_plan_with_facts`, and records it on those
projection arms. It also checks a recorded shape before remapping an already
projected value.

Pinned by `optional_shape_map_to_optional_slice_records_projection_shape` and
`optional_shape_map_to_optional_mut_slice_records_projection_shape`; the existing
optional string-view test covers the string arm.

## Update 2026-08-11 (Codex follow-up) — optional borrowed-element suffix fallbacks removed

The optional borrowed-element target-view adapter no longer asks rendered text
whether `.as_ref()`, `.as_mut()`, `.as_deref()`, `.as_deref_mut()`, or
`.as_ref().map(|value| value.as_str())` was already appended. It now relies on
the recorded `OptionalProjected` shape and emits the target projection whenever
the tag says it has not already happened.

Deleting the suffix checks exposed one remaining unshaped producer:
`lower_optional_owned_argument_for_owned_target` recovered `node.name` through a
raw field access before the slot-aware expression lowerer could see it. That
branch now delegates nullable receiver field reads to
`lower_optional_receiver_field_access_for_slot`, so `consume(node.name)` lowers
through the same shaped receiver-field plan as return and assignment slots.

Pinned by `optional_receiver_field_element_recovery_uses_borrowed_carrier_plan`;
the stale source assertions in `optional_element_borrow_carrier_construction_is_centralized`
were updated to reflect that optional receiver slot conversion now lives behind
`conversion_lowering::apply_optional_receiver_slot_projection`.

## Update 2026-08-11 (Codex follow-up) — required view suffix fallbacks removed

The ordinary shape-coercion path now records the final target slot shape when a
view coercion produces `StringView` or `SliceView`. That covers the non-optional
`AsStr`, `AsSlice`, and `AsMutSlice` producers, including the common
optional-unwrap-then-required-slice path.

With those producers carrying `actual`, `apply_required_borrowed_element_target_view`
no longer needs to ask rendered text whether `.as_slice()`, `.as_mut_slice()`,
or `.as_str()` is already present. It now uses only the recorded target slot
shape for idempotence, matching the optional borrowed-element adapter's rule.

Pinned by the extended `shape_coercion_owns_string_view_adapters`,
`view_coercion_policy_owns_sequence_slice_adapters`, and
`optional_sequence_to_slice_uses_borrowed_unwrap_then_slice_view` checks, plus
the earlier required borrowed-element idempotence regression.

## Update 2026-08-11 (Codex follow-up) — more clone recovery producers are tagged

`OwnedRecovery` was already the structural answer for "has this expression
already recovered an owned value?", but a few producers still returned untagged
Rust even though they were performing clone recovery:

- proven-capability optional clone adapters (`.clone()` / `.cloned()` without
  the audit comment);
- borrowed optional slice recovery (`.map(|value| value.to_vec())`);
- direct borrowed sequence recovery (`.to_vec()`).

Those paths now wrap their existing rendered Rust in `RustExprShape::OwnedRecovery`.
The emitted code is unchanged; consumers get one more structural answer before
falling back to rendered suffix checks.

## Update 2026-08-11 (Codex follow-up) — clone recovery idempotence preserves shape

The collection and sequence recovery adapters already avoided double work when
the rendered expression was visibly recovered:

- `values.iter().cloned().collect::<Vec<_>>()`;
- `groups.iter().cloned().collect::<std::collections::HashSet<_>>()`;
- `values.to_vec()`.

Those idempotent returns used to hand back the untagged input expression. They
now preserve the same emitted Rust while wrapping it in `RustExprShape::OwnedRecovery`.
That makes a second structural pass see the recovery by tag, rather than keeping
the suffix fallback permanently load-bearing for these producer paths.

## Update 2026-08-11 (Codex follow-up) — set-to-slice idempotence no longer trusts `.as_slice()`

The set-to-borrowed-slice adapter no longer keeps its rendered `.as_slice()`
compatibility fallback. A value is considered already adapted only when it
records the produced target slot shape; an untagged raw
`temporary.as_slice()` spelling is not semantic evidence and is adapted through
the ordinary set snapshot path.

The regression still keeps the positive case: a tagged slice view is returned
unchanged. The negative case now pins the 0007 boundary explicitly: raw suffix
text alone does not answer an idempotence question.

## Update 2026-08-11 (Codex follow-up) — lower clone idempotence asks the tag for broad recovery

`lower.rs` still needs the direct-clone rendered predicate where the question is
specifically `.clone()`/`.clone_box()`. But the broader
`AnyCloneRecovery` idempotence check was unnecessarily rendering before calling
the same broad predicate that `coercion.rs` had already upgraded to
tag-first/text-fallback.

That branch now passes the `RustExpr` to `rust_expr_has_clone_recovery`, so an
`OwnedRecovery` producer answers structurally before any rendered suffix check is
consulted. The direct-clone branch remains text-compatible because
`OwnedRecovery` deliberately does not distinguish direct clones from collection
or sequence recovery.

## Update 2026-08-11 (Codex follow-up) — result propagation no longer trusts raw `?`

`expr_is_result_propagated` no longer treats `RustExpr::Raw` text ending in `?`
as proof that the try operator has already been applied. That fallback was the
exact ambiguity this audit called out: a rendered question mark could be syntax
or data. The only positive answer is now the structural
`RustExprShape::ResultPropagated` tag recorded by `propagate_result_projection`.

The in-module regression keeps idempotence for tagged propagated expressions and
pins the new boundary: an untagged `Raw("call()?")` is not semantic evidence.

## Update 2026-08-11 (Codex follow-up) — slice-view identity no longer trusts raw `.as_slice()`

`expr_is_slice_view` now mirrors the result-propagation boundary: only a recorded
`RustExprShape::SliceView` answers yes. The ordinary `AsSlice` producer already
tags its output, and shape-coercion records the produced target slot shape, so
the query no longer needs to infer the semantic state from a rendered
`.as_slice()` suffix.

The in-module regression keeps the positive tagged case and pins the boundary:
an untagged `Raw("values.as_slice()")` is not considered a slice view by this
structural query.

## Update 2026-08-11 (Codex follow-up) — optional unwrap clone recovery is tagged

Two optional-clone recovery paths still returned raw strings after replacing
`value.unwrap()` with a clone-before-unwrapping form. Those paths now preserve
the same emitted Rust while recording `RustExprShape::OwnedRecovery`:

- `adapt_reused_owned_optional_argument`;
- `apply_clone_recovery_decision`;
- `optional_clone_recovery_owned_element_projection`.

This is another producer-side migration for the broad clone-recovery predicate:
later idempotence checks can see the `OwnedRecovery` tag before falling back to
rendered suffix checks.

## Update 2026-08-11 (Codex follow-up) — mutable slice producers record `MutableSliceView`

Mutable slice producers still emitted raw `value.as_mut_slice()` or reused
`SliceView`, which made mutable and shared slice views indistinguishable at the
consumer. `RustExprShape` now has a separate `MutableSliceView`, and the known
mutable producers record it while keeping the same emitted Rust:

- direct `Coercion::AsMutSlice`;
- shape-plan `BorrowSequenceMutSliceView`;
- required borrowed-element target adaptation;
- clone-recovery normalization for reused mutable-borrow array arguments.

`BorrowProjectionUse::MutableSliceView` no longer trusts an untagged raw
`.as_mut_slice()` suffix. It accepts the structural tag, or a matching recorded
mutable-borrow actual/target slot shape.

## Update 2026-08-11 (Codex follow-up) — generic clone idempotence preserves shape

The generic `Coercion::CloneRecovery` adapter already recognised an input that
had visibly recovered an owned value, such as `value.clone()`, but returned that
input unchanged. That made the first pass idempotent while keeping the next pass
dependent on rendered clone suffixes.

The idempotent return now preserves the emitted Rust and records
`RustExprShape::OwnedRecovery`. The broad clone-recovery predicate can therefore
answer structurally after one adapter pass, matching the collection and `to_vec`
idempotence migrations.

## Update 2026-08-11 (Codex follow-up) — erased-object clone recovery is tagged

`clone_erased_object_ref_once` is another owned-recovery producer: it wraps an
erased-object borrow with `java_clone_erased_object_ref(&...)`, and it also
recognises already-wrapped values to avoid double wrapping. Both paths used to
return untagged expressions.

They now preserve the same emitted Rust while recording
`RustExprShape::OwnedRecovery`, so erased-object clone recovery participates in
the same structural idempotence path as ordinary clones and sequence recovery.

## Update 2026-08-11 (Codex follow-up) — borrowed-element copy/string recovery is tagged

The borrowed-element adapter had already tagged clone recovery, but the sibling
copy and string-view recovery arms still returned untagged expressions:

- optional `.copied()`;
- optional `.map(|v| v.to_string())`;
- Vec/Set `.into_iter().copied().collect(...)`;
- Vec/Set `.into_iter().map(|v| v.to_string()).collect(...)`.

Those paths also recover owned values from borrowed elements, so they now record
`RustExprShape::OwnedRecovery` while preserving the emitted Rust.

## Update 2026-08-11 (Codex follow-up) — direct clone projections are tagged

Three remaining clone producers emitted recovery text but did not record the
recovery:

- `apply_clone_projection` (`value.clone()`);
- `Coercion::CloneBoxDynTrait` (`value.clone_box()`);
- `Coercion::CloneErasedObject` (`java_clone_erased_object_ref` /
  `java_clone_erased_object_vec_ref`).

All three now preserve their emitted Rust while recording
`RustExprShape::OwnedRecovery`. That lets broad clone-recovery idempotence ask
the expression tag first instead of rediscovering these cases from suffixes or
helper names.

## Update 2026-08-11 (Codex follow-up) — collection snapshots are tagged

The native map-view snapshot helpers materialise owned collections from
borrowed map views:

- `owned_map_keys_vec_projection`;
- `owned_map_keys_set_projection`;
- `owned_map_values_vec_projection`;
- `owned_map_values_set_projection`.

The generic collection snapshot helpers do the same for collection views:

- `owned_collection_to_vec_projection`;
- the erased-object branch of `owned_collection_to_vec_projection_for_receiver_type`.

Each helper clones keys, values, or erased objects into a fresh `Vec` or
`HashSet`, but previously returned only raw Rust. They now preserve the same
emitted Rust while recording `RustExprShape::OwnedRecovery`, so downstream
clone-recovery idempotence does not have to rediscover those snapshots from
`.cloned().collect(...)` or `java_clone_erased_object_ref(...)` text.

## Update 2026-08-11 (Codex follow-up) — mutable optional reborrows and dispatch adapters record target shape

Two more adapters had the target slot shape in scope but returned raw Rust:

- `reborrow_optional_mut_projection` appended `.as_deref_mut()` for reused
  optional mutable-borrow arguments;
- borrowed-element target dispatch adapters emitted `as &dyn Trait` or the
  concrete facade handoff block.

Both now preserve the same emitted Rust while recording the produced target slot
shape. The dispatch adapter also has a top-level recorded-target early exit, so
a second pass can prove idempotence from the slot tag before considering any
rendered text.

## Update 2026-08-11 (Codex follow-up) — optional unwrap preserves owned recovery

`apply_borrowed_element_adapter` already tagged optional copied/cloned/string
element recovery, but the final `.expect("Java null receiver")` used when the
carrier had to be unwrapped rebuilt the expression as raw text and discarded the
tag.

That tail now preserves `RustExprShape::OwnedRecovery` when the adapted value
already recorded it. The emitted Rust is unchanged; downstream recovery
idempotence can keep asking the tag after nullable borrowed-element recovery.

## Update 2026-08-11 (Codex follow-up) — map-key borrow idempotence asks the string-view tag

`BorrowProjectionUse::MapKey` still decided "already borrowed enough" from
rendered spellings: `.as_str()`, string literals, `java_string_*` helpers, or a
slot-shape fallback. A producer that had already recorded
`RustExprShape::StringView` but rendered as an opaque helper was borrowed again.

The map-key branch now accepts the recorded `StringView` tag before consulting
the rendered fallback. The regression test pins both halves: a tagged
`custom_key_view(value)` is left alone, while the same untagged raw expression is
still borrowed.

## Update 2026-08-11 (Codex follow-up) — owned string recovery is tagged

`Coercion::ToOwnedString` was another recovery producer with a rendered
idempotence check: if an input already ended in `.to_string()`, the adapter
returned it unchanged; otherwise it emitted another `.to_string()`. Both paths
left the expression untagged.

The adapter now records `RustExprShape::OwnedRecovery` on both the newly emitted
and already-owned paths. When the coercion is driven by a slot shape, the
`OwnStringFromView` policy also records the owned target slot shape, so a later
consumer can see both the coarse recovery tag and the exact produced slot.

## Update 2026-08-11 (Codex follow-up) — borrow producers are tagged

Plain borrow adapters still produced only rendered Rust:

- `Coercion::Borrow`;
- `Coercion::BorrowMut`;
- `Coercion::UnwrapOptionBorrowed`;
- `Coercion::UnwrapOptionMutBorrowed`.

They now record `RustExprShape::Borrowed` or `RustExprShape::MutBorrowed`. The
general argument/display/receiver borrow idempotence path accepts those tags, and
the mutable-borrow path accepts `MutBorrowed` alongside the more specific
`MutableSliceView`. The regression uses an opaque `custom_borrow(value)` spelling
to show the structural tag, not rendered `&...` syntax, is what prevents a second
borrow.

## Update 2026-08-11 (Codex follow-up) — forced shared borrows are tagged

`force_shared_borrow_projection` was still a smaller borrow producer that emitted
`&...` as raw Rust while throwing away the semantic fact that the result was
already borrowed. Its call sites are the lowering paths that need to force a
shared borrow around helper inputs and field/array projections, so leaving it
untagged kept those paths dependent on rendered `&` recognition.

The helper now records `RustExprShape::Borrowed` while preserving the same
emitted Rust. The existing borrow-producer regression now includes an opaque
`custom_view(value)` spelling to pin idempotence through the tag rather than
through textual borrow syntax.

## Update 2026-08-11 (Codex follow-up) — reused clone shared-borrow normalization is tagged

`normalize_reused_clone_for_borrowed_argument` already tagged the specialized
string-view, slice-view, and mutable-slice arms, but the plain shared-borrow
fallback still returned raw `&name`. That branch only runs after clone recovery
has been proven, so it is also a semantic borrow producer rather than just a
rendered spelling.

The fallback now records `RustExprShape::Borrowed` while preserving the same
`&name` output. A new `type_coercion` regression pins the shape and checks that a
second slot adaptation does not turn the reused borrow into `&&name`.

## Update 2026-08-11 (Codex follow-up) — ownership-only optional carrier projection records its family

The legacy `project_optional_element_borrow_carrier` helper only knows the
element ownership, not the full element slot shape, so it cannot safely record
the precise `actual` required by the slot-shape idempotence checks. It still
knows that `.as_ref()` and `.as_mut()` have projected an optional carrier,
though, and previously returned that semantic fact as raw Rust only.

Those two arms now record `RustExprShape::OptionalProjected` without an `actual`.
The regression pins both halves of the boundary: the emitted `.as_ref()` /
`.as_mut()` text is unchanged, and the helper does not pretend to know a precise
element slot shape.

## Update 2026-08-11 (Codex follow-up) — optional element mapping accepts opaque shaped adapters

`render_optional_element_projection` still has compatibility arms for canonical
rendered mappings such as `&value`, `value.as_str()`, and `value.as_slice()` so
the established `.as_ref()` / `.as_deref()` spellings stay unchanged. But an
opaque producer that already recorded `StringView`, `SliceView`,
`MutableSliceView`, or `Borrowed` used to fall through to the generic
`Option::map` arm, which is wrong for borrowed mappings because the carrier must
first be projected with `.as_ref()` or `.as_mut()`.

The renderer now asks the mapped value's recorded shape for those opaque cases
and emits the corresponding optional projection while recording the produced
element slot shape. The regression uses `custom_string_view(value)` and
`custom_slice_view(value)` so the behavior is proven from the tag, not from the
canonical rendered suffix.

## Update 2026-08-11 (Codex follow-up) — cast parenthesization uses expression structure first

`IndexToUsize` and primitive widening still asked whether a cast operand was
compound by rendering the expression and scanning for operator substrings. That
made the rendered text, rather than the IR node, decide whether parentheses were
needed.

Those coercions now ask the `RustExpr` structure first: `Binary` operands are
parenthesized even through a shape tag, while non-raw literals are not
parenthesized just because their emitted contents contain `" + "`. Raw input
keeps a compatibility fallback until the producers above it stop throwing away
structure, but that fallback is no longer a blind substring table: it scans only
for top-level binary operators, so `left+right` is parenthesized while
`call("left + right")` and `call(left + right)` are not misclassified as
top-level compound operands.

## Update 2026-08-11 (Codex follow-up) — optional construction records a producer tag

`rust_expr_is_constructed_optional_value` still had to rediscover present
optional values from rendered `Some(...)`, raw `if ... { Some(...) }`, or a broad
`.get(...).cloned()` substring. The `Some` producers now record
`RustExprShape::OptionalConstructed`, and the predicate accepts that producer tag
before looking at structure or raw text. `OptionalProjected` stays separate: it
can be an element borrow projection and must not suppress a later required
borrow.

The raw `.get(...).cloned()` compatibility fallback was also narrowed: it now
requires the receiver to be a terminal `.get(...)` call. This keeps
`fields.get(k).cloned()` but rejects chained expressions such as
`values.get(index).into_iter().cloned()`, where the earlier `.get(` was not the
optional carrier being cloned.

## Update 2026-08-11 (Codex follow-up) — mutable alias fallback is rooted, not arbitrary `contains`

`rendered_mutable_alias_projection` still has a raw compatibility path because
not every same-call argument producer carries a semantic alias tag. The broad
`contains(".as_mut().expect(...)")` and `contains(".as_deref_mut()")` checks were
too loose, though: `values.map(|item| item.as_deref_mut())` was classified as a
mutable alias of `values` even though the mutable projection was nested inside
the closure body.

The fallback now accepts the same rooted spellings by suffix:
`values.as_mut().expect(...)`, `values.field.as_mut().expect(...)`,
`values.as_deref_mut()`, and `values.field.as_deref_mut()`. Nested occurrences
inside another expression no longer count as proof that the prior argument is a
mutable alias of the base local.

## Update 2026-08-11 (Codex follow-up) — constructor self detection is token-aware

`rust_expr_mentions_constructor_self_temporary` already recursed through the
structured `RustExpr` arms, but its raw compatibility fallback accepted any
rendered substring containing `__java_self`. That meant string literals,
raw-string literals, and longer identifiers such as `not__java_self` could be
treated as evidence that a constructor temporary was actually referenced.

The raw fallback now scans for an identifier token outside string, char, and raw
string literals. This keeps the compatibility path for opaque producers such as
`helper(__java_self.clone())`, while rejecting literal text and longer
identifiers instead of relying on a blind substring check.

## Update 2026-08-11 (Codex follow-up) — arithmetic display stripping is structure-first

`strip_arithmetic_display_conversion` was the other concrete operator-substring
site named in the original audit. It stripped `.to_string()` whenever the
rendered expression contained spaced arithmetic fragments such as `" + "` or
`" * "`, which made string contents and nested calls look like arithmetic.

`render_java_display_operand` now asks the `RustExpr` structure first: a
structured zero-argument `.to_string()` call on a `Binary` arithmetic receiver is
unwrapped without rendering and reparsing the expression. The remaining raw
compatibility path is narrower than the old substring check: it only accepts a
terminal `.to_string()` whose receiver is a balanced parenthesized arithmetic
expression, or the existing generated numeric-cast shape
`value * (100 as f64).to_string()` where the terminal `.to_string()` belongs to
the right arithmetic operand. The shared raw operator scanner skips ordinary and
raw string literals. Cases such as `left + right.to_string()`,
`("left + right").to_string()`, and `(r#"left + right"#).to_string()` stay
untouched.

## Update 2026-08-11 (Codex follow-up) — string literal queries use literal nodes

`string_literal_source` rendered any `RustExpr`, stripped a textual
`.to_string()` suffix, and accepted any text that started and ended with `"`.
That made opaque raw fragments such as `"bin"` or `"%s".to_string()` semantic
proof that the expression was a source string literal. The query drives
`String.join` delimiter/value handling and `String.format` template lowering, so
the raw text was load-bearing.

The query now accepts only `RustExpr::Literal` string tokens, including the
existing structured literal-token spelling that carries a `.to_string()` suffix,
plus a structured zero-argument `.to_string()` method call whose receiver is
such a literal. Raw quoted text falls through to the runtime join/format paths.
That keeps source string literals fast and explicit while removing another
render-then-reparse semantic decision.

## Update 2026-08-11 (Codex follow-up) — clone receiver casts use the bounded scanner

Clone recovery still has a raw compatibility path because not every producer
preserves receiver structure. Its method-receiver guard used to parenthesize any
rendered expression containing `" as "`, including string literals and macro
arguments that merely mentioned those characters.

That guard now reuses the balanced top-level cast scanner already used by cast
receiver rendering. Receiver-level casts such as `value as Target` are still
parenthesized before `.clone()`, while raw strings such as `" as "`, raw string
literals, and nested string contents such as `format!(" as ")` no longer make
the direct-clone semantic decision. Audited clone recovery and raw `.to_vec()`
recovery use the broader literal-aware token scanner because established
generated code parenthesizes indexed receivers such as
`Table::VALUES[index as usize]` in those recovery paths. Structured index
receivers also keep their own syntax guard for owned sequence recovery instead
of depending on a blind rendered substring.

## Update 2026-08-11 (Codex follow-up) — erased-object clone idempotence ignores literal text

`clone_erased_object_ref_once` kept erased-object clone recovery idempotent by
checking whether the rendered expression contained
`java_clone_erased_object_ref(` or `java_clone_erased_object_vec_ref(` anywhere.
That made string contents semantic: a raw expression containing the helper name
inside a string literal was treated as already recovered and skipped the required
wrapper.

The fallback now scans for an actual helper-call token outside string, char, and
raw-string literals. Existing recovered calls still stay idempotent, while
literal text containing the helper spelling is wrapped like any other bare
erased-object value.

## Update 2026-08-11 (Codex follow-up) — optional unwrap detection ignores literal text

`rendered_expr_has_optional_unwrap_or_java_null_expect` used broad
`.contains(".unwrap()")` and `.contains(".expect(\"Java null receiver\")")`
checks when deciding whether a reused owned optional argument had already been
unwrapped. That made string contents semantic in the same way: a raw expression
that only mentioned those spellings inside a literal could be treated as already
unwrapped.

The compatibility fallback now scans outside string, char, and raw-string
literals. Real generated unwrap/expect spellings still count, but literal text
does not make `adapt_reused_owned_optional_argument` skip the owned optional
recovery path.

## Update 2026-08-11 (Codex follow-up) — optional string-view construction has a structural path

`normalize_optional_constructed_string_view_expr` still rendered the whole
optional construction and peeled `Some(&...as_str())` from text. That was a
compatibility spelling check where the structured producer already knows the
important fact: `Some` contains a borrowed expression whose element records
`RustExprShape::StringView`.

The normalizer now handles structured `Some(&<StringView>)` by asking the
element tag and removing the extra borrow without rendering the optional
expression. The raw `Some(&...as_str())` spelling stays as a compatibility
fallback for untagged producers, but the tagged path no longer depends on that
suffix.

## Update 2026-08-11 (Codex follow-up) — raw `if` optional-construction fallback ignores literals

The remaining raw compatibility arm in
`rendered_expr_is_constructed_optional_value` accepted any rendered expression
starting with `if ` and containing `{ Some(`. That kept old generated conditional
`Option` values idempotent, but it also made string contents semantic:
`if ready { format!("{ Some(value)") } else { None }` was classified as an
already-constructed optional value.

The fallback now scans for `{ Some(` outside string, char, and raw-string
literals. Structured `If`/`Some` expressions still answer through the IR, and
generated raw conditionals keep the compatibility path without letting literal
text suppress optional construction.

## Update 2026-08-11 (Codex follow-up) — raw inherent impl owner parsing handles nested generic bounds

`occupied_inherent_methods_for_items` still has to inspect `RustItem::Raw`
support items so the fallback method generator does not emit a duplicate
inherent `to_string`/`get_class` method. Its compatibility parser skipped generic
parameters with `find('>')`, which stopped at the first close angle. A raw item
like `impl<T: Into<Foo>> Boxed<T> { pub fn to_string(...) ... }` therefore
recorded the owner as `>` instead of `Boxed`, and duplicate fallback suppression
missed the real method.

The raw parser now accepts both `impl Owner` and `impl<T> Owner` prefixes, then
uses a balanced angle-bracket scan for the leading generic parameter list. Trait
impls are still ignored, and the regression covers both the owner helper and the
occupied-method integration. This is still a compatibility path over raw Rust
text; the broader 0007 end state remains to avoid making decisions from rendered
source at all.

## Update 2026-08-11 (Codex follow-up) — slice and borrow compatibility checks are narrower

Two more raw-text compatibility shortcuts were narrowed.

`Coercion::AsSlice` used to skip appending `.as_slice()` whenever rendered text
contained `::` and did not contain `(`. That was meant for associated paths such
as `Owner::VALUES`, but it also treated quoted raw text containing `::` as
already slice-like. The exception now accepts only path-like associated items,
and records `SliceView` when it takes that path.

`parenthesize_borrow_operand` used to treat any raw operand containing `[` as
prefix-safe. `left + values[i]` therefore borrowed as `&left + values[i]`
instead of `&(left + values[i])`. The indexed fast path now accepts only simple
index chains such as `values[i]`, while compound indexed expressions are
parenthesized for shared and mutable borrow projections.

## Update 2026-08-11 (Codex follow-up) — optional clone recovery unwrap rewrite ignores literals

`replace_optional_clone_recovery_unwrap` still used a broad
`rendered.contains("value.unwrap()")` check before rewriting reused optional
values to clone through `.as_ref().cloned().expect(...)`. That made literal text
semantic: raw strings or format arguments that merely mentioned `value.unwrap()`
could be rewritten as if the generated expression had actually consumed the
optional carrier.

The rewrite now uses the same string/char/raw-string-aware scanner as the
optional unwrap detector. Real generated `value.unwrap()` and
`value.clone().unwrap()` expressions still take the recovery path, including the
erased-object variant, but normal and raw string literals containing those
spellings are left untouched.

## Update 2026-08-11 (Codex follow-up) — downcast expect-message rewrite ignores literals

`rewrite_downcast_expect_message` still decided from broad rendered-text
membership: if the expression text contained both the downcast turbofish marker
and the invalid-cast `.expect(...)` marker, it replaced the first matching
expect message. That made literal text semantic in the same way as the optional
unwrap rewrite: a format string or raw string mentioning both markers could be
treated as if it were an actual generated downcast.

The rewrite now first requires the downcast marker outside string, char, and
raw-string literals, then replaces only an invalid-cast expect marker found by
the same scanner. Real generated `value.downcast::<T>().expect(...)` output
still receives the clearer replacement message; literal marker text is left
unchanged.

## Update 2026-08-11 (Codex follow-up) — invalid-pattern counting ignores literals

`output_markers::count_invalid_patterns` still counted unsupported-output
markers with plain `line.contains(...)`. That is acceptable for real generated
comments and macros, but it also made ordinary string contents affect the
translation metrics: a generated string mentioning `compile_error!`, the
downcast marker pair, an untranslated-call marker, the unsupported inner-capture
marker, or the `= None;` heuristic could be counted as an actual invalid
translation pattern.

The counter now scans each line outside string, char, and raw-string literals
for the macro/call/downcast/`None` patterns whose real emitted evidence starts
outside literal data. Markers in generated code and comments still count; those
marker spellings inside ordinary literal data do not. The `= None;` heuristic
keeps its existing exclusions for `let mut` and explicitly typed `Option`
initializers, but those checks are now literal-aware too.

Two message markers deliberately keep the old whole-line match:
`unsupported Java array creation` and `unsupported Java inner-class outer
capture construction`. Their emitters put the marker text inside
`compile_error!`/`panic!` message strings, and `tests/output_markers.rs` pins
that those message strings remain visible to the corpus metric.

## Update 2026-08-11 (Codex follow-up) — raw self-receiver replacement ignores literals

Constructor delegation still has a raw compatibility fallback that rewrites
lowered uses of `self.field` to the temporary constructor receiver
(`__java_self.field` or `__java_self.clone().field`). That fallback used
`text.replace("self.", ...)`, so arbitrary rendered text could become semantic:
string and raw-string literals containing `self.value` were rewritten, and an
identifier like `myself.value` was changed in the middle.

The fallback now scans the raw text, rewrites only `self.` prefixes outside
string, char, and raw-string literals, and requires an identifier boundary before
`self`. Structured `RustExpr::Field(self, ...)` still takes the structural path;
the scanner only narrows the raw escape hatch. The statement-level constructor
trailing rewrite uses the same helper, so raw expression and raw statement
fallbacks agree.

## Update 2026-08-11 (Codex follow-up) — anonymous dyn-trait clone exception is exact

`Coercion::CloneBoxDynTrait` has one legitimate "do not append
`.clone_box()`" case: the placeholder constructor emitted for anonymous trait
objects, `<dyn Trait>::anonymous()`, already returns an owned `Box<dyn Trait>`.
That exception was detected with `rendered.starts_with("<dyn ") &&
rendered.contains(">::anonymous()")`, so a later literal argument mentioning the
same suffix could suppress a required clone-box recovery.

The check is now exact for the generated constructor shape:
`trim().starts_with("<dyn ") && trim().ends_with(">::anonymous()")`. The real
anonymous placeholder remains unchanged, while expressions such as
`<dyn Trait>::named(">::anonymous()")` take the normal clone-box recovery path.

## Update 2026-08-11 (Codex follow-up) — unimplemented counter ignores literals

`output_markers::count_unimplemented_facade_methods` still counted
`unimplemented!(` with plain `line.contains(...)`. A generated string or raw
string mentioning the macro could therefore inflate the facade-stub metric even
though no stub body existed on that line.

The counter now uses the same literal-aware line scanner as
`count_invalid_patterns`: real macro text in generated code and comments still
counts, while normal and raw string literal contents do not.

## Update 2026-08-11 (Codex follow-up) — compile-error counter ignores literals

`output_markers::count_compile_error_markers` still counted
`compile_error!(` with `line.matches(...)`. That made generated literal data
look like a refused translation whenever a string or raw string mentioned the
macro call spelling.

The counter now uses the same outside-literals scanner and counts all real
occurrences on a line without counting normal or raw string literal contents.

## Update 2026-08-11 (Codex follow-up) — stubbed-constant counter requires syntactic anchors

`output_markers::count_stubbed_constants` has to keep matching the unmodelled
constant message inside a `compile_error!` string, because that is how the
marker is emitted. But its syntactic anchors, `pub const` and `compile_error!`,
were also broad line-substring checks, so a generated string containing a whole
fake constant declaration could be counted as a real stubbed constant.

The counter now requires `pub const` and `compile_error!` outside string, char,
and raw-string literals while keeping the prefix/suffix message check unchanged.
Real generated unmodelled constants still count; quoted diagnostic examples do
not.

## Update 2026-08-11 (Codex follow-up) — no-op facade method marker ignores literals

`output_markers::count_noop_facade_call_sites` counts no-op facade method calls
from an explicit generated marker comment. The constructor half already skipped
strings and comments while scanning for `Type::new(...)`, but the method-marker
half used a whole-file `matches(...)` over the marker prefix. A generated string
or raw string containing `noop Java facade method call: ...` therefore counted
as a call site.

The method-marker half now uses the same outside-literals line scanner as the
other counters. Marker comments still count, because comments are the intended
carrier; normal and raw string literal contents do not.

## Update 2026-08-11 (Codex follow-up) — unmodelled-upcast counter ignores literals

`output_markers::count_unmodelled_upcast_conversions` still counted
`Unmodelled Java upcast: ...` with a whole-line substring check. The real
generated marker is carried in doc comments, so comments still need to count,
but quoted diagnostic examples or raw string fixtures should not move the
metric.

The counter now uses the shared outside-literals scanner. Generated upcast
comments still count as before, while normal and raw string literal contents do
not.

## Update 2026-08-11 (Codex follow-up) — no-op constructor call scan skips raw strings

The constructor half of `count_noop_facade_call_sites` intentionally uses a
small scanner instead of the shared outside-literals helper because constructor
calls inside `//` comments should not count. That scanner skipped ordinary
strings and line comments, but not raw strings, so a diagnostic raw string such
as `r#"Font::new_3(...)"#` could be scored as a no-op facade constructor call.

The scanner now skips raw-string literal spans using the same raw-string helper
as the other output-marker counters. Real `Type::new...(` calls still count,
while ordinary strings, raw strings, comments, and identifier-prefix collisions
do not.

## Update 2026-08-11 (Codex follow-up) — inner-constructor raw rewrite ignores literals

The 0023 companion-constructor fallback in `lower.rs` still scanned raw rendered
text with `find("::new")`. It then asked the structured companion-path predicate
whether the surrounding path was a same-outer captured inner constructor, but
the initial match could come from string, char, or raw-string literal data.

That fallback now finds `::new` only outside literals before applying the same
path predicate. Literal diagnostic text mentioning `OuterB::new()` is left
unchanged, while real raw fallback calls still rewrite to the `_with_outer`
companion and receive `__outer.clone()`.

## Update 2026-08-11 (Codex follow-up) — optional erased-string rewrites ignore literals

`rewrite_optional_erased_borrowed_string_view_call_args` still used whole-string
`contains`/`replace` for optional erased `String` handoff fragments. A generated
string or raw string containing
`name.map(|value| Box::new(value.clone()) as Box<dyn Any>)`, or the
`name.unwrap().downcast::<String>()...as_str()` suffix, could therefore be
rewritten as if it were executable Rust.

The named-local rewrites now replace only matches outside string, char, and
raw-string literals, and the generic suffix rewrite uses the same
outside-literals finder. Real optional erased-string view handoffs still rewrite
to borrowed `as_ref()`/`downcast_ref()` forms; literal text stays unchanged.

## Update 2026-08-11 (Codex follow-up) — no-op constructor impl depth skips literal braces

`output_markers::noop_facade_constructors` tracked the current inherent `impl`
with `line.matches('{')` and `line.matches('}')`. A string, char, raw string, or
comment containing `}` inside a preceding method could therefore close the
tracked `impl` early and make a later no-op facade constructor invisible to the
call-site counter.

The constructor collector now counts only syntax braces for this depth tracking:
normal strings, char literals, raw strings, line comments, and inline block
comments are skipped. The existing multiline-impl regression now includes those
literal/comment braces before the no-op constructor and still finds the call.

## Update 2026-08-11 (Codex follow-up) — erased static helper rewrite ignores literals

`rewrite_erased_source_static_helper_calls_for_trait` rewrites generated source
static helper calls from `owner_helper(...)` to `Self::helper(...)` when building
the erased static trait surface. It searched the entire rendered function with
`rendered.find(prefix)`, so a string or raw string mentioning
`owner_helper()` could be rewritten as if it were executable Rust.

The rewrite now searches for the owner helper prefix outside string, char, and
raw-string literals before applying the existing identifier/function-name check.
The regression `erased_static_trait_helper_rewrite_ignores_literal_mentions`
keeps literal mentions unchanged while rewriting the real helper call.

## Update 2026-08-11 (Codex follow-up) — line scanners skip multiline raw-string openers

The shared output-marker line scanner skipped raw string contents only when the
raw string terminator appeared on the same line. A valid multiline raw string
whose opening line contained `compile_error!(`, `.put(`, or another marker
spelling was therefore counted as generated Rust syntax.

`raw_string_literal_end_at` now treats a raw string opener without a same-line
terminator as consuming the rest of that line. The scanner is still line-based,
but the opening line no longer leaks literal marker text into counters. The
regression `invalid_pattern_count_ignores_multiline_raw_string_opening_line`
pins that boundary through the shared invalid-pattern detector.

## Update 2026-08-11 (Codex follow-up) — no-op facade definitions require `fn` lines

The no-op facade method and constructor detectors accepted broad line suffixes
such as `{ Default::default() }` and `{ self.clone() }`. That meant a comment or
literal-looking line containing `pub fn new... { Default::default() }` could be
treated as a real no-op facade definition; for constructors, that fake
definition could then make later `Type::new...(` calls look like erased facade
cost.

The detectors now first parse a `fn`/`pub fn` line shape and extract the function
name from that syntax before applying the no-op-body check. Comments and
assignment/macro text no longer define facade methods or constructor call-site
paths, while real generated one-line facade bodies still count.

## Update 2026-08-11 (Codex follow-up) — clone-recovery comment stripping ignores literals

`rendered_expr_has_clone_recovery` tolerates clone-recovery comments by first
removing Rust block comments, but the stripper used plain `find("/*")`. A normal
or raw string containing `/* ... */` could therefore be treated as if part of
the expression had been commented out before clone-recovery idempotence checked
the suffix.

The stripper now finds block comments only outside string, char, and raw-string
literals. Actual generated clone-recovery comments are still ignored for
idempotence, while comment-looking literal contents are preserved unchanged.

## Update 2026-08-11 (Codex follow-up) — `.get(...).cloned()` fallback skips raw strings

The optional-construction compatibility predicate still has to recognise raw
generated collection lookups such as `map.get(key).cloned()`, but its terminal
call parser skipped normal strings and chars only. A valid raw string argument
containing an unescaped quote and parenthesis could make the parser consume the
raw-string contents as Rust syntax and miss the real closing `get(...)`.

`rendered_terminal_method_is_get` now skips raw-string literal spans with the
same helper used by the other 0007 scanners. The public
`rust_expr_is_constructed_optional_value` regression keeps real
`.get(r#"..."#).cloned()` projections recognised while raw-string text that
merely mentions `values.get(index)` remains inert.

## Update 2026-08-11 (Codex follow-up) — no-op facade counters skip multiline raw-string contents

`count_noop_facade_methods` and the constructor half of
`count_noop_facade_call_sites` had been narrowed to syntactic-looking `fn` and
`impl` lines, but they still processed each line independently. A multiline raw
string whose content line began with `pub fn fake(...) { Default::default() }`
or `impl Font { pub fn new(...) ... }` could therefore be counted as generated
facade code, and the fake constructor could make later literal call examples
look like no-op facade cost.

Those counters now carry multiline raw-string state before applying their line
shape checks. Real generated one-line facade bodies still count; raw-string
documentation/examples spanning multiple lines do not define methods or
constructors for the metric.

## Update 2026-08-11 (Codex follow-up) — shared marker counters keep multiline raw-string state

The shared outside-literals scanner treated a multiline raw-string opener as
literal text only on the opening line. Counter paths that walked one line at a
time still saw later raw-string body lines as executable Rust, so marker text
such as `compile_error!(`, `unimplemented!(`, unmodelled-upcast comments, or a
fake unmodelled `pub const` could move metrics if it appeared after the first
line of a raw string.

The marker counters now elide multiline raw-string spans before applying their
existing single-line literal-aware checks. If a raw string closes and real code
continues on the same line, the suffix is still scanned; only the raw-string
body is inert. This covers invalid-pattern, compile-error, unimplemented-facade,
stubbed-constant, unmodelled-upcast, and no-op facade call-site metrics.

## Update 2026-08-11 (Codex follow-up) — raw inherent-method scan ignores literals

`occupied_inherent_methods_for_items` still scans raw Rust items because some
fallback impls are emitted as `RustItem::Raw`. That scanner used line prefixes
directly: a documentation raw string or block comment containing
`impl Widget { pub fn to_string(...) ... }` could be treated as a real inherent
method and suppress the generated `to_string` / `get_class` fallback for the
type. A literal `}` could also end the tracked impl owner early.

The scanner now elides strings, raw strings, line comments, and block comments
before parsing `impl` / `fn` lines, and it updates impl depth from code-only
braces. The new regressions keep literal/comment method examples inert while
real raw inherent methods are still detected.

Follow-up 2026-08-26: the scanner is now scoped to the only semantic question
that still asks it anything: whether generated Object fallback methods would
duplicate an existing raw inherent `to_string` or `get_class`. Other raw methods
are no longer recorded as occupied-method facts. This keeps the compatibility
guard while shrinking the raw-text-derived fact surface. Covered by
`lower::tests::raw_inherent_methods_ignore_literal_and_comment_mentions` and
`lower::tests::raw_inherent_method_scan_ignores_literal_braces_for_impl_depth`.

## Update 2026-08-11 (Codex follow-up) — raw impl owner parsing keeps HRTB bounds

The same raw inherent-method scanner excluded trait impls with a broad
`contains(" for ")` check. That also rejected real inherent impls whose generic
parameter list or `where` clause used higher-ranked trait bounds such as
`for<'a> Parser<'a>`, making fallback method occupancy depend on a substring
inside a bound.

`raw_inherent_impl_owner` now parses the first impl target and only treats a
following top-level `for` as a trait impl marker. Higher-ranked bounds remain
part of the generic syntax; generic trait impls like `impl<T> Display<T> for
Boxed<T>` are still excluded.

## Update 2026-08-11 (Codex follow-up) — output-marker impl owner parsing keeps HRTB bounds

`output_markers` had its own raw inherent-impl owner parser for the no-op facade
constructor call-site metric, and it kept the same broad trait-impl shortcut.
That made the metric depend on whether `for` appeared inside a generic bound
rather than after the impl target.

The metric parser now uses the same top-level rule as `lower.rs`: parse the
first owner target, skip its own generic arguments, and only exclude the header
when the next token is trait-impl `for`. The regression keeps an inherent
`impl<T> Widget<T> where T: for <'a> Parser<'a>` constructor visible while a
generic trait impl remains invisible to the no-op constructor call-site count.
The constructor path extraction now also reuses the already-validated function
name instead of reparsing the raw line with `split_once("fn ")`.

## Update 2026-08-11 (Codex follow-up) — unsupported marker patterns skip literals too

`count_invalid_patterns` had been converted to the outside-literals scanner for
most marker spellings, but two adjacent arms still used direct substring checks:
`unsupported Java array creation` and `unsupported Java inner-class outer
capture construction`. That meant a generated string or raw string containing
those diagnostic words could still move the invalid-pattern metric.

The array-creation arm now uses the same string/char/raw-string-aware scanner as
the other invalid-pattern markers. The inner-class outer-capture arm does the
same for comments and raw syntax, but also accepts marker text inside a real
`panic!` call because that is how the translator deliberately emits this
diagnostic. The existing literal-marker regression now covers both unsupported
marker texts, while `tests/output_markers.rs` keeps the panic fixture connected
to the `invalid_patterns` counter.

## Update 2026-08-11 (Codex follow-up) — borrowed string literals record `StringView`

`literal_slot_plan` already recognised a Java string literal flowing into a
borrowed `&str` target, but `apply_literal_slot_plan` emitted only a plain
`RustExpr::Literal("\"...\"")`. Downstream string-view consumers therefore still
needed the rendered-literal fallback to know the value was already a string
view.

Borrowed string-view literal plans now wrap the literal with
`RustExprShape::StringView`. The emitted Rust is unchanged; the existing
literal-slot regression now pins the recorded tag, and the numeric-char literal
case stays untagged.

## Update 2026-08-11 (Codex follow-up) — generic optional maps preserve target shape

`render_optional_element_projection` had precise target-element shape for every
optional map it rendered, but the final generic `map(|value| ...)` fallback
returned raw text while the specialized borrow/string/slice arms recorded
`OptionalProjected`.

The generic fallback now records the same `OptionalProjected` family plus the
target element slot shape. This does not change emitted Rust; it closes another
producer that forced later optional-map idempotence to rediscover semantics from
rendered text.

## Update 2026-08-11 (Codex follow-up) — collection clone adapters ask `OwnedRecovery` first

The borrowed Vec/Set clone adapters already tagged their own outputs as
`OwnedRecovery`, but their idempotence check still started from rendered
`.cloned().collect(...)` suffixes. A previous producer that had already recorded
owned recovery with an opaque helper spelling was cloned again.

Both adapters now return an existing `OwnedRecovery` expression before checking
textual suffix compatibility for untagged legacy inputs. The collection clone
recovery regression now includes opaque tagged Vec and Set recoveries to pin the
structural path.

## Update 2026-08-11 (Codex follow-up) — optional string-view projection records its family

`optional_string_view_projection` is used by equality/operator lowering to turn
an optional string carrier into `Option<&str>` with
`.as_ref().map(|value| value.as_str())`. It knew that an optional carrier had
been projected, but returned raw Rust with no shape tag.

The helper now records `RustExprShape::OptionalProjected` while preserving the
same emitted Rust. It deliberately does **not** record a precise slot shape,
because this operator-side helper has no target `RustSlotShape` in scope. The
regression pins both facts: the emitted expression is unchanged and the family
tag is present without a recorded element slot.

## Update 2026-08-11 (Codex follow-up) — optional copied expect is owned recovery

`optional_copied_expect_projection` emitted
`.copied().expect("Java null receiver")`, recovering an owned copy from a
borrowed optional element, but returned an untagged raw expression.

It now records `RustExprShape::OwnedRecovery` while preserving the same emitted
Rust. The existing JDK projection spelling test also pins the recovery tag, so
later recovery/idempotence checks can ask the expression shape before falling
back to rendered suffix compatibility.

## Update 2026-08-11 (Codex follow-up) — optional owned-value projections are tagged

Two optional-value helpers cloned an owned value out of a borrowed optional
carrier but returned only raw rendered Rust:

- `optional_get_owned_value_projection`;
- `optional_or_else_owned_value_projection`.

Both now record `RustExprShape::OwnedRecovery` while keeping the exact same
emitted expressions. The JDK platform projection test pins the rendered
`.as_ref().map(|value| (*value).clone())...` spellings and the recovery tag.

## Update 2026-08-11 (Codex follow-up) — typed `None` is an optional carrier

`typed_none_projection` produced `None::<T>` as raw Rust even though it is the
absent-value sibling of the already-tagged `Some(...)` construction path.

The helper now records `RustExprShape::OptionalConstructed` while preserving the
same emitted Rust. The JDK platform projection test pins both the
`None::<Box<dyn std::any::Any>>` spelling and the fact that
`rust_expr_is_constructed_optional_value` answers structurally for the tagged
expression.

## Update 2026-08-11 (Codex follow-up) — shared optional unwrap needs a narrower tag

`unwrap_optional_shared_value` looks like an optional projection because it
lowers a known-present optional carrier with `.as_ref().unwrap()`, but tagging it
with the existing `RustExprShape::OptionalProjected` is too broad. The
`slot_shape_design` gate moved from `525/75` to `524/76`: current
`OptionalProjected` consumers treat that family as an optional-value projection,
while this helper produces a required borrowed element.

The attempted tag was reverted. This producer should stay raw until the IR has a
separate "required element projected from optional carrier" shape, or until the
existing consumers are split so required unwrapping and optional mapping are not
collapsed into one `OptionalProjected` answer.

## Update 2026-08-11 (Codex follow-up) — dyn-trait borrow projection is tagged

`borrow_as_dyn_trait` emitted `&value as &dyn Trait` as raw Rust even though the
expression is a required shared borrow. That made borrow idempotence rely on the
rendered `&...` spelling for this dyn-trait projection path.

The helper now records `RustExprShape::Borrowed` while preserving the emitted
Rust. The existing borrow-producer idempotence test pins the dyn-trait spelling,
the structural tag, and the fact that `ensure_borrow_projection` returns the
tagged expression unchanged.

## Update 2026-08-11 (Codex follow-up) — clone-recovery comment matches are no longer documented as live

`rendered_expr_has_clone_recovery` had already stopped matching the
`/* TODO(translation): validate added clone */` text directly: it strips trailing
block comments first, then checks generic clone suffixes. The lower
`rust_expr_has_clone_recovery` doc comment still described the old eight-arm,
comment-wording-sensitive fallback as if it were live.

The stale comment is now aligned with the current invariant: recorded
`OwnedRecovery` wins structurally, and the legacy rendered fallback is
comment-insensitive while untagged producers remain.

## Update 2026-08-11 (Codex follow-up) — raw string-view fallback requires a complete literal

`rendered_expr_is_already_string_view` still has to answer for untagged literal
producers, but its raw-string branch accepted any rendering that merely started
with `r"` or `r#"`. That made incomplete raw text and raw literals with suffix
calls look like borrowed string views.

The fallback now reuses the local raw-string literal parser and only accepts a
raw string when the parsed literal spans the whole rendered expression. The
string-view fallback regression keeps real raw literals positive and pins
`r#"literal"#.to_string()` plus an unterminated raw string as negative cases.

## Update 2026-08-11 (Codex follow-up) — borrow operands only trust complete string literals

`parenthesize_borrow_operand` used a broad prefix-safe rule for rendered string
literals: anything starting with `"` or `r"`/`r#"` was left unparenthesized.
That means a raw producer such as `"literal".to_string()` could be borrowed as
`&"literal".to_string()`, where the borrow applies to the literal before the
method call instead of to the whole expression.

The prefix-safe rule now accepts ordinary and raw string literals only when a
literal parser consumes the entire rendered expression. Complete literals still
borrow without parentheses, while normal and raw string suffix calls are
parenthesized before the borrow.

## Update 2026-08-11 (Codex follow-up) — ordinary string-view literals use the same complete-literal check

The preceding raw-string fix left the ordinary string branch using
`starts_with('"') && ends_with('"')`. That still let rendered text such as
`"left" + "right"` answer "already a string view" just because the whole
fragment happened to begin and end with quotes.

`rendered_expr_is_already_string_view` and the borrowed-map-key fallback now
share `rendered_expr_is_complete_string_literal`, so ordinary and raw literals
have the same rule: the parsed literal must span the entire rendered expression.
The regression keeps escaped ordinary literals positive and pins suffix and
concatenation-shaped renderings as negative.

## Update 2026-08-11 (Codex follow-up) — raw `Some(...)` fallback requires the outer call to span the fragment

`rendered_expr_is_constructed_optional_value` still accepted top-level raw
`Some(...)` by prefix alone. That made incomplete or suffixed raw fragments such
as `Some(value` and `Some(value).map(...)` look like already-constructed optional
values.

The raw compatibility path now finds the matching close paren for the outer
`Some(` while skipping normal strings, chars, and raw strings, and accepts the
fragment only when that close paren is the end of the rendered expression.
`unwrap_some_expr` uses the same parser, so raw unwrapping and raw construction
detection agree.

## Update 2026-08-11 (Codex follow-up) — cast receiver scanner skips raw-string spans

`parenthesize_cast_method_receiver` uses a top-level `as` scanner before adding
method calls such as `.clone()`. It skipped ordinary strings and chars, but raw
strings were only handled accidentally by the ordinary quote state machine. A
raw string containing a quote before `as`, for example
`r#"quoted " as text"#`, could therefore be treated as a cast expression.

The scanner now jumps over raw-string literal spans using the same local parser
as the other 0007 fallbacks. The clone-projection regression keeps a real
receiver-level cast parenthesized while preserving raw-string literals that
contain `as` text.

## Update 2026-08-11 (Codex follow-up) — boxed dyn-trait borrows are tagged

`BorrowDynTrait` already recorded `RustExprShape::Borrowed`, but its sibling
`BorrowBoxedDynTrait` still emitted only raw `value.as_ref() as &dyn Trait`
text. That kept the boxed trait-object route dependent on the generic rendered
borrow fallback for idempotence.

The boxed helper now records the same `Borrowed` shape while preserving the
emitted Rust. The borrow-producer idempotence test covers both the exact spelling
and the structural fast path through `ensure_borrow_projection`.

## Update 2026-08-11 (Codex follow-up) — concrete facade borrows are tagged

The remaining sibling in the same coercion arm was `BorrowConcreteFacade` for
non-native facade targets. It emitted the handoff block
`{ let _java_value = value; &Facade }` as raw Rust, even though the result is a
required borrow.

That producer now records `RustExprShape::Borrowed` without changing the emitted
block. The borrow-producer idempotence test covers the facade spelling and the
structural no-double-borrow path.

## Update 2026-08-11 (Codex follow-up) — anonymous dyn-trait placeholders are tagged

`CloneBoxDynTrait` skipped clone recovery for `<dyn Trait>::anonymous()` by
checking the rendered placeholder spelling. The producer already knew this was
the anonymous placeholder, but returned raw Rust and forced the consumer to
rediscover that fact from text.

`anonymous_dyn_trait_projection` now records `RustExprShape::AnonymousDynTrait`.
The clone-box adapter asks that tag first and preserves the tagged expression
unchanged; the rendered fallback remains only for legacy raw placeholders.

## Update 2026-08-11 (Codex follow-up) — `.into()` clone recovery uses structured calls

Clone recovery preserves conversion suffixes by turning `value.into()` into
`value.clone()...into()`, but that path was still driven by
`rendered.strip_suffix(".into()")`. The producer for external `into` coercions
also emitted raw `.into()` text, so the consumer had no structure to ask.

`into_projection` now builds a `RustExpr::MethodCall` for `.into()`, and
`ExternalInto` uses that helper. `apply_clone_recovery` handles a structured
zero-argument `into` call by recovering the receiver and reattaching the method
call, preserving the emitted Rust while avoiding the suffix parser on the main
producer path. The rendered fallback remains for legacy raw `.into()` text.

## Update 2026-08-11 (Codex follow-up) — optional get projections unwrap shape tags

`rust_expr_is_constructed_optional_value` had a structured branch for
`map.get(key).cloned()` / `.copied()`, but its helper matched the receiver
directly rather than through `RustExpr::unshaped()`. That meant tagging the
`get` projection, which is the direction this audit recommends, could make the
same structured expression fall back to "not an optional carrier".

The helper now unwraps shape tags before checking for the terminal `get` method.
The rendered `.get(...).cloned()` parser remains only for legacy raw fragments.

## Update 2026-08-11 (Codex follow-up) — optional string-view wrapping asks structure first

`normalize_optional_constructed_element_expr` rendered an expression just to
strip `&...as_str()` before wrapping it in `Some(...)`. The string-view producer
already records `RustExprShape::StringView`, and the adjacent
`normalize_optional_constructed_string_view_element` path already knew how to
use that tag.

The element normalizer now recognizes a structured shared borrow of a string
view before rendering. The raw `&value.as_str()` compatibility fallback remains
for legacy fragments.

## Update 2026-08-11 (Codex follow-up) — optional element mapping classifies built-ins structurally

`render_optional_element_projection` rendered the synthetic mapped element and
switched on exact strings such as `&value`, `value.as_str()`,
`value.as_slice()`, and `value.as_mut_slice()` to choose the short optional
projection form.

It now classifies those built-in projections from `RustExpr` structure:
identity, shared borrow, string view, shared slice view, and mutable slice view.
After the view producers below started preserving method-call structure for
ordinary receivers, the rendered exact-string switch was deleted. Opaque tagged
adapters still render their helper body for the existing closure projection.

## Update 2026-08-11 (Codex follow-up) — view producers keep method-call structure for structured receivers

The optional element classifier above only helps if producers preserve structure.
`Coercion::AsStr`, `Coercion::AsSlice`, and `Coercion::AsMutSlice` still rendered
ordinary structured receivers into raw `value.as_*()` text.

Those producers now emit `RustExpr::MethodCall` when the receiver is already
structured, while retaining the existing raw rendering path for raw receivers
and the special deref/associated-path cases. The optional mapping regression now
feeds the classifier through those real producers.

## Update 2026-08-11 (Codex follow-up) — direct clone projection keeps structure for structured receivers

`apply_clone_projection` tagged its result as `OwnedRecovery`, but still rendered
even `value.clone()` as raw text. That meant consumers could see the recovery
tag, but expression-level rewrites still lost the method-call receiver.

The helper now emits a structured zero-argument `clone` method call for
structured receivers and keeps the existing raw renderer only for raw receivers,
where cast and prefix parenthesisation still need the compatibility path.

## Update 2026-08-11 (Codex follow-up) — optional slice projections keep method-call structure

The optional element mapping path had been upgraded to classify built-in slice
adapters structurally, but it still returned raw `option.as_deref()` and
`option.as_deref_mut()` text after classification.

Those built-in optional slice projections now preserve `RustExpr::MethodCall`
structure while recording the same `OptionalProjected` target slot shape. Raw
closure projections remain raw where the adapter body is still rendered text.

## Update 2026-08-11 (Codex follow-up) — simple method projections keep structure

Several JDK projection helpers still rendered straightforward method chains such
as `value.len() as i32`, `values.iter().cloned()`, and `value.is_nan()` into
opaque raw text even when their receivers were structured.

The length, matrix-row-count, iterator cloned/copied, iterator `next().expect`,
and primitive `is_nan` producers now return structured `RustExpr::MethodCall`
and `RustExpr::Cast` nodes for structured receivers. Raw receiver inputs keep
the old raw renderer, preserving existing spelling for complex legacy fragments.

## Update 2026-08-11 (Codex follow-up) — Java iterator construction composes structured calls

`java_iterator_from_collection_projection` still formatted the whole
`JavaIterator::new(values.iter().cloned())` expression as raw text after the
inner iterator projection had gained structure.

It now builds a structured `RustExpr::Call` to `JavaIterator::new` and reuses
`sequence_iter_cloned_projection` for the argument. The emitted Rust is
unchanged, but the call and iterator chain are visible to structural consumers.

## Update 2026-08-11 (Codex follow-up) — collection get projections keep method structure

The native sequence/map access helpers still rendered `get`, `get_mut`, and
their trailing `expect(...)` calls as raw text.

Those helpers now produce structured `RustExpr::MethodCall` nodes for structured
receivers, including the nested `expect` call. Raw receivers retain the old raw
path so legacy fragments keep their existing spelling and parenthesisation.

## Update 2026-08-11 (Codex follow-up) — sort and fill projections keep method structure

`collection_sort_projection` and `array_fill_projection` were still plain
method-call producers that rendered `values.sort()` and `values.fill(value)` as
raw strings.

They now reuse the structured method-call helper for structured receivers, with
the same raw fallback for raw receivers. The emitted Rust is unchanged.

## Update 2026-08-11 (Codex follow-up) — optional unwrap helpers keep method structure

`unwrap_optional_shared_value` and `unwrap_java_null_receiver_projection` still
rendered `as_ref().unwrap()` and `expect("Java null receiver")` chains as raw
text.

Both helpers now compose structured method calls for structured receivers. Raw
receivers retain the existing rendered fallback through the shared method-call
helpers.

## Update 2026-08-11 (Codex follow-up) — owned optional move unwrap keeps method structure

The borrowed-element slot adapter had one direct move-unwrap arm that still
formatted `value.expect("Java null receiver")` as raw text. That branch is used
when an owned optional carrier is being moved into an owned non-optional target,
so it does not need the borrowed-element shape wrapper, but it can still preserve
the expression tree.

It now reuses the structured `expect` projection for structured receivers and
keeps the existing raw fallback for raw receiver fragments. The regression
`optional_owned_element_move_unwrap_preserves_expr_structure` pins both the
emitted spelling and the `RustExpr::MethodCall` shape.

## Update 2026-08-11 (Codex follow-up) — collection snapshot chains keep method structure

The map and collection snapshot helpers recorded `OwnedRecovery`, but still
rendered ordinary chains like `values.keys().cloned().collect::<Vec<_>>()` as a
single raw expression.

The keys/values/iter + cloned + typed collect paths now compose structured
method calls for structured receivers, while preserving the raw fallback through
the shared method-call helper. The erased-object snapshot branch remains raw
because its `map(|value| ...)` closure has no `RustExpr` representation yet.

## Update 2026-08-11 (Codex follow-up) — erased-object boxing preserves call and cast structure

`erased_object_box_projection` rendered `Box::new(value) as Box<dyn Any>` as a
single raw expression even when `value` was structured.

Structured inputs now lower to a `RustExpr::Call` for `Box::new` wrapped in a
`RustExpr::Cast`, preserving the same emitted Rust. Raw inputs keep the old
rendered path because it includes the compatibility guard that parenthesizes raw
top-level comma expressions before boxing them.

## Update 2026-08-11 (Codex follow-up) — optional receiver projections keep method structure

`project_optional_receiver` still rendered all optional receiver recovery forms
as raw text: `.expect("Java null receiver")`, `.as_ref().expect(...)`,
`.as_mut().expect(...)`, and the unchecked `.as_ref().unwrap()` /
`.as_mut().unwrap()` variants.

Structured receiver inputs now compose those method chains through the shared
method-call helpers. Raw receiver fragments keep the previous renderer,
including the compatibility behavior that strips a terminal `.unwrap()` before
adding the requested receiver projection.

## Update 2026-08-11 (Codex follow-up) — borrowed facade return recovery keeps method structure

`apply_rust_borrowed_facade_return` still rendered the non-optional borrowed
return recovery step (`.clone()` / `.to_path_buf()`) as raw text before boxing it
as erased `Object`.

The non-optional path now composes the recovery method call structurally and then
feeds it to the structured erased-object boxing helper. The optional path stays
raw for now because it is a `map(|value| ...)` closure and the current IR has no
closure expression node.

## Update 2026-08-11 (Codex follow-up) — direct clone idempotence asks structure first

`lower.rs` still rendered an expression to decide the narrow direct-clone
idempotence case (`.clone()` / `.clone_box()`), even though the clone producers
now preserve `RustExpr::MethodCall` structure and record `OwnedRecovery`.

`rust_expr_has_direct_clone_recovery` now recognises zero-argument structured
`clone` and `clone_box` method calls before falling back to the legacy rendered
predicate. This is intentionally narrower than broad `OwnedRecovery`: sequence
recovery such as `to_vec()` is owned recovery, but it is not a direct clone and
must not satisfy that idempotence arm. `clone_recovery_idempotence_matches` now
uses the structural predicate for `DirectClone`; the rendered fallback remains
only for legacy raw producers.

## Update 2026-08-11 (Codex follow-up) — map-key borrow idempotence asks structure for built-ins

The map-key borrow adapter already accepted a recorded `StringView` tag, but
structured built-ins such as `name.as_str()` and string literals still flowed
through `emit::emit_expr` and the rendered `.as_str()` / literal fallback.

`rust_expr_is_already_borrowed_map_key` now answers those built-ins from
`RustExpr` structure first: recorded `StringView`, complete string literals, and
zero-argument `as_str` method calls. The rendered predicate remains for legacy
raw helpers such as `java_string_*`, while structured non-string-view calls still
get borrowed.

## Update 2026-08-11 (Codex follow-up) — proven sequence recovery keeps `.to_vec()` structure

`to_owned_sequence_projection` still flattened structured receivers into raw
`value.to_vec()` text even when clone recovery was proven and no audit comment
had to be inserted.

For `ProvenCloneCapability`, structured receivers now produce a structured
zero-argument `to_vec` method call tagged as `OwnedRecovery`; an already
structured `to_vec` call is returned idempotently. The `AuditRequired` path stays
raw because the emitted audit comment is currently part of that expression.

## Update 2026-08-12 (Codex follow-up) — owned optional unwrap keeps method structure

The borrowed optional unwrap arms already recorded shape and had targeted
producer coverage, but the owned `Coercion::UnwrapOption` arm still rendered
`value.unwrap()` as raw text even for structured receivers.

That arm now uses the shared zero-argument method projection helper. Structured
receivers preserve `RustExpr::MethodCall { method: "unwrap" }`; raw receivers
keep the old rendered fallback spelling.

## Update 2026-08-12 (Codex follow-up) — borrowed optional unwrap keeps method structure

The borrowed optional unwrap coercions recorded `Borrowed` / `MutBorrowed`
shape, but the expression inside the shape was still raw rendered text for
`.as_ref().expect("Java null receiver")` and `.as_mut().expect(...)`.

Those arms now compose the existing zero-argument method and `expect` projection
helpers. Structured receivers keep the nested method-call chain; raw receivers
still take the legacy rendered fallback.

## Update 2026-08-12 (Codex follow-up) — copied borrowed values keep unary deref structure

`Coercion::CopyBorrowedValue` still rendered `*value` as raw text even when the
borrowed value was a structured expression.

The coercion now goes through `deref_borrowed_projection`, which emits a
structured unary `*` for structured inputs and keeps the old raw rendering for
legacy raw fragments. The copy-recovery policy test now pins both the IR shape
and the emitted Rust spelling.

## Update 2026-08-12 (Codex follow-up) — optional element carrier borrows keep method structure

`project_optional_element_borrow_carrier` and the shape-aware sibling recorded
`OptionalProjected`, but still rendered `.as_ref()` / `.as_mut()` as raw text.

Both functions now use the shared zero-argument method projection helper.
Structured option receivers preserve `RustExpr::MethodCall` structure while raw
receivers keep the old rendered fallback spelling. The carrier projection test
now pins the shared and mutable method-call shapes.

## Update 2026-08-12 (Codex follow-up) — optional mutable reborrow keeps method structure

`reborrow_optional_mut_projection` recorded the produced optional element slot
shape, but new projections still appended `.as_deref_mut()` as raw text.

The producer now uses the shared zero-argument method projection helper.
Structured receivers keep a `RustExpr::MethodCall`; raw receivers keep the
legacy rendered fallback and the existing suffix compatibility path.

## Update 2026-08-12 (Codex follow-up) — forced shared borrows keep unary structure

`force_shared_borrow_projection` already recorded `Borrowed`, but still flattened
structured inputs into raw `&value` text.

The helper now emits a structured unary `&` for structured receivers and keeps
the raw fallback for opaque fragments. The borrow-producer regression now covers
both the structured identifier path and the legacy raw helper path.

## Update 2026-08-12 (Codex follow-up) — string helper receivers keep borrow structure

`string_helper_receiver_projection` already recorded `StringView`, but ordinary
structured receivers still became raw `&value` text before entering string
runtime helpers.

The helper now reuses the structured forced-borrow projection for structured
receivers, then re-tags the result as `StringView`. Optional unwrap receivers and
opaque raw fragments keep their existing rendered compatibility paths.

## Update 2026-08-12 (Codex follow-up) — reused-clone local views keep structure

`normalize_reused_clone_for_borrowed_argument` had been upgraded to record the
produced view or borrow shape, but its local-name branches still built raw
`name.as_str()`, `name.as_slice()`, `name.as_mut_slice()`, and `&name` text.

Those branches now build the same local projections through structured
`RustExpr::Var`, method-call, and unary-borrow helpers. The reuse regression now
pins the emitted Rust and the IR shape for all four local-name outputs.

## Update 2026-08-12 (Codex follow-up) — borrowed-element unwrap tail preserves structure

The owned optional move-unwrap arm was already documented as structured, but the
shared borrowed-element adapter still rebuilt the final
`.expect("Java null receiver")` tail as raw text after element recovery.

That unwrap tail now uses `method_expect_projection`, preserving structured
`copied()` / `cloned()` receivers while retaining the `OwnedRecovery` tag when
the recovered element had one. Raw receivers keep the legacy rendered fallback.

## Update 2026-08-12 (Codex follow-up) — shape-aware optional receivers keep structure

`project_optional_receiver_with_shape` still rebuilt the optional receiver
projection as raw text before attaching the produced element slot shape.

It now delegates the projection itself to `project_optional_receiver`, then
records the produced `OptionalProjected` slot shape around that expression.
Structured receivers therefore keep the nested method-call chain while raw
receivers retain the same compatibility fallback.

## Update 2026-08-12 (Codex follow-up) — required borrowed-element views keep structure

`apply_required_borrowed_element_target_view` recorded the produced target slot
shape, but still rendered required `.as_slice()`, `.as_mut_slice()`, and
`.as_str()` tails as raw text.

Those tails now use the shared zero-argument method projection helper for
structured receivers and keep the raw fallback for raw receiver fragments. The
required optional-array-to-slice regression now pins both the old raw spelling
and the structured nested method-call path.

## Update 2026-08-12 (Codex follow-up) — optional borrowed-element method views keep structure

`apply_optional_borrowed_element_target_view` already recorded the produced
optional element slot shape, but four simple method-tail arms still rendered
their adapters as raw text: `.as_deref()`, `.as_deref_mut()`, `.as_ref()`, and
`.as_mut()`.

Those arms now reuse the shared zero-argument method projection helper for
structured receivers and keep the same raw fallback for opaque receiver
fragments. The optional string-view arm remains raw because it is a
`map(|value| value.as_str())` closure projection, which the current expression IR
does not represent structurally yet.

## Update 2026-08-12 (Codex follow-up) — primitive parse projections keep method structure

`primitive_parse_projection` rendered `value.parse::<T>().unwrap()` as raw text
even for structured receivers, despite the expression being a simple pair of
zero-argument method calls.

Structured receivers now lower to nested `RustExpr::MethodCall` nodes. A
structured shared-borrow receiver is stripped before parsing, matching the old
raw renderer's `&value` compatibility behavior. Raw fragments still use
`render_parse_receiver`, preserving the existing parenthesization and
borrow-stripping fallbacks.

## Update 2026-08-12 (Codex follow-up) — optional string-helper unwrap keeps structure

`string_helper_receiver_projection` still recognized optional string receivers
by checking whether the rendered text ended in `.as_ref().unwrap()`, then
returned raw `{receiver}.as_str()` text.

The helper now recognizes the structured `as_ref().unwrap()` method-call chain
before rendering and appends `.as_str()` as a structured method call. Raw
receiver fragments keep the existing suffix compatibility path.

## Update 2026-08-12 (Codex follow-up) — tuple field projections keep field structure

`tuple_field_projection` rendered `entry.1` as raw text even though `RustExpr`
already has a field-access node and the emitter preserves numeric field names.

The helper now returns `RustExpr::Field(_, "1")` directly. The emitted Rust is
unchanged, while later expression rewrites can see the receiver and field
boundary instead of reparsing the rendered tuple access.

## Update 2026-08-12 (Codex follow-up) — primitive number value casts keep structure

`primitive_number_value_projection` rendered boxed primitive value extraction as
raw `value as f64` text even when the receiver was structured.

Structured receivers now lower to `RustExpr::Cast`, preserving the emitted Rust
for normal operands and exposing the cast boundary to later passes. Raw receiver
fragments keep the previous renderer, including its historical precedence
spelling for opaque text.

## Update 2026-08-12 (Codex follow-up) — primitive f64 method casts keep structure

`primitive_f64_method_projection` similarly rendered `(value as f64).ln()` as a
single raw string for structured operands.

Structured operands now produce a `RustExpr::Cast` receiver followed by the
requested zero-argument method call. Raw operands keep the legacy renderer, so
existing opaque fragments retain their exact spelling and precedence behavior.

## Update 2026-08-12 (Codex follow-up) — string char-array projections keep method structure

`string_to_char_array_projection` rendered `value.chars().collect::<Vec<char>>()`
as raw text even when `value` was a structured receiver.

Structured receivers now compose the `chars` call with the typed `collect` call
as `RustExpr::MethodCall` nodes. Raw receivers keep the old renderer. The nearby
`getBytes` projection remains raw because its `map(|byte| *byte as i8)` closure
still has no expression-IR representation.

## Update 2026-08-12 (Codex follow-up) — simple runtime calls keep call structure

`system_exit_projection` and `thread_sleep_millis_projection` rendered their
runtime handoffs as raw text even though both are ordinary function-call trees.

They now emit `RustExpr::Call` nodes for `std::process::exit`,
`std::thread::sleep`, and the nested `Duration::from_millis` call. The numeric
arguments remain explicit casts, preserving the existing emitted Rust, including
the legacy raw-fragment spelling for opaque sleep operands.

## Update 2026-08-12 (Codex follow-up) — float min/max helpers keep call structure

The integer `Math.max`/`Math.min` projections already built structured calls to
`std::cmp`, but the float and double arms still rendered
`java_math_max_f32(...)`, `java_math_max_f64(...)`, `java_math_min_f32(...)`, and
`java_math_min_f64(...)` as raw strings.

Those helper arms now use `RustExpr::Call` too. The emitted Rust is unchanged;
the distinction between helper function and arguments is no longer hidden inside
one rendered string.

## Update 2026-08-12 (Codex follow-up) — array string helpers keep call structure

`arrays_to_string_projection` and `arrays_deep_to_string_projection` were the
remaining one-argument array helper wrappers in this cluster that still rendered
their calls as raw text.

Both now emit `RustExpr::Call` nodes, matching the neighbouring array equality,
comparison, mismatch, and hash-code helper projections. The generated Rust is
unchanged.

## Update 2026-08-12 (Codex follow-up) — native collection constructors keep call structure

`native_collection_empty_new_projection` already built a structured no-argument
constructor call, but the sibling `with_capacity` and default-constructor helpers
still rendered `Type::with_capacity(...)` and `Type::new()` as raw text.

Both now emit `RustExpr::Call` nodes. `with_capacity` keeps the explicit `usize`
cast as a structured `RustExpr::Cast` for ordinary operands. An already-cast
capacity keeps the legacy rendered nested-cast spelling, because generated
fixtures currently pin `x as i32 as usize` rather than `(x as i32) as usize`.

## Update 2026-08-12 (Codex follow-up) — empty collection helpers keep call structure

The `Collections.empty*` helpers still returned raw `Type::new()` strings for
`Vec`, `HashSet`, `HashMap`, and `BTreeSet`, even though the native collection
constructor path now preserves these calls structurally.

Those empty helpers now emit `RustExpr::Call` nodes as well. The generated Rust
is unchanged.

## Update 2026-08-12 (Codex follow-up) — optional copied expect keeps method structure

`optional_copied_expect_projection` already recorded `OwnedRecovery`, but still
rendered the `.copied().expect("Java null receiver")` chain as one raw string.

It now composes the existing zero-argument method and expect helpers. Structured
receivers keep the nested `RustExpr::MethodCall` chain while raw receivers retain
the same emitted compatibility spelling.

## Update 2026-08-12 (Codex follow-up) — set source construction and empty varargs keep structure

Two more collection edges in the same projection cluster still produced opaque
strings for ordinary call shapes.

`native_collection_from_source_projection` now keeps
`values.into_iter().collect::<HashSet<_>>()` as nested method calls when the
source expression is structured, while raw sources preserve the exact legacy
rendering. A later follow-up removed that raw-source fallback too: raw sources
now also flow through nested method-call IR and only the raw receiver leaf stays
opaque. Empty varargs now use a structured `Vec::new()` call; non-empty varargs
now use `RustExpr::VecLiteral`.

## Update 2026-08-12 (Codex follow-up) — no-arg native defaults keep call structure

Two more no-argument constructors were still emitted as opaque strings:
`String::new()` from empty `String.format` and `String::new()` / `Vec::new()`
from slot default rendering.

Those paths now emit `RustExpr::Call` nodes. The generated Rust is unchanged,
but later passes can see the call boundary instead of rediscovering it from a
rendered string.

## Update 2026-08-12 (Codex follow-up) — exception message stringification keeps structure

Exception construction still stringified structured message expressions by
rendering them and appending `.to_string()` as raw text. The no-argument
exception constructor also built its empty message as raw `String::new()`.

`empty_exception_message` now emits a structured `String::new()` call, and
`exception_message_expr` preserves a structured `.to_string()` method call for
non-raw messages. Raw message fragments keep the legacy renderer. The emitted
exception construction remains byte-identical.

## Update 2026-08-12 (Codex follow-up) — logical-not projection keeps unary structure

`logical_not_projection` rendered every negation as raw `!(...)` text even when
the operand was already structured.

Structured operands now emit a `RustExpr::Unary` node, letting the emitter own
the required parentheses for compound children. Raw operands retain the legacy
`!(...)` compatibility spelling.

## Update 2026-08-12 (Codex follow-up) — scalar narrowing keeps method structure

Rust scalar slot narrowing still rendered `value.try_into().unwrap()` as one raw
string for structured operands.

Structured operands now keep the `try_into` and `unwrap` calls as nested
`RustExpr::MethodCall` nodes. Raw operands retain the previous compatibility
spelling.

## Update 2026-08-12 (Codex follow-up) — borrowed subtype constructors keep call structure

`subtype_owned_constructor_from_borrowed_projection` still rendered
`Target::new_1(value.clone())` as one raw string.

Structured borrowed values now produce a normal `RustExpr::Call` to
`Target::new_1` with a structured `.clone()` argument. Raw borrowed fragments
retain the previous compatibility spelling.

## Update 2026-08-12 (Codex follow-up) — binary f64 math projections keep method structure

The one-argument f64 math projection already kept `(value as f64).ln()` as a
structured cast receiver plus method call, but the two-argument sibling and
`powf` still rendered the whole expression as raw text.

When both operands are structured, `primitive_f64_binary_method_projection` and
`math_pow_projection` now emit a structured cast receiver and structured cast
argument. Any raw operand keeps the legacy renderer.

## Update 2026-08-12 (Codex follow-up) — math abs keeps method structure

`math_abs_projection` rendered both `value.wrapping_abs()` and `value.abs()` as
raw text even for structured operands.

Structured operands now produce a zero-argument `RustExpr::MethodCall` to
`wrapping_abs` or `abs` according to the source primitive. Raw operands retain
the previous compatibility spelling.

## Update 2026-08-12 (Codex follow-up) — floorDiv/floorMod helper calls keep structure

`math_floor_div_projection` and `math_floor_mod_projection` rendered the helper
call and its primitive argument casts as a single raw string.

When both operands are structured, the helpers now emit `RustExpr::Call` with
structured `RustExpr::Cast` arguments. Raw operands keep the legacy rendered
fallback. The `floorMod(long,int)` return narrowing still preserves the old
outer raw cast spelling while using the structured helper call internally.

## Update 2026-08-12 (Codex follow-up) — Math.round keeps helper call structure

`math_round_projection` rendered `java_math_round_f32(...)`,
`java_math_round_f64(...)`, and the non-float `as f64` adaptation as raw text.

Structured operands now produce a `RustExpr::Call`, with a structured
`RustExpr::Cast` argument for non-float sources. Raw operands retain the
previous compatibility spelling.

## Update 2026-08-12 (Codex follow-up) — primitive compare helper keeps call structure

`primitive_compare_to_projection` rendered
`java_primitive_compare_to(left, right)` as raw text even when both operands
were structured.

Structured operands now produce a normal `RustExpr::Call` to the helper. If
either operand is raw, the previous rendered compatibility path remains.

## Update 2026-08-12 (Codex follow-up) — simple primitive hash-code arms keep structure

`primitive_hash_code_projection` still rendered every primitive hash-code shape
as raw text.

The simple structured cases now keep their expression shape: boolean hash codes
emit `RustExpr::If`, integral hash codes emit `RustExpr::Cast`, and float hash
codes emit a structured `to_bits()` method call followed by a cast. Raw receivers
and the long/double bit-mixing arms keep the previous rendered compatibility
path.

## Update 2026-08-12 (Codex follow-up) — long/double hash-code arms keep structure

The remaining long and double arms in `primitive_hash_code_projection` mixed
casts, shifts, xor, and `to_bits()` calls inside raw strings.

Structured receivers now build those hash-code expressions as `RustExpr::Cast`,
`RustExpr::Binary`, and `RustExpr::MethodCall` nodes. Raw receivers keep the
legacy rendered spelling.

## Update 2026-08-12 (Codex follow-up) — string compare helpers keep receiver structure

The shared string compare/equals-ignore-case helper built its first argument by
rendering `render_string_view_argument(receiver)` and wrapping that string in
`RustExpr::Raw`.

It now reuses `string_helper_receiver_projection`, so ordinary receivers keep the
structured string-view borrow (`RustExpr::Unary` tagged as `StringView`). The
second argument remains the already-planned expression supplied by the caller.

## Update 2026-08-12 (Codex follow-up) — string search and substring helpers keep argument structure

`string_index_of_projection`, `string_last_index_of_projection`, and
`string_substring_projection` built structured helper calls but rendered their
string-view arguments into `RustExpr::Raw` first.

They now share `string_view_argument_projection`, the expression form of the
existing string-view borrow renderer. The helper calls preserve structured
borrow arguments while keeping the same emitted Rust and the existing
char-literal needle normalization.

## Update 2026-08-12 (Codex follow-up) — String.getBytes keeps the method chain structure

`string_get_bytes_projection` rendered the whole
`.as_bytes().iter().map(...).collect::<Vec<i8>>()` chain as one raw expression.

Structured receivers now produce nested method-call nodes for `as_bytes`,
`iter`, `map`, and `collect`, while preserving the raw closure argument and the
exact emitted Rust. Raw receivers keep the previous compatibility fallback.

## Update 2026-08-12 (Codex follow-up) — String.charAt keeps its receiver chain structure

`string_char_at_projection` rendered the whole
`.chars().nth(...).expect(...)` expression as raw text.

Structured receivers now keep the `chars`, `nth`, and `expect` calls as
`RustExpr::MethodCall` nodes. The index-to-`usize` adapter still uses the
existing rendered helper because that helper owns compatibility parentheses and
the stream/string index distinction. Raw receivers keep the previous fallback.

## Update 2026-08-12 (Codex follow-up) — List.indexOf keeps its iterator chain structure

`sequence_index_of_projection` rendered the whole
`.iter().position(...).map(...).unwrap_or(-1)` chain as raw text.

Structured receivers now keep the `iter`, `position`, `map`, and `unwrap_or`
calls as `RustExpr::MethodCall` nodes. The closure bodies stay raw for now,
because they still splice in the rendered search value. Raw receivers keep the
previous fallback.

## Update 2026-08-12 (Codex follow-up) — File.exists keeps its path call structure

`file_exists_projection` rendered
`std::path::Path::new(file.get_path().as_str()).exists()` as a single raw
expression.

Structured receivers now keep `get_path`, `as_str`, `Path::new`, and `exists`
as expression nodes while preserving the emitted Rust. Raw receivers keep the
previous fallback.

## Update 2026-08-12 (Codex follow-up) — char sequence string collection keeps structure

`char_sequence_collect_string_projection` rendered
`.iter().collect::<String>()` as one raw expression.

Structured inputs now keep the `iter` and `collect::<String>` calls as
`RustExpr::MethodCall` nodes while preserving emitted Rust. Raw inputs keep the
previous fallback.

## Update 2026-08-12 (Codex follow-up) — optional owned recovery keeps its chain structure

`optional_get_owned_value_projection` and
`optional_or_else_owned_value_projection` already recorded `OwnedRecovery`, but
their inner expressions were still one raw string containing the full
`.as_ref().map(...).expect(...)` or `.unwrap_or(...)` chain.

Structured receivers now keep the optional receiver chain as method-call nodes
while preserving the raw clone closure and the emitted Rust. Raw receivers keep
the previous fallback.

## Update 2026-08-12 (Codex follow-up) — nullable display plans keep method-chain structure

The nullable Java display adapters in `apply_java_display_plan` rendered their
null policy as single raw strings:

- `optional.as_deref().unwrap_or("null")`;
- `optional.as_ref().map(...).unwrap_or_else(...)`;
- `borrowed_optional.map(...).unwrap_or_else(...)`.

Structured inputs now keep the receiver chains as `RustExpr::MethodCall` nodes
while preserving the raw closures and emitted Rust. Raw receivers keep the old
fallback.

## Update 2026-08-12 (Codex follow-up) — borrowed-subtype constructor calls stay structured

`subtype_owned_constructor_from_borrowed_projection` already built a structured
`Target::new_1(value.clone())` call for structured borrowed values, but fell
back to one raw string when the borrowed expression was raw.

The projection now always keeps the outer constructor call as `RustExpr::Call`.
Raw borrowed expressions remain raw only inside the cloned argument, preserving
the emitted Rust while keeping the call boundary inspectable.

## Update 2026-08-12 (Codex follow-up) — mutable-alias detection asks the IR first

`rendered_expr_is_mutable_alias` still accepted a `RustExpr`, but immediately
rendered it and classified alias shapes with exact strings and suffixes:
`&mut base`, `base.as_mut_slice()`, `.as_mut().expect(...)`, and
`.as_deref_mut()`.

The query now checks structured `RustExpr::Unary` and `RustExpr::MethodCall`
forms first, including field/index chains rooted at the aliased base. The
rendered classifier remains as the raw compatibility fallback.

## Update 2026-08-12 (Codex follow-up) — optional string-view projection keeps its receiver chain

`optional_string_view_projection` still recorded `OptionalProjected`, but the
inner value was one raw string for
`.as_ref().map(|value| value.as_str())`.

Structured receivers now keep the `as_ref` and `map` calls as `RustExpr` nodes
while preserving the raw closure and emitted Rust. Raw receivers keep the old
fallback.

## Update 2026-08-12 (Codex follow-up) — string-view detection handles structured projections before text

`expr_is_string_view` previously asked the recorded tag first, then rendered any
untagged expression and checked for `.as_str()` or a complete string literal.

The query now recognises structured `.as_str()` method calls and literal string
tokens directly. Raw expressions still go through the old rendered compatibility
path.

## Update 2026-08-12 (Codex follow-up) — clone-recovery detection handles structured clone calls before text

`rust_expr_has_clone_recovery` previously asked the recorded `OwnedRecovery`
tag first, then rendered every untagged expression to recognise `.clone()`,
`.clone_box()`, `.cloned()`, and
`.cloned().expect("Java null receiver")`.

The query now recognises those `RustExpr::MethodCall` shapes directly. The
rendered predicate remains only as the compatibility path for raw expressions.

## Update 2026-08-12 (Codex follow-up) — index-to-usize coercion keeps cast structure

`Coercion::IndexToUsize` still rendered every index adapter as raw
`{expr} as usize` text, even when the input was a normal `RustExpr::Var`,
`Binary`, or other structured expression.

Structured inputs now lower to `RustExpr::Cast { ty: "usize" }`. Raw fragments
retain the old rendered fallback and its parenthesization rules, so opaque
generated text keeps the previous spelling while ordinary index casts expose
their cast boundary to later passes.

## Update 2026-08-12 (Codex follow-up) — floorMod return narrowing keeps cast structure

`math_floor_mod_projection` already used a structured helper call for structured
operands, but the `long`-operation/`int`-return case immediately rendered that
call back into `({call}) as i32` raw text.

That narrowing now stays as `RustExpr::Cast { ty: "i32" }` when the helper call
is structured. If either helper operand is raw, the old rendered compatibility
spelling remains.

## Update 2026-08-12 (Codex follow-up) — collection capacity nested casts stay structured

The first native collection constructor cleanup kept one raw compatibility
exception: if `with_capacity` already received a cast expression, it rendered
`capacity as i32 as usize` as raw text to preserve the old spelling.

That exception is gone. Capacity arguments now always pass through a
`RustExpr::Cast { ty: "usize" }`, including nested casts. The emitter writes the
nested case as `((capacity as i32) as usize)` inside the call argument, which is
valid Rust and keeps both cast boundaries inspectable.

## Update 2026-08-12 (Codex follow-up) — String.format argument stringification keeps structure

`render_string_format_arg_value` rendered each dynamic `String.format` argument
as `{arg}.to_string()` text before the arguments were packed into the raw
`vec![...]` macro expression.

Structured arguments now pass through `string_format_arg_value_projection`,
which emits a normal zero-argument `.to_string()` method call. Raw fragments
keep the old rendered spelling. A later vec-macro pass moved the runtime
argument packing itself to `RustExpr::VecLiteral`.

## Update 2026-08-12 (Codex follow-up) — fallback exception messages keep value structure

`fallback_exception_expr` still rendered the debug-message handoff as one raw
`format!("{:?}", value)` string even when the thrown value was a structured
expression.

Structured values now build a `RustExpr::Call` to `format!` with the original
value preserved as an argument. Raw values retain the previous rendered fallback
so opaque fragments keep the same emitted Rust.

## Update 2026-08-12 (Codex follow-up) — primitive char casts keep operand structure

`apply_cast_conversion` still rendered Java primitive casts involving `char` as
raw text because `int -> char` needs the Java `0xffff` mask and `char -> int`
needs a two-step `u32` cast.

Structured operands now keep those conversions as expression nodes: `int -> char`
builds the `char::from_u32(...).unwrap_or('\0')` call with a structured masked
argument, and `char -> numeric` builds nested `RustExpr::Cast` nodes. Raw
operands retain the previous rendered compatibility spelling.

## Update 2026-08-12 (Codex follow-up) — slot default trait construction is structured

The central `SlotDefaultValuePolicy::DefaultTrait` renderer still returned raw
`Default::default()` text. That made every slot default using the trait fallback
opaque before later wrapping, including shared immutable defaults such as
`Rc::new(Default::default())`.

The default-trait arm now emits a zero-argument `RustExpr::Call` to
`Default::default`. Existing emitted Rust is unchanged, while wrappers and later
passes can inspect the default construction boundary.

## Update 2026-08-12 (Codex follow-up) — call-lowering default fallbacks are structured

`call_lowering` still had two local raw `Default::default()` producers: the
reference-return super-boundary fallback and the missing-argument fallback for
`Collections.unmodifiable*`.

Both now use a local structured default-trait call helper. The emitted Rust is
unchanged, but call-target lowering no longer hides those fallback values behind
opaque text.

## Update 2026-08-12 (Codex follow-up) — lower.rs default fallback expressions are structured

Two lowerer expression paths still constructed raw `Default::default()` values:
preinitialized locals in try/catch lowering, and anonymous external constructor
bodies flowing into concrete native slots.

Both now use a local zero-argument `RustExpr::Call` to `Default::default`.
Generated Rust remains the same, while the fallback value stays inspectable
until it reaches the emitter.

## Update 2026-08-12 (Codex follow-up) — Array2 row handoffs keep call structure

`array2_from_rows_projection` rendered the full
`ndarray::Array2::from_rows(...)` handoff as raw text, including the borrowed
argument form used when a static two-dimensional Java array feeds an `Array2`
slot.

Structured row expressions now build a normal `RustExpr::Call` to
`ndarray::Array2::from_rows`; borrowed slots wrap that call in a structured
unary `&`. Raw row fragments retain the old compatibility spelling.

## Update 2026-08-12 (Codex follow-up) — Array2 indexing keeps the array expression structure

`array2_index_projection` rendered the whole `values[[row, col]]` expression as
raw text, even when `values` was already a structured expression.

Structured array expressions now lower to `RustExpr::Index`, keeping the array
side inspectable while using a raw two-coordinate index payload because the IR
does not yet have a tuple/list index node. Raw array fragments keep the previous
whole-expression fallback.

## Update 2026-08-12 (Codex follow-up) — tuple IR unblocks structured Array2 allocation calls

`array2_from_elem_projection` rendered
`ndarray::Array2::from_elem((rows as usize, cols as usize), default)` as one raw
string. The blocker was not the outer call but the shape argument: a tuple had to
be expressed as raw text, and raw expression normalization strips outer
parentheses at expression boundaries.

`RustExpr::Tuple` now owns tuple spelling, including the single-element trailing
comma case. `array2_from_elem_projection` uses it for structured row/column
dimensions and emits a normal `RustExpr::Call` to `ndarray::Array2::from_elem`.
Raw dimensions retain the previous whole-expression fallback.

## Update 2026-08-12 (Codex follow-up) — Array2 row mutation keeps method-call structure

`array2_row_mut_assign_stmt` rendered
`values.row_mut(index).assign(&ndarray::ArrayView1::from(value));` as one raw
statement.

When the array, row index, and replacement value are structured expressions, the
projection now emits a structured expression statement: nested `row_mut` and
`assign` method calls around a structured borrow of
`ndarray::ArrayView1::from(value)`. Any raw operand keeps the previous raw
statement fallback.

## Update 2026-08-12 (Codex follow-up) — Vec macro allocation has IR nodes

One-dimensional Java array allocation and non-primitive two-dimensional array
allocation still used raw `vec![...]` strings, as did array initializer lists
after comment-only elements were filtered.

`RustExpr::VecLiteral` and `RustExpr::VecRepeat` now own the two `vec!` macro
forms. The array lowering projections emit those structured nodes for list
initializers, `new T[n]`, `new T[rows][cols]` object-array carriers, and ragged
outer-row allocation. Emitted Rust is unchanged; only intentionally opaque
operands, such as default element expressions, remain raw leaves.

The change also added the new nodes to the existing expression visitors. The
constructor `self` rewrite was the useful guardrail: once `vec![0; self.width as
usize]` was no longer raw text, the visitor had to recurse through the repeat
length or super-constructor field-read initialization regressed.

Static storage array initialization now uses the same nodes. Constant folded
array literals build `RustExpr::VecLiteral`, and default-filled `new T[N]`
initializers build `RustExpr::VecRepeat` with a structured `as usize` length
cast. That removes two more raw `vec!` assembly sites without changing emitted
Rust.

The same pass covered the simple collection and varargs producers that build
plain vectors: `Arrays.asList`, `Collections.singletonList`,
`Collections.nCopies`, packed varargs, and runtime `String.format` argument
packing now use `VecLiteral` or `VecRepeat` instead of re-rendering their
operands into `vec![...]` strings.

Rust array literals now have a sibling `RustExpr::ArrayLiteral` node. That lets
`Collections.singleton` and `Collections.singletonMap` keep the outer
`HashSet::from`/`HashMap::from` call structure and preserve the `[value]` /
`[(key, value)]` argument as inspectable IR instead of flattening the whole
projection into raw text.

Macro calls that fit ordinary call shape now use it too. `print_macro_projection`
renders `println!`/`print!` through `RustExpr::Call` with a raw macro callee, and
the constant-format arm of `string_format_projection` does the same for
`format!`. The emitted Rust stays byte-identical; the macro argument list no
longer has to be reassembled as one raw string.

Associated paths now have `RustExpr::Path`. The path-producing helpers for
`Self::member`, `Owner::member`, external crate paths, `JavaClass::<T>` members,
and associated constants now return path IR instead of raw strings. Static
storage accessor helpers use a structured call whose callee is a path, so the
`()` call boundary is visible too.

Common standard-library and support-function callees now use the same path IR.
`String::new`, `Vec::new`, `Default::default`, collection constructors,
`JavaException::{new,with_cause}`, `char::from_u32`, and the structured
`ndarray` helpers no longer hide their callee behind `RustExpr::Raw`.

That exposed one remaining consumer that was still matching raw text:
try/catch return rewriting recognised only raw `Ok` and `Err` callees. It now
accepts `RustExpr::Path(["Ok"])` / `RustExpr::Path(["Err"])` as the primary
shape and keeps the `Ok(Some(value))` wrapper structured. Raw single-name
compatibility remains for older opaque producers.

The earlier default-construction updates also had two stale local helpers left
behind in `call_lowering.rs` and `lower.rs`. Both now construct
`Default::default()` through `RustExpr::Path(["Default", "default"])`, and the
production search for raw `Default::default` call callees is empty.

The JDK platform helpers now do the same for standard-library callees:
`std::path::Path::new`, `std::process::exit`, `std::thread::sleep`, and
`std::time::Duration::from_millis` are structured paths inside their existing
`RustExpr::Call` nodes. The raw receiver compatibility fallbacks are unchanged.

The integer `Math.max`/`Math.min` projections also no longer hide
`std::cmp::{max,min}` behind raw callee text. The floating-point Java helper
calls remain helper-name leaves.

Runtime helper callees in `coercion.rs` now use `RustExpr::Path` as well, even
for single-segment helpers such as `java_string_hash_code`,
`java_arrays_equals`, and the floating-point `java_math_*` shims. Emission is
unchanged, but the call target is no longer an opaque raw string.

`apply_rust_borrowed_facade_return` also stopped rendering `Some(...)` by hand
for optional borrowed-facade returns. It now uses the existing
`render_option_some` helper, so the expression records
`RustExprShape::OptionalConstructed` just like the other optional construction
paths.

The raw compatibility arm for `Some(&value.as_str())` normalization now keeps
that same structure. It still accepts the old rendered spelling, but returns a
tagged `Some(value.as_str())` call instead of a new raw string.

`Some` itself is now path-aware in the optional construction helpers and
queries. `render_option_some` emits a `RustExpr::Path(["Some"])` callee, while
the construction/unwrap predicates accept both that shape and the older
`Var("Some")` compatibility shape.

`String.format` locale-argument detection now asks `RustExpr::Var` and
`RustExpr::Path` directly before falling back to rendered text for raw legacy
inputs. A structured `Locale::US` path is recognized without broadening matches
such as `LocaleSuffix::US`.

Borrowed collection clone recovery now recognizes structured
`.cloned().collect::<Vec<_>>()` and
`.cloned().collect::<std::collections::HashSet<_>>()` method chains before the
rendered suffix fallback. That keeps idempotence for structured producers
without forcing them through text just to prove the clone already happened.

`call_lowering`'s super-boundary void fallback no longer returns raw `Ok(())` or
raw `()`. The `Result<(), JavaException>` arm now builds a structured call to
`Ok` with a zero-item `RustExpr::Tuple`, and the non-result void arm uses the
same tuple node directly. Emitted Rust is unchanged; the unit value and result
wrapper are visible to later expression visitors.

The same unit helper now covers the native-call fallback sites in
`call_lowering`: `System.gc`, malformed `Arrays.equals`/`Arrays.deepEquals` /
`Arrays.binarySearch` arguments, and missing singleton/nCopies collection
values. The source-level guard is file-wide: `call_lowering.rs` no longer
contains raw `Ok(())` or raw `()` expression producers.

Constructor and source-summary boundary returns in `lower.rs` now use the same
callee shape for non-void `Result` wrapping: `Ok` is a `RustExpr::Path`, not
`RustExpr::Raw("Ok")`. The 0007 source guard now rejects reintroducing raw `Ok`
call targets in `lower.rs`.

External facade free/associated calls now keep plain Rust facade paths
structured too. `apply_external_facade_receiver_call` converts identifier-only
`crate::Type::method` strings into `RustExpr::Path` before building the call,
with a raw fallback for generic or otherwise non-plain paths. The HDF5 facade
receiver test now asserts the structured callee shape.

The `Serializable` marker `instanceof` predicate also stopped rendering the
whole helper call as raw text. `render_instanceof_check` now builds a structured
call to `java_is_serializable` with a structured borrowed operand, and the
`type_coercion` test pins both the emitted spelling and the IR shape.

The Java iterator facade constructor is path-shaped as well:
`java_iterator_from_collection_projection` now calls `JavaIterator::new` through
`RustExpr::Path(["JavaIterator", "new"])` instead of a single raw-ish variable
name containing `::`. Its existing projection test now asserts that callee
shape.

Subtype-constructor recovery from borrowed values now gets the same treatment.
`subtype_owned_constructor_from_borrowed_projection` builds `Target::new_1` as a
structured path when the owner path is identifier-only, with a raw fallback for
non-plain owner paths. The projection test now asserts the structured callee for
both structured and raw borrowed operands.

Structured erased-object boxing no longer hides `Box::new` as a single
`RustExpr::Var("Box::new")`. The structured branch of
`erased_object_box_projection` now uses `RustExpr::Path(["Box", "new"])`, and
the existing erased-object facade-return assertions pin that shape.

Boxing into a non-`Any` trait object now follows the same boundary:
`boxed_dyn_trait_projection` emits a structured `Cast` around a structured
`Box::new` call whenever the input is structured, and keeps the raw fallback
only for legacy raw input. The fallback also shares the erased-object path's
top-level comma guard, so raw tuple payloads are not split across call
arguments.

Erased-object constructor boxing now keeps its unit and multi-argument payloads
structured too. `erased_object_constructor_projection` feeds
`RustExpr::Tuple(Vec::new())` or `RustExpr::Tuple(args)` into the structured
boxing helper instead of first rendering `()` / `(left, right)` as raw strings.

Nullable enum switch labels in `lower.rs` also stopped manufacturing the whole
`Some(...)` expression as raw text. Source enum labels now build
`RustExpr::Path` when the variant path is plain, external enum constants reuse
the existing associated-path projection, and both are wrapped in a shaped
`Some` call.

The optional-borrowed string-literal argument fallback in `lower.rs` now does
the same at the wrapper boundary. The literal payload can remain raw, but
`lower_optional_argument_for_borrowed_target` now applies the existing borrow
projection and wraps it in the shaped `Some` helper instead of formatting
`Some(&{rendered})` as one opaque string.

Erased-object clone recovery now keeps helper calls structured for structured
inputs. `clone_erased_object_ref_once(value)` and the
`Coercion::CloneErasedObject` scalar/vector arms build `RustExpr::Call` nodes to
`java_clone_erased_object_ref` / `java_clone_erased_object_vec_ref` with a
structured borrow argument. The idempotence check asks for an existing
one-argument structured helper call before falling back to rendered legacy text,
so raw fragments keep their old compatibility behavior.

The raw fallback for Rust scalar integral narrowing now parenthesizes compound
method receivers before adding `.try_into().unwrap()`. Structured inputs were
already represented as nested `RustExpr::MethodCall` nodes; this closes the
legacy spelling bug where an opaque `left + right` fallback emitted
`left + right.try_into().unwrap()` instead of applying the conversion to the
whole expression.

Optional borrowed-facade returns now keep the outer optional adapter structured
too. `Option<&Facade>` to `Object`/`Optional<Object>` still uses a raw closure
leaf for `|value| Box::new(value.to_path_buf()) as Box<dyn Any>`, but the
surrounding `.map(...)` and, when needed, `.expect("Java null receiver")` are
`RustExpr::MethodCall` nodes rather than one opaque rendered chain.

Optional receiver projection raw fallbacks also keep their adapter tails
structured now. A legacy raw receiver such as `values.get(key)` remains a raw
leaf, but the `.expect(...)`, `.as_ref().unwrap()`, and `.as_mut().unwrap()`
steps are `RustExpr::MethodCall` nodes. Compound raw receivers are parenthesized
before the method chain, so `left + right` becomes
`(left + right).as_ref().unwrap()` rather than binding the unwrap only to the
right-hand side.

Borrowed sequence `extend` arguments now own their supported iterator recovery
chains as method-call IR as well. The source collection can still be a raw leaf,
and the `String` recovery closure remains raw, but `.iter()`, `.map(...)`,
`.copied()`, and `.cloned()` are no longer emitted as one opaque string for
supported policies. The unsupported recovery case stays raw because it carries a
generated audit comment.

Borrowed Vec/Set element recovery now applies the same split when converting a
borrowed collection back to owned storage. The collection source can remain a
raw leaf and `|v| v.to_string()` remains a raw closure, but the
`into_iter().map(...).collect::<...>()` and
`into_iter().copied().collect::<...>()` tails are method-call IR for supported
Vec and HashSet recovery policies.

Two more fixed-value native projections are structural now. `Reader.ready()`'s
stubbed `true` result is a boolean literal node, and `new URI(x)` lowers through
a `URI::parse` path node instead of an opaque function-name string. Both emit
the same Rust as before, but they no longer require the call path to be
recovered from raw text later.

Static storage initialization no longer falls back to raw text for two simple
known shapes either. A null optional fallback is a `None` path node, and
non-String static constant references keep the associated-path IR returned by
`self_associated_path_projection` instead of rendering it and wrapping the text
back into `RustExpr::Raw`.

Source-enum static API calls are path-shaped now too. The lowering for
`Kind.values()` and `Kind.valueOf(name)` builds calls to the owner's associated
`values` / `value_of` path instead of formatting the full call text in
`lower_source_java_enum_static_call`.

The generic borrowed-element unwrap fallback also reuses the structured
`expect("Java null receiver")` projection now. Raw receivers still emit the same
text through that helper, while structured optional-carrier values keep a
`RustExpr::MethodCall` tail instead of being rendered first and wrapped back as
raw text.

Optional string-view mapping has the same split now. The common
`option.as_ref().map(|value| value.as_str())` adapter is centralized in
`optional_string_view_projection_expr`: structured option receivers produce
method-call IR for `.as_ref().map(...)`, while legacy raw receivers keep the old
rendered spelling.

The simpler optional shared-native map arm follows the same rule. Mapping
`Option<T>` to an optional shared-borrow native element now builds the
`.as_ref()` tail with `zero_arg_method_projection`, so structured option
receivers keep a method-call node and raw receivers keep their compatibility
spelling.

The remaining optional mapped-value arms now share `optional_map_projection_expr`
for their receiver tails. The closure body is still raw because it is generated
from a mapped `value` expression, but `.as_ref().map(...)`, `.as_mut().map(...)`,
and direct `.map(...)` are method-call IR for structured receivers, with the old
rendered spellings preserved for raw receivers.

Top-level uncaught-exception recovery for `Result` expressions is structured for
ordinary expression IR now. `unwrap_result_or_panic_uncaught_java_exception`
still keeps its raw rendered parser for legacy `?` fragments, but a structured
receiver builds a `RustExpr::MethodCall` for `.unwrap_or_else(...)` instead of
rendering the full chain and wrapping it back into `RustExpr::Raw`.

The inner-class companion handle argument no longer manufactures clone calls as
raw text either. Captured-inner `self.__outer.clone()` and current outer receiver
`__java_self.clone()` both go through field/clone projection IR now. This only
removes the opaque handle argument expression; ordinary outer-method inner
construction remains tied to the open shared-receiver work tracked by 0023.

Map-key normalization now keeps a known shared-borrow `String` local structured
when adding the `.as_str()` view. Raw `&key` compatibility still emits the old
`key.as_str()` text, but structured `&key` collapses to `key` and then builds the
`as_str` tail through method-call IR.

`Math.round` lowering no longer turns raw operands into a fully raw helper call.
`math_round_projection` now always builds a `RustExpr::Call` to
`java_math_round_f32` / `java_math_round_f64`; a raw argument remains only as the
operand leaf, including the legacy `left + right as f64` emission for non-float
sources.

`Math.abs` follows the same raw-leaf rule now. Raw input still gets
parenthesized to preserve the old `(left + right).wrapping_abs()` spelling, but
the `.abs()` / `.wrapping_abs()` tail is a `RustExpr::MethodCall` rather than
part of one opaque string.

The one-argument f64 math-method adapter now uses the same structured path for
raw and non-raw inputs. `primitive_f64_method_projection` always builds a cast
receiver plus method-call tail; raw input remains only as the cast operand leaf,
so the existing `(left + right as f64).ln()` emission is unchanged.

The two-argument f64 method adapters now share that same cast helper. `atan2`
and `powf` no longer fall back to one raw string when either operand is opaque;
each side is a structured `as f64` cast and the method call is structured, with
raw operands retained only as cast leaves.

The central `.expect(...)` projection follows the raw-leaf rule now too.
`method_expect_projection` no longer renders raw receivers into a larger raw
string; callers get a `RustExpr::MethodCall` tail and any opaque receiver remains
only the receiver leaf.

The central zero/normal method-call helper now follows that same rule.
`method_call_projection` always returns `RustExpr::MethodCall`; it no longer has
a raw-receiver branch that formats `receiver.method(args)` as one opaque string.
That moved `.as_deref()`, `.unwrap()`, `.clone()`, and similar helper-mediated
tails into structured IR while preserving legacy raw receivers as leaves.

The Java length/row-count-to-`i32` adapter uses the same boundary now.
`method_call_as_i32_projection` always builds `Cast(MethodCall(receiver, ...),
i32)`, so `len() as i32`, `nrows() as i32`, and `ncols() as i32` expose their
method tail and cast to later visitors even when the receiver itself is a raw
legacy leaf.

Logical negation is structured for raw operands now as well.
`logical_not_projection` always returns `RustExpr::Unary { op: "!", ... }`.
Rendered legacy predicates such as `left && right` remain grouped raw operand
leaves, while the `!` operator is visible to later IR visitors and precedence
handling.

Primitive `compareTo` helper calls are structured for raw operands too.
`primitive_compare_to_projection` now always builds a `RustExpr::Call` to the
`java_primitive_compare_to` path; opaque operands remain only call-argument
leaves instead of forcing the whole helper invocation back into raw text.

Native collection malformed-arity fallback zeros in `call_lowering.rs` are
literal nodes now. `Arrays.copyOf`, `Arrays.copyOfRange`, and
`Collections.nCopies` no longer use `RustExpr::Raw("0")` for missing numeric
arguments.

Static storage initializer references use projection helpers for simple string
and length adapters now. String references to associated constants build a
structured `.to_string()` method-call tail, and static string `.length()` /
`.len()` initializers route through `len_i32_projection` instead of formatting a
raw `.len() as i32` suffix.

Static string constant concatenation also keeps the call boundary structured
now. The generated `format!` macro token remains a raw callee leaf, but the
`format!("{}{}", left, right)` wrapper is a `RustExpr::Call` with literal and
expression arguments instead of one fully rendered raw expression.

`string_to_char_array_projection` follows the same raw-leaf rule. A raw string
receiver remains an opaque receiver leaf, but the `.chars().collect::<Vec<char>>()`
tail is now structured method-call IR instead of a freshly formatted raw chain.

The fallible numeric binary lowering temps also stopped using `RustExpr::Raw`
for plain identifiers. `__java_left` and `__java_right` are `RustExpr::Var`
nodes before being passed through the structured numeric operator helpers. The
surrounding fallible block expression remains raw and is still a future
composition target.

No-op facade method call-site markers are represented explicitly now. The
lowering builds a `RustExpr::MarkedBlock` carrying the marker text and lowered
call expression instead of rendering both into one raw block expression. Emission
is unchanged, so the existing output-marker counters still see the same marker
surface while later IR visitors can inspect the wrapped call.

Optional owned-value recovery now keeps its receiver chain structured for raw
receivers too. `optional_get_owned_value_projection` and
`optional_or_else_owned_value_projection` share the same
`.as_ref().map(|value| (*value).clone())` method-call IR; the closure remains a
raw leaf, but `.expect("Java null receiver")` and `.unwrap_or(...)` are no
longer part of a fully formatted raw expression.

The defensive malformed-arity fallback for `Buffer.put` no longer emits a raw
`{ }` block expression. It now reuses the structured unit expression helper; the
normal modeled one-argument path still discards the argument through the existing
discard-value projection.

The erased-object subtype-to-supertype slot adapter no longer formats
`Box::new(value) as target` as one raw expression when the source value is a
shared handle. `lower_source_subtype_to_supertype_slot` now builds a structured
`Box::new` call and wraps it in `RustExpr::Cast`; the cast target type remains
the rendered type string carried by the cast node.

Optional element map projection has one structured receiver path now. Raw
optional receivers keep their opaque source as the receiver leaf, while the
`.as_ref()` / `.as_mut()` adapter and `.map(|value| ...)` tail are method-call
IR. The generated closure body remains a raw leaf because it is assembled from
the mapped value expression.

Interface `I.super.defaultMethod(...)` lowering no longer renders the trait
helper call as one raw string. `lower_interface_super_default_call` now builds
the default helper callee with `owner_associated_path_projection` and passes
`self` plus lowered arguments through `function_call_projection`.

Optional constructed string-view normalization also stopped rebuilding accepted
`.as_str()` payloads as raw text. The recognizers for `&value.as_str()` and
`Some(&value.as_str())` remain exact, but once they accept a legacy raw spelling
the `value.as_str()` payload is a structured method-call tail over the raw
receiver leaf.

Optional string display no longer stores the `std::convert::AsRef::<str>::as_ref`
callee as a raw expression. `JavaDisplayPlan::OptionalString` now builds the
callee as a `RustExpr::GenericPath`; emission is unchanged, but downstream IR
visitors can see that the mapped closure calls a real path instead of opaque
text.

Sorted-map navigation key lowering also stopped rendering the whole iterator
chain through `emit_expr` and `RustExpr::Raw(format!(...))`. `lowerKey`,
`floorKey`, `ceilingKey`, and `higherKey` now build structured `Bound` tuples,
`range::<_, _>`, `next`/`next_back`, and the final `map(|(key, _)| key.clone())`
closure as Rust IR. The broad `rendered_string_heuristics_have_centralization_guardrails`
inventory is green again, and the 99-fixture differential gate still passes.

2026-08-25: unresolved receiverless collection default fallbacks no longer build
`<{ty} as Default>::default()` with `RustExpr::Raw(format!(...))`. `RustExpr` has
a `TraitAssociatedPath { ty, trait_name, member }` node now, emitted as
`<Type as Trait>::member`, and the `addAll(defaultNames())` fallback composes
that path with a structured zero-argument call plus the existing postfix TODO
comment. The focused `unresolved_set_return_in_add_all_uses_target_slot_shape`
fixture still emits the same Rust and compiles its generated module.

Follow-up the same day: prefix-borrow classification now treats
`TraitAssociatedPath` as structured prefix-safe syntax, matching `GenericPath`
and `TypeAssociatedPath`. The focused borrow-idempotence unit test covers
`&<String as Default>::default` directly.

Another 2026-08-25 cleanup removed a rendered-string fallibility heuristic from
erased-object clone recovery. `borrowed_erased_object_clone_arg` no longer calls
`emit::emit_expr(&expr)` to infer that raw text ending in `ok_or_else(...)?` is
already borrowed-safe. Untagged raw text is now borrowed like any other opaque
expression; structured `RustExpr::Try` and shaped result propagation remain the
accepted evidence.

Follow-up the same day: central rendered-expression recovery now promotes
complete top-level `+` expressions when their operands can already be recovered
structurally. `left + 12`, `left + format!("{}", name)`, and
`left + right.method(value)` are now `RustExpr::Binary` trees with literal,
macro-call, or method-call operands instead of one opaque raw string. The
`left + right as usize` spelling intentionally remains raw for now because the
current emitter parenthesizes structured/raw cast operands inside `+`, which
would churn generated Rust without fixing a semantic hole.

Follow-up the same day: the old clone-comment failure mode is now pinned
directly. `rendered_expr_has_clone_recovery` may ignore block comments before
checking generic clone suffixes, but `value.clone()/* TODO... */.into()` is not
accepted as an already-recovered clone just because the removed comment-specific
`contains` arm would have matched it. The guard keeps the comment text inert
while preserving the structured `.into()` clone-recovery output.

Follow-up the same day: arithmetic display conversion no longer has the
`raw_arithmetic_display_conversion_receiver` strip/scan/reparse path. A raw
`(left + right).to_string()` spelling is first recovered through the central
rendered-expression projection, then the existing structured
`RustExpr::MethodCall { method: "to_string" }` and `RustExpr::Binary` checks
decide whether the conversion is redundant. Ambiguous raw text such as
`value * (100 as f64).to_string()` is no longer treated as if the whole
arithmetic expression had been converted to a string.

Follow-up the same day: one-argument string runtime helpers now recover parseable
raw second operands before building the helper call. `compareTo`,
`compareToIgnoreCase`, and `equalsIgnoreCase` still emit the same helper calls,
but a legacy `right.as_str()` operand is represented as a structured
`RustExpr::MethodCall` argument instead of remaining opaque inside the otherwise
structured `RustExpr::Call`.

Follow-up the same day: borrowed-element slot adapter input recovery now falls
back through `RustExpr::raw_leaf(rendered)` instead of directly rebuilding
`RustExpr::Raw(rendered)`. Emission is unchanged, but this keeps the unavoidable
opaque fallback behind the centralized raw-leaf constructor after the method
receiver recovery attempt fails.

Follow-up 2026-08-25: the optional-erased-string `Raw(format!(...))` hits found
by the direct source scan were test fixtures, not production builders. The
production optional-erased-string path is already guarded to use structured
projection first and to reserve rendered recovery for raw leaves. Added
`layering::coercion_production_does_not_build_raw_expression_with_format` so
production `coercion.rs` cannot reintroduce direct `RustExpr::Raw(format!(...))`
without an explicit test failure. This does not close 0007; it fences one noisy
search class and keeps future cleanup focused on real production raw producers.

Follow-up 2026-08-25: the raw compatibility path for integral parse null
normalization no longer turns the stripped receiver back into
`RustExpr::Raw(receiver.to_string())` before applying the optional-string display
plan. It now goes through `rendered_plus_expr_projection_or_raw_leaf`, so
parseable receivers such as optional map/list access chains recover to ordinary
expression IR and only truly opaque receiver text remains a raw leaf. Regression:
`lower::tests::integral_parse_nullable_arg_maps_raw_optional_string_expect_null_to_number_format_input`.

Follow-up 2026-08-25 audit pass: a direct scan for
`RustExpr::Raw(format!(...))` / `RustStmt::Raw(format!(...))` in the core
lowering files did not find remaining production builders. The `coercion.rs`
matches were all under its `#[cfg(test)] mod tests` recovery fixtures, and
`exception_lowering.rs` / `array_lowering.rs` direct raw values in the sampled
output are also test inputs or centralized compatibility consumers. The live
0007 residue is therefore not the old direct-formatting class; it is the
remaining intentional `RustExpr::raw_leaf` fallback surface after structured
recovery fails.

Follow-up 2026-08-25: the centralized rendered-expression recovery now handles
simple unary `!expr` and `*expr` leaves before falling back to `RustExpr::Raw`.
The helper only accepts the recovery when the operand is also structural, so
opaque/broken raw text still remains a raw leaf. Regression:
`coercion::tests::rendered_projection_recovers_borrowed_leaf_expressions`.

Follow-up 2026-08-29: borrowed Vec/Set clone recovery no longer has a dedicated
raw `strip_suffix` parser for `.collect::<...>()` and
`.cloned().collect::<...>()`. Raw inputs first pass through central rendered
expression recovery; only structurally recovered collect calls are treated as
already collected or collectable, while malformed suffix-looking fragments stay
opaque and use the ordinary fallback receiver path. Regression:
`coercion::tests::raw_borrowed_collect_clone_compat_recovers_structured_collect_calls`.

Follow-up 2026-08-26: raw comma helpers now share the raw-string literal
scanner. `raw_contains_top_level_comma` had a one-off
`raw[idx..].starts_with("r#\"")` branch that recognized only one raw-string
spelling, so commas inside forms such as `r##"a, b"##` could still be treated as
top-level separators. It now uses `raw_string_literal_end_at`, matching the
splitter. The private text-splitting helper was also renamed so the source guard
anchors on the real scanner body instead of a wrapper/helper prefix collision.
Regressions:
`coercion::tests::raw_comma_splitter_ignores_commas_inside_raw_string_literals`
and
`slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

Follow-up 2026-08-26: checked Java-null parse receiver recovery no longer
rebuilds its `ok_or_else` closure body from a raw
`JavaException::new("NullPointerException", ...)` string. It now reuses the
structured `exception_lowering::new_exception_expr` helper, so the recovered
`value.as_ref().ok_or_else(...)?` shape stays inspectable through the closure
body. Emitted Rust is unchanged. Guarded by
`slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

The same guard also exposed a stale source anchor for the arrays exception
panic helper. The helper is now anchored under its current
`arrays_java_exception_panic_expr` name and uses `rust_string_literal_projection`
for both exception class and message instead of rebuilding string literals with
`RustExpr::Literal(format!(...))`.

Follow-up 2026-08-26: catchable Java-null rewriting in `lower.rs` now shares the
same structured exception constructor. `java_null_pointer_exception_expr` calls
`exception_lowering::new_exception_expr` with `rust_string_literal_projection`
instead of hand-building `JavaException::new` from `RustExpr::Literal` pieces.
The rendered Rust stays the same; the NPE constructor is no longer duplicated
between `coercion.rs` recovery and `lower.rs` rewriting. Guarded by
`slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

The same centralization pass also moved the list/string/index bounds exception
helpers in `call_lowering.rs` and `coercion.rs` onto
`exception_lowering::new_exception_expr`. Those helpers still emit the same
`JavaException::new(..., "...".to_string())` Rust, but they no longer duplicate
the exception-call shape with local `RustExpr::Literal` construction. The
guard now checks these helpers as well.

Follow-up 2026-08-26: the adjacent `NoSuchElementException` result helpers now
share that path too. `map_key_result_projection` and
`optional_get_owned_value_result_projection` still render the same
`ok_or_else(|| JavaException::new(...))` Rust for empty sorted maps and
`Optional.get()`, but their closure bodies now come from
`exception_lowering::new_exception_expr` with `rust_string_literal_projection`.
The centralization guard covers these helpers.

Follow-up 2026-08-26: the fallible array-index read path in `lower.rs` now uses
the same constructor helper for its `ArrayIndexOutOfBoundsException` closure.
The dynamic `format!("Index {} out of bounds for length {}", ...)` message
remains structured as a `RustExpr::MacroCall`, but the surrounding exception
call is no longer hand-built from local `JavaException::new` pieces.

The array range panic/return helpers in `coercion.rs` were moved over in the
same style. `arrays_java_exception_panic_expr`, `return_java_exception_stmt`,
and `panic_java_exception_stmt` now use the central constructor while keeping
their existing panic/`Err(...)` wrappers and rendered messages.

Follow-up 2026-08-26: primitive result exception helpers followed the same
path. `math_to_int_exact_result_projection` and the radix-out-of-range branch
inside primitive parse now use `exception_lowering::new_exception_expr` for
`ArithmeticException`/`NumberFormatException`, with string messages carried by
`rust_string_literal_projection`. The existing `JavaException::from_display`
parse-error path is a different API and remains explicit.

Follow-up 2026-08-26: checked Java integer division/remainder lowering in
`operator_lowering.rs` now uses the central exception constructor for its
catchable `/ by zero` `ArithmeticException` path. The surrounding `Err(...)`
block stays structured; only the duplicated local `JavaException::new` pieces
were removed.

Follow-up 2026-08-26: the integral-parse nullable-argument compatibility path
now preserves unrecoverable raw input through `RustExpr::raw_leaf(rendered)`
instead of directly constructing `RustExpr::Raw(rendered)`. Parseable raw
optional-string receivers still recover before applying the Java `"null"`
number-format input rule; only the opaque fallback was centralized.

Follow-up 2026-08-26: the broad source guard now strips `#[cfg(test)]` items
instead of truncating each file at the first test-only attribute. That matters
because files such as `coercion.rs` contain small test-only helper islands before
later production helpers. The same guard now pins the only production raw
constructors to `RustExpr::raw_leaf` / `RustStmt::raw_leaf` in `rust_ir.rs`.

Follow-up 2026-08-26: the unrecoverable raw-statement fallback in
`make_java_null_expect_catchable_stmt` now also goes through
`RustStmt::raw_leaf` after structured recovery fails. Emitted Rust is unchanged,
but the catchable-null compatibility path no longer reconstructs a raw
statement directly. Regression coverage remains under
`lower::tests::make_java_null_expect_catchable_recovers_raw_block_statements`
and the source guard
`slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

Follow-up 2026-08-26: stream lambda raw-expression ingress now recovers the
narrow Java static-member spelling `Owner.VALUE` to a structured associated
path before generic raw recovery. The matcher is intentionally local to
`stream_raw_expr` and requires exactly one dot plus an uppercase-looking owner,
so lowercase instance-field-looking raw text such as `owner.value` is left to
generic recovery and remains a field projection rather than an associated path.
Regressions:
`slot_shape_design::stream_lambda_emission_uses_structured_operator_rendering`
and the source guard
`slot_shape_design::stream_lambda_lowers_split_length_and_integer_parse_in_nullable_map`.

Follow-up 2026-08-26: `Coercion::AsSlice` associated-path compatibility no
longer treats raw text containing `::` plus identifier characters as semantic
path evidence. Raw inputs now pass through `rendered_projection_or_raw_leaf`;
only a recovered `RustExpr::Path` suppresses `.as_slice()`. The valid
`Owner::VALUES` compatibility case still recovers to a structured path, while
malformed path-looking text remains an opaque receiver. Regressions:
`coercion::tests::as_slice_path_compatibility_does_not_trust_literal_text` and
`slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

Follow-up 2026-08-26: rendered function-call recovery now shares the central
raw-string literal scanner when looking for the top-level call parentheses.
`rendered_call_parts` no longer has a one-off `r#"` branch that stops at the
next quote and misses multi-hash raw strings; it uses
`raw_string_literal_end_at`, matching the comma and operator scanners. A call
such as `helper(r##"a ) b"##, value)` now recovers as a structured
`RustExpr::Call` instead of treating the `)` inside the raw-string body as
the call close syntax. Regressions:
`coercion::tests::rendered_projection_recovers_borrowed_leaf_expressions`,
`coercion::tests::raw_comma_splitter_ignores_commas_inside_raw_string_literals`,
and `slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

Follow-up 2026-08-27: mutable-alias detection no longer jumps straight from a
`RustExpr::Raw` leaf to the rendered-string compatibility matcher. Raw inputs
first pass through `rendered_projection_or_raw_leaf`; recovered method/unary
projections are checked by the same structured `rust_expr_mutable_alias_projection`
used for non-raw IR, and the substring fallback is reserved for unrecoverable
legacy leaves. Regressions:
`slot_shape_design::rendered_mutable_alias_projection_names_same_call_alias_shapes`
and `slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

Follow-up 2026-08-26: production optional-element borrow lowering no longer
falls back to the ownership-only carrier projection helper after already
computing a `LoweredType`. `conversion_lowering::apply_reused_optional_unwrap`
and the two optional-element borrow lowerers in `lower.rs` now require
`LoweredType::OptionalElementBorrow` before rendering `.as_ref()` / `.as_mut()`,
so those production projections keep the precise recorded element slot shape.
The ownership-only helper remains only as a documented compatibility surface
with direct unit coverage; no production caller depends on its imprecise
`OptionalProjected` tag.

Follow-up 2026-08-28: erased-object downcast suppression for unresolved static
owner calls no longer treats raw text containing
`unimplemented!("unresolved Java static call ...")` as semantic evidence. The
detector now accepts only the structured `RustExpr::MacroCall` shape produced by
lowering, while literals or opaque raw leaves that merely contain the message are
not exempted from normal downcast adaptation. Regression:
`coercion::tests::unresolved_static_owner_detection_is_structural_not_raw_text`;
the production source guard
`layering::coercion_production_does_not_build_raw_expression_with_format` now
also pins this detector against reintroducing a `RustExpr::Raw` branch.

Follow-up 2026-08-28: the same-outer inner-constructor compatibility path no
longer discards successful structured recovery when no storage wrapping or
outer-companion retargeting is needed. Whole raw constructor calls such as
`OuterB::new_1(value)` now become structured `RustExpr::Call` nodes even when
the helper returns `false` for "no semantic rewrite"; opaque or non-call raw
text remains untouched. Statement recovery follows the same rule for simple
recoverable raw statements. Regressions:
`lower::tests::same_outer_inner_constructor_noop_rewrite_recovers_raw_constructor_call`,
`lower::tests::same_outer_inner_constructor_raw_stmt_rewrite_recovers_without_semantic_change`,
and `slot_shape_design::constructor_delegation_uses_structured_paths_without_raw_scanning`.

Follow-up 2026-08-28: Java `String.format(Locale, ...)` overload detection no
longer treats raw text prefixes such as `Locale::` or `Locale::US + fallback`
as semantic locale arguments. Raw inputs first go through the central rendered
expression recovery, and only recovered `Var`/`Path` forms headed by `Locale` or
`JavaUtilLocale` are accepted. Regression:
`coercion::tests::string_format_arg_projection_owns_display_to_string_spelling`.

Follow-up 2026-08-28: struct-literal field reordering in `item_assembly.rs` no
longer treats raw text tokens as move/read evidence. Raw expression and simple
raw-statement compatibility inputs first go through the central parseable-raw
recovery helper; only recovered structured IR can prove a direct move or a
read of another field source. Opaque raw text such as `left +` or marker text
inside a string literal no longer influences field ordering. Regressions:
`item_assembly::tests::raw_direct_move_vars_require_structured_recovery`,
`item_assembly::tests::raw_reads_require_structured_recovery`, and
`layering::item_assembly_return_type_wrappers_delegate_to_fact_aware_rendering`.

Follow-up 2026-08-28: borrowed method receiver parenthesizing now attempts
parseable raw recovery before deciding from rendered text. `&reader.field` and
`(&reader.field)` both normalize to structured grouped unary-borrow IR; only
unrecoverable raw borrow text remains an opaque raw leaf instead of being
grouped from a `starts_with('&')` text predicate. Regression:
`type_coercion::parenthesizes_borrowed_method_receiver` and the source guard
`slot_shape_design::slot_rendering_owns_rust_type_rendering_boundaries`.

Follow-up 2026-08-28: optional default detection no longer accepts a rendered
typed `None` as a single path segment. `RustExpr::Path(["None::<T>"])` is now
rejected; the valid structured form is `RustExpr::GenericPath { base: ["None"],
args: [T], member: None }`, and raw compatibility for rendered `None::<T>`
continues to recover into that shape through the narrow `None`/`Default`
projection helper. Regression:
`lower::tests::none_or_default_detection_uses_structured_raw_recovery_only`.
The source guard
`slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`
also strips the `#[cfg(test)] mod tests` tail module directly, so braces inside
test fixture strings no longer make test-only `RustExpr::Raw` constructors look
like production sites.

Follow-up 2026-08-28: rendered range recovery is now fail-closed at the endpoint
boundary. A raw `a..b` compatibility leaf is recovered only when both endpoints
recover to structured Rust IR and neither endpoint is the reserved `self`
pseudo-identifier. This keeps valid generated range shapes structured, while
leaving opaque fragments such as `self..broken` raw so assignment recovery,
source-raw expression lowering, and borrowed-string temporary hoisting do not
infer semantics from invalid compatibility text. Regression coverage:
`lower::tests::source_raw_expr_lowering_recovers_parseable_projection_before_raw_leaf`,
`lower::tests::borrowed_string_temporary_hoist_keeps_parseable_raw_structured_without_hoist`,
`lower::tests::raw_assignment_projection_recovers_bitwise_and_shift_compound_ops`,
and the broad `cargo test raw_ --lib` slice.

Follow-up 2026-08-28: integral-parse nullable argument recovery no longer
recognizes `.as_ref().expect("Java null receiver").as_str()` by stripping that
suffix from opaque raw text. The compatibility helper now recovers the whole
rendered expression first and requires the structured
`as_str(expect(as_ref(receiver)))` chain with a non-raw receiver before applying
Java's `"null"` `NumberFormatException` input policy. Malformed raw text such
as `self..broken.as_ref().expect("Java null receiver").as_str()` remains a raw
leaf instead of being treated as semantic nullable-string evidence. Regression:
`lower::tests::integral_parse_nullable_raw_null_expect_requires_structured_receiver_recovery`.

Follow-up 2026-08-28: `strip_redundant_java_null_expect_after_unwrap` no
longer runs literal-blind global replacements over `RustExpr::Raw` text. Raw
inputs first go through central rendered-expression recovery; only recovered
structured method-call chains can collapse redundant `.expect("Java null
receiver")` layers. Opaque text, including format-string literals containing
the same token sequence, is preserved. Regression:
`coercion::tests::redundant_java_null_expect_strip_does_not_rewrite_raw_literals`.

Follow-up 2026-08-28: `strip_terminal_unwrap_projection` no longer strips a
`.unwrap()` suffix directly from opaque raw text. Raw inputs are recovered
first, and only a structured zero-argument terminal `unwrap` call is removed;
literal text such as `format!("value.unwrap()")` is preserved. Regression:
`coercion::tests::strip_terminal_unwrap_projection_requires_structured_raw_recovery`.
The unrecovered fallback now goes through `RustExpr::raw_leaf`, as does the
matching opaque fallback in `strip_redundant_java_null_expect_after_unwrap`, so
these consumers no longer open-code production `RustExpr::Raw` construction.

Follow-up 2026-08-28: optional string-view construction's raw element
compatibility no longer manufactures a `StringView` from any text matching
`&...as_str()`. The helper still accepts string-literal spellings and parseable
receivers, but an unrecovered raw receiver such as `&name trailing.as_str()`
now stays opaque instead of becoming semantic evidence. Regression:
`coercion::tests::optional_string_view_construction_normalizes_structured_borrow_by_tag`.

Follow-up 2026-08-28: arithmetic display conversion no longer treats a raw
receiver with an arithmetic-looking token as redundant when rendered recovery
failed. `left +` inside a structured `.to_string()` receiver now keeps the
conversion instead of being returned as an arithmetic display operand.
Regression: `coercion::tests::java_display_operand_removes_redundant_arithmetic_to_string`.
The same pass tightened grouped rendered recovery: `(left +)` now remains a raw
compatibility leaf instead of becoming `Grouped(Raw(...))`, so callers cannot
mistake a parenthesized malformed fragment for structured IR.

Follow-up 2026-08-28: `lower.rs` catchable-null expression rewriting now keeps
the unrecovered raw fallback behind `RustExpr::raw_leaf` instead of rebuilding
`RustExpr::Raw(rendered)` directly. Parseable raw expressions still recover and
rewrite embedded Java-null expects; opaque malformed raw text is preserved as an
opaque compatibility leaf. Regression:
`lower::tests::make_java_null_expect_catchable_recovers_parseable_raw_expression`.

Follow-up 2026-08-29: string-search char-literal coercion no longer strips a
leading `&` from raw text. Raw compatibility now recovers the whole rendered
fragment first, so `&'='` can become a structured borrowed char literal while
`&'=' trailing` remains opaque. Borrow-projection idempotence also no longer
uses a raw `starts_with('&')` check; raw inputs must recover to a structured
unary borrow before they suppress an additional borrow. Regressions:
`coercion::tests::string_index_and_substring_projections_own_runtime_helper_shapes`
and
`coercion::tests::raw_borrow_projection_idempotence_requires_complete_structural_recovery`.

Follow-up 2026-08-29: raw deref string-view recovery no longer strips a leading
`*` from arbitrary opaque text. The compatibility path now accepts a simple
identifier deref or a complete recovered unary deref expression; malformed raw
text such as `*name trailing` is grouped as an opaque receiver before `.as_str()`
is attached. Regression:
`coercion::tests::as_str_coercion_removes_simple_borrowed_item_deref`.

Follow-up 2026-08-29: anonymous dyn-trait constructor idempotence no longer
parses `<dyn ...>::anonymous()` with a dedicated raw prefix/suffix helper. Raw
compatibility now goes through central rendered-expression recovery and only a
complete structured zero-argument anonymous call can suppress clone-box
recovery. Regression:
`coercion::tests::clone_box_parenthesizes_dyn_trait_cast_receiver`.

Follow-up 2026-08-29: `String.join` null-item detection no longer decides from
direct raw text equality. The compatibility path now recovers only complete
`None`, `None::<T>`, or `Default::default()` projections, with balanced
turbofish type text for the generic `None` case; malformed raw fragments such
as `None trailing` and `Default::default() trailing` remain ordinary opaque
values. Regressions:
`coercion::tests::string_join_projection_helpers_own_literal_and_to_string_idempotence`
and `slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

Follow-up 2026-08-29: top-level plus and simple binary rendered-expression
recovery no longer produce a partially structured `RustExpr::Binary` when
either operand remains an opaque raw leaf. Complete recovered leaves still
compose, so cases like `left + format!("{}", name)` and `self.value ==
other.value` remain structured, while malformed fragments such as `left + a ?
b` and `left == a ? b` stay raw as a whole. Regression:
`coercion::tests::rendered_projection_recovers_borrowed_leaf_expressions`.

Follow-up 2026-08-29: optional borrowed sequence iteration no longer has a
separate panic-only branch hidden in the sequence source projection helper.
Borrowed, mutable-borrowed, and owned optional sequence sources now share the
structured `ok_or_else(...)?` Java-exception propagation shape when the caller
is lowering inside a catchable result context; non-propagating callers keep the
old `expect("Java null receiver")` behavior. Regression:
`coercion::tests::jdk_collection_static_projections_own_expression_shapes`.

Follow-up 2026-08-29: rendered block recovery no longer rejects every block
whose text contains a semicolon. A single complete expression statement such as
`{ value.clone(); }` now recovers to a structured `RustExpr::Block` with an
expression statement, semicolons inside string/raw literals no longer force the
block to stay raw, and multi-statement blocks still fail closed as raw leaves.
Regression:
`coercion::tests::rendered_projection_recovers_borrowed_leaf_expressions`.

Follow-up 2026-08-29: the call-target lowering layering guard now scans only
production `call_lowering.rs` before `#[cfg(test)]`, so test helpers can render
expressions while production `apply_call_target` remains checked against direct
`emit::emit_expr` use. This is guard-scope cleanup rather than a runtime
lowering change. Regression:
`layering::call_target_lowering_uses_shared_projection_helpers`.

Follow-up 2026-08-29: struct-literal constructor return move reordering no
longer lets `rust_expr_direct_move_var` or `rust_expr_reads_var` decide from a
`RustExpr::Raw` leaf. Legacy parseable field expressions are recovered once at
the `constructor_return_body` boundary, before reordering; the direct-move and
read predicates themselves now accept only structured expression evidence. This
keeps old parseable constructor fields ordered correctly while removing two
semantic consumers of rendered raw text. Regressions:
`item_assembly::tests::raw_direct_move_vars_require_structured_recovery` and
`item_assembly::tests::constructor_return_field_recovery_happens_before_move_reordering`,
and `item_assembly::tests::raw_reads_require_structured_recovery`.

Follow-up 2026-08-29: primitive parse receiver lowering no longer strips a
leading `&` directly from raw text. The raw compatibility path now recovers the
complete rendered expression first and removes only a structured unary borrow,
so `&value` still feeds `value.parse::<T>()` while compound or malformed raw
text keeps the borrow inside the grouped receiver. Regression:
`coercion::tests::primitive_parse_projection_owns_parse_call_spelling`.

Follow-up 2026-08-29: same-outer inner-constructor raw statement recovery no
longer routes through the broad `raw_structured_stmt_projection` helper. The
statement compatibility path now recovers only assignment, return, or expression
statements whose recovered structure contains a plain `new*` constructor call;
unrelated raw statements such as `helper(value);` and
`return helper(value);` stay opaque when this constructor-specific pass walks
them. Constructor-bearing statements remain recoverable so legacy parseable
constructor spellings can still move toward structured IR. Regressions:
`lower::tests::same_outer_inner_constructor_raw_stmt_rewrite_recovers_without_semantic_change`
and
`slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

Follow-up 2026-08-29: item-assembly constructor-return move reordering no
longer asks `RustStmt::Raw` text whether it reads a local. Parseable raw
statement compatibility inside block-valued struct fields is recovered at the
`constructor_return_body` boundary, before read/move analysis; unrecovered raw
statements now fail closed in `rust_stmt_reads_var`. Regression:
`item_assembly::tests::constructor_return_block_field_recovery_happens_before_move_reordering`.

Follow-up 2026-08-29: raw `None`/`Default::default()` detection in lowering no
longer owns a separate rendered-text parser. The lower-side raw arm now calls
the narrow `coercion::rendered_none_or_default_projection` helper, so typed
`None::<T>` recovery is shared with the existing `String.join` null-item path
without opening lower to broad rendered-expression recovery. The constructor
self-temporary detector now recovers parseable raw expressions and simple raw
expression/return/let statements before falling back to token scanning for
unrecovered compatibility text. Regressions:
`lower::tests::none_or_default_detection_uses_structured_raw_recovery_only` and
`coercion::tests::constructor_self_temporary_raw_fallback_requires_identifier_token`.

Follow-up 2026-08-29: stream-lambda raw static-member recovery is still kept
narrow to the parseable `Owner.member` compatibility shape, but it no longer
renders the owner from raw text alone. The accepted raw owner now resolves
through the same `StreamRenderContext` alias/facts path as structured field
access before emitting `Owner::member`. Regression:
`tests/slot_shape_design.rs::stream_pipeline_chain_uses_string_and_optional_shapes`.

Follow-up 2026-08-29: the same stream-lambda raw recovery now accepts the
qualified-owner compatibility shape `package.Type.member` by splitting at the
last dot and validating the owner path segment-by-segment. Parsed stream field
access also preserves the full qualified owner before emitting the associated
path. `a.Item.KIND` and raw `"a.Item.KIND"` therefore lower through structured
`AItem::KIND` instead of collapsing to `Item::KIND` or staying opaque raw text.
Regression:
`tests/slot_shape_design.rs::stream_lambda_emission_uses_structured_operator_rendering`.

Follow-up 2026-08-29: erased-object clone recovery no longer strips a leading
`&` from raw text before projection. Raw inputs now recover through the central
rendered-expression path first; complete structured borrows stay borrowed, while
malformed borrow-looking text stays opaque and is grouped by the ordinary borrow
adapter. Map-key borrow idempotence also no longer accepts raw alphanumeric text
as local-binding evidence; raw simple locals must recover to `RustExpr::Var`,
and numeric raw text receives a real borrow. Regressions:
`coercion::tests::erased_object_clone_recovery_records_recovery_shape`,
`coercion::tests::map_key_borrow_normalizes_raw_local_before_local_identity_checks`,
and `slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

Follow-up 2026-08-29: central rendered binary recovery now understands plain
top-level `<` and `>` comparisons without splitting generic arguments such as
`Owner::VALUE::<Vec<T>>`. Top-level `+` recovery also refuses to steal an outer
lower-precedence comparison/logical expression, so `left == right + 1` recovers
as equality with nested addition rather than `(left == right) + 1`. Regressions:
`coercion::tests::rendered_if_projection_requires_complete_structured_expression`
and
`coercion::tests::rendered_projection_recovers_borrowed_leaf_expressions`.

Follow-up 2026-08-29: Java display receiver cleanup no longer decides whether a
raw `.to_string()` receiver is arithmetic by scanning rendered text for
top-level operators. Raw receivers recover through the central
`rendered_plus_expr_projection_or_raw_leaf` path first, then reuse the
structured arithmetic display receiver logic; unrecoverable raw text stays
opaque. The old arithmetic-only scanner was deleted, leaving only the binary
scanner still needed by the compound cast fallback. Guarded by
`slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`
and the existing display-operand regressions.

Follow-up 2026-08-29: borrowed string-view projection application no longer
strips a leading `*` from raw text before reconstructing an expression. The
`AlreadyStringView` path now recovers a complete deref string-view expression,
verifies that the inner recovered expression is structurally a string view, and
otherwise preserves the opaque raw leaf unchanged. Regressions:
`type_coercion::borrowed_string_view_projection_owns_rendered_view_compatibility`
and `slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

Follow-up 2026-08-29: generic raw method-receiver parenthesization now recovers
parseable raw text into `RustExpr` before deciding whether grouping is needed.
Recovered binary, cast, control-flow, block, closure, try, and deref receivers
are grouped structurally; unrecoverable raw text is grouped only for the legacy
syntax-safety cases and still goes through `RustExpr::raw_leaf`. Regressions:
`coercion::tests::raw_method_receiver_parenthesization_prefers_recovered_structure`
and `slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

Follow-up 2026-08-29: `to_owned_sequence_projection` no longer passes an
optional rendered receiver string into its `.to_vec()` receiver grouping
predicate. Raw inputs still recover first through the existing rendered
projection path; grouping then depends on the recovered `RustExpr` shape, and
unrecovered raw falls back through `parenthesize_raw_method_receiver` instead of
checking for an `as` token in a second string channel. Regressions:
`coercion::tests::to_owned_sequence_projection_keeps_proven_recovery_structured`
and `slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.

Follow-up 2026-08-29: clone receiver grouping no longer uses separate rendered
string predicates for direct clone and audited clone recovery. Raw clone
receivers now go through the shared recovery-first method receiver helper, and
audited clone recovery recovers parseable raw operands before applying its
structural `Some(...)`/`.into()` arms. The dead broad `as` token scanner was
deleted. Regressions:
`coercion::tests::clone_recovery_parenthesizes_only_receiver_level_casts`,
`coercion::tests::audited_clone_recovery_preserves_into_suffix`,
`coercion::tests::audited_clone_recovery_for_constructed_optional_clones_element`,
and `slot_shape_design::rendered_string_heuristics_have_centralization_guardrails`.
