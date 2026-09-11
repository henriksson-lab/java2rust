# 0036 — Type inference is exponential in the length of a `+` chain

Status: **fixed** (found and closed 2026-08-04)
Severity: high — the translator does not finish on a common idiom
Kind: algorithmic; a node rebuilt instead of inspected

## Symptom

Translating a string concatenation with about a dozen segments takes minutes.
Measured on one class whose only content is `return a + "|" + b + …`:

| segments | time |
|---:|---:|
| 7 | 3.6 s |
| 9 | 25.8 s |
| 11 | **> 120 s** (killed) |

Roughly 2.8× per added segment. The ~3 s floor is fixed start-up cost (external
pack loading), so the growth above it is the defect.

A `toString()` that concatenates a dozen fields is ordinary Java. On a real
corpus this is the difference between a file translating and a run appearing to
hang — the same class of problem as
[0035](0035-idiom-gaps-suite-never-terminates.md), where "slow" and "broken"
were indistinguishable from outside.

## Root cause

`infer_binary_type` (`src/types.rs`) answered "is this `+` a string
concatenation?" by **rebuilding the node it was already holding**:

```rust
BinaryOp::Add
    if crate::string_concat::is_string_concat(
        &Expr::Binary {
            op,
            left: Box::new(left.clone()),    // deep-clones the whole left subtree
            right: Box::new(right.clone()),
        },
        env,
    ) => JavaType::String,
```

`is_string_concat` then flattens that reconstructed node, which calls
`infer_expr_type` on the operands, which re-enters `infer_binary_type` on the
left subtree, which clones and rebuilds *again*. A `+` chain is left-nested, so
for depth *n* this is

```
T(n) = 2·T(n−1) + O(n)      →      exponential
```

with a deep `Expr` clone at every level. The profile is dominated by `malloc`,
`_int_free` and `Expr::clone`, which is the signature of the same subtree being
copied over and over rather than of real work.

## Fix

Ask about the operands directly instead of reconstructing the node:

```rust
BinaryOp::Add
    if crate::string_concat::binary_add_operands_make_string_concat(left, right, env) =>
    JavaType::String,
```

`binary_add_operands_make_string_concat` was already the function
`is_string_concat` bottoms out in. Follow-up 2026-09-10 narrowed it and the
string-concat analysis/plan/render pipeline to crate-local callers; the direct
pipeline assertions now live beside the owning modules.

**The two are equivalent for a `Binary { Add, left, right }` node.**
`is_string_concat` is `parts.len() > 1 && binary_add_is_string_concat(expr)`,
and when the operand check passes, `flatten_string_concat` descends into *both*
sides and so always produces more than one part. The `parts.len() > 1` guard can
therefore never be the deciding factor for a node of this shape, which is why
dropping the flatten is not a behaviour change.

Result: exponential → flat.

| segments | before | after |
|---:|---:|---:|
| 9 | 25.8 s | 3.0 s |
| 11 | > 120 s | 3.0 s |
| 40 | — | 3.0 s |

At 40 segments the time is still the fixed start-up cost, so the concatenation
itself has become free.

## How it was found

Not by looking for it. A ten-segment probe written to measure
[0031](../0031-stubbed-constant-census.md)'s constant values would not finish, and
`Tiny.java` and `ObjectArray.java` translated in 3 s — so the input, not the
machine, was the variable. Bisecting on segment count gave the growth curve in
two minutes, and `perf` pointed at the allocator rather than at any analysis.

Confirmed **pre-existing** by stash-and-compare against HEAD before changing
anything, so it is not a regression from the round-5 work.

## Guard

`tests/differential/Constants.java` now returns a ten-segment concatenation, so
a return of this defect shows up as the differential suite hanging rather than
as a silent slowdown. That is a weak guard — it detects, it does not bound — and
a real one would be a translation-time budget in the harness. Worth adding if
this class recurs.

## The general shape

This is the input-side twin of
[0007](../0007-rust-ir-is-mostly-opaque-strings.md)'s complaint. There, decisions
are made by re-rendering a structure to text and inspecting the text; here, a
decision was made by re-*building* a structure that was already in hand. Both
are "reconstruct what you already have, then re-derive from it", and both cost
more than the answer is worth.

Grepped for other `Expr::Binary { .. Box::new(left.clone()) .. }`
reconstructions: there is exactly one more, `parser.rs:2036`, and it is
legitimate — desugaring `a op= b` genuinely builds a new node rather than asking
a predicate about an existing one. So this was the only instance of the bad
shape.
