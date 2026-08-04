# 0007 — The Rust IR is mostly opaque strings, so decisions are made by substring search

Status: **validated by inspection** (structural)
Severity: high — this is the mechanism the convergence plan is fighting
Kind: IR modelling gap

## Observation

`src/rust_ir.rs` defines a structured `RustExpr`, but 58% of all constructed
expressions are the escape hatch:

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
frames the string checks as a *habit* to be replaced site by site. They are not
a habit — they are a **forced consequence** of `RustExpr::Raw` being the
dominant constructor. As long as 58% of expressions carry no structure, any new
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
   `OptionalProjected`, `SliceView`, `StringView`, `ResultPropagated`). Make
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

Current baseline: 542 `Raw` / 929 total, 37 round trips, 55 distinct fragments.

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
SliceView | ResultPropagated`) in `src/rust_ir.rs`, with `shaped()`,
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
