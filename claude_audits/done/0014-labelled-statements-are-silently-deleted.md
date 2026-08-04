# 0014 — Labelled statements are silently deleted from the output

Status: **validated** (runtime-confirmed; isolated to the exact trigger)
Severity: **critical** — code disappears with no diagnostic at any level
Kind: missing parser handler + a diagnostic path that does not fire

## Symptom

Any loop carrying a Java label is **removed entirely** from the generated Rust.
No `compile_error!`, no stderr warning, no comment. The output compiles cleanly
and silently does nothing.

```java
public class LabeledContinue {
    public int run(int n) {
        int total = 0;
        outer:
        for (int i = 0; i < n; i++) {
            for (int j = 0; j < n; j++) {
                if (j == 1) continue outer;
                total += 1;
            }
        }
        return total;
    }
}
```

Generated:

```rust
pub fn run(&self, n: i32) -> i32 {
    let t: i32 = 0;
    return t;              // the entire nested loop is gone
}
```

`run(3)` returns `0`; Java returns `3`.

## The trigger is the label itself, not the labelled jump

Isolated with four variants:

| probe | source | loop emitted? |
|---|---|---|
| `L1` | nested loops, **unlabelled** `continue` | **yes** — correct |
| `L2` | label present, **no labelled jump at all** | **no** — deleted |
| `L3` | `break outer` | **no** — deleted |
| `L4` | labelled `while` + `continue outer` | **no** — deleted |

`L2` is the decisive one: a label with no labelled jump anywhere still causes
the whole statement to vanish. So this is not about lowering `break`/`continue`
to a target — it is that a labelled statement is not recognised at all.

## Second confirmed instance: `assert`

The same fallback swallows `assert_statement`:

```java
public int run(int n){ int t=1; assert n > 0 : "must be positive"; t=2; return t; }
```
```rust
pub fn run(&self, n: i32) -> i32 {
    let mut t: i32 = 1;
    t = 2;                    // the assert is gone
    return t;
}
```

Zero stderr, zero `compile_error!`, compiles clean. Java assertions are disabled
unless `-ea` is passed, so the runtime impact is smaller than for labelled loops
— but the mechanism is identical, and a translated program silently loses every
invariant check its author wrote. `debug_assert!` is the natural target.

An exhaustive grammar diff (see `AUDIT-BACKLOG.md`) shows the statement-level
fallback is hit by exactly three node kinds: `labeled_statement`,
`assert_statement`, and `yield_statement`. The first two are confirmed above.

## Root cause

`parse_stmt_node` (`src/parser.rs:1223`) has no `"labeled_statement"` arm.
tree-sitter-java wraps `outer: for (…) {…}` in a `labeled_statement` node, so it
falls to the catch-all:

```rust
_ => Stmt::Unsupported(format!("unsupported statement node: {}", node.kind())),
```

`grep -n 'labeled_statement' src/parser.rs` returns nothing.

The second half of the bug is that `Stmt::Unsupported` then produces **no
diagnostic here**. The machinery exists — `src/unsupported.rs:113` maps
`Stmt::Unsupported(reason)` to `UnsupportedStatementLowering::ParserUnsupported`,
and `unsupported_statement_stmt` emits a `compile_error!` — but nothing in the
generated file mentions it:

```
$ grep -n 'compile_error\|unsupported\|labeled' L2.gen.rs
(no output)
$ wc -c L2.err
0
```

`src/lower.rs:4855` even asserts `unreachable!("unsupported statement kinds
should be handled before statement lowering")`, so something upstream is
filtering `Stmt::Unsupported` out of the statement list rather than converting
it to a diagnostic.

## Why this is the most dangerous finding in this audit

Every other issue either fails to compile or produces a wrong value. This one
**removes code**. A translated program can pass a compile gate, run, and simply
skip whole algorithms. Nothing in the corpus metrics would notice: labelled
loops contribute zero compile errors precisely because they are gone.

Labelled `break`/`continue` is common in exactly the kind of code this project
targets — nested scans over sequences and matrices with early exit.

## Fix

Two independent changes; do **both**, and do the second one first.

1. **Make silent drops impossible.** Find where `Stmt::Unsupported` is filtered
   out before lowering and make it emit `unsupported_statement_stmt` instead.
   Any statement the parser does not understand must become a `compile_error!`
   in the output. A translator may refuse to translate; it must never quietly
   omit. Add a test asserting that a deliberately unsupported construct produces
   a diagnostic.
2. **Add the parser handler.** `"labeled_statement" => parse_labeled_node(…)`,
   carrying the label into `Stmt::For`/`While`/`DoWhile`/`Block`. `java_ir` already
   has `label: Option<String>` on loops (`src/parser.rs:1672` sets `label: None`),
   and `RustStmt::Loop { label, … }`, `RustStmt::Break(Option<String>)` and
   `RustStmt::Continue(Option<String>)` all exist. Rust's `'label: for … { break 'label; }`
   maps directly, so lowering is nearly free once parsing works.

## Regression tests worth adding

```java
outer: for (i…) { for (j…) { if (j==1) continue outer; t+=1; } }   // t == n
outer: for (i…) { for (j…) { if (j==1) break outer;    t+=1; } }   // t == 1
outer: while (…) { … continue outer; }
outer: { if (c) break outer; … }        // labelled block, not a loop
```

Plus a negative test: **any** construct the parser rejects must appear as a
`compile_error!` in the output. That test is what would have caught this class
rather than this instance.

## Related

The `LabeledBreak.java` probe in `repro/` was earlier recorded as "needs
ndarray, inconclusive". That was wrong in an instructive way: it "compiled"
only because its loop had been deleted. The `ndarray` error came from the
`int[][]` parameter, not the loop.
