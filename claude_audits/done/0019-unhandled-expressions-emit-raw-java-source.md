# 0019 — Unhandled expressions are emitted as raw Java source into the Rust output

Status: **validated** (rustc-confirmed)
Severity: high — Java text leaks into the output; the resulting error names Rust
syntax, not the unsupported construct
Kind: parser fallback

## Symptom

`parse_expr_node` ends with:

```rust
_ => Expr::Raw(text(node, source).to_string()),
```

Any expression node the parser does not handle is copied **verbatim from the
Java source** into the generated Rust. No diagnostic.

## Validated instance: switch expressions

```java
public class P2 { public int run(int v){ int r = switch(v){ case 1 -> 10; case 2 -> 20; default -> 0; }; return r; } }
```

Generated:

```rust
pub fn run(&self, v: i32) -> i32 {
    let r: i32 = switch(v){ case 1 -> 10; case 2 -> 20; default -> 0; };   // raw Java
    return r;
}
```

```
error: expected one of `.`, `;`, `?`, `else`, or an operator, found `{`
```

Same for the colon/`yield` form (`P3`). `switch_expression` **is** handled in
`parse_stmt_node` but not in `parse_expr_node`, so a switch used as a *value*
falls through to the raw fallback.

## Why the fallback shape matters

Compare the two failure modes the parser currently has:

| unhandled | result | diagnostic |
|---|---|---|
| statement | **deleted** ([0014](0014-labelled-statements-are-silently-deleted.md)) | none |
| expression | **raw Java injected** (this issue) | none |

Neither says "unsupported Java construct". The expression case at least fails
loudly, but it fails as a *Rust syntax error* at a position that has nothing to
do with the cause — and a syntax error voids every other diagnostic in that file
(same leverage problem as [0012](0012-unsupported-diagnostics-emit-invalid-rust.md)).

`Expr::Raw` is also used for *deliberate* raw emission elsewhere, so a
parser-fallback raw is indistinguishable from an intentional one. Nothing
downstream can tell "we chose this text" from "we gave up".

## Fix

1. Replace the fallback with an explicit unsupported marker carrying the node
   kind and span, and route it through `plan_unsupported_expression`
   (`src/unsupported.rs:76`) so it becomes a `compile_error!` naming the
   construct. Same principle as 0014: refusing to translate is fine, silently
   emitting Java is not.
2. Handle `switch_expression` in `parse_expr_node`. The statement path already
   parses it, so this is mostly reuse; lowering to a Rust `match` expression is
   direct.
3. Consider making the parser-fallback a distinct variant from `Expr::Raw`
   (e.g. `Expr::Unparsed { kind, text, span }`) so the two cases are never
   conflated.

## Exhaustive scope

Diffing the tree-sitter-java grammar (151 named node types) against every string
literal in `src/parser.rs` gives the complete set of expression-level constructs
that hit this fallback:

- `switch_expression` — **confirmed above**
- `template_expression` / `string_interpolation` — string templates (preview
  feature; low priority, but it will emit raw Java if it ever appears)

The statement-level equivalents are in
[0014](0014-labelled-statements-are-silently-deleted.md). Together these two
issues cover the entire "parser does not understand it" surface, and the list is
now bounded and known — see `AUDIT-BACKLOG.md`.

## Regression test worth adding

```java
int r = switch (v) { case 1 -> 10; default -> 0; };   // must compile and return 10 for v==1
```

Plus a general one: no generated `.rs` file may contain `switch(`, `case `,
`->` outside a closure, or any other Java-only token. A cheap lexical scan of
the output would catch the whole class.
