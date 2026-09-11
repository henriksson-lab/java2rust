# 0004 — String concatenation absorbs nested arithmetic `+`

Status: **validated** (runtime-confirmed wrong output; compiles cleanly)
Severity: high — silent wrong answer, no compile error, common Java idiom
Kind: local rule bug with an exact fix — good first fix, no new machinery needed

## Symptom

`"text " + (a + b)` is translated as a three-part string concatenation
`text, a, b` instead of concatenating `text` with the *sum* of `a` and `b`.

This produces Rust that compiles and runs, and silently returns the wrong
string. There is no diagnostic.

## Minimal repro

```java
public class StrOp {
    public String label(int a, int b) {
        return "total = " + (a + b);
    }
}
```

| | `label(1, 2)` |
|---|---|
| Java | `"total = 3"` |
| emitted Rust | `"total = 12"` |

Verified by compiling and running the emitted file:

```
label(1,2) = "total = 12"
```

## Emitted Rust

```rust
pub fn label(&self, a: i32, b: i32) -> String {
    return format!("total = {}{}", (a).to_string(), (b).to_string());
}
```

## Root cause

`src/string_concat.rs`:

```rust
fn flatten_add<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) {
    if let Expr::Binary { op: BinaryOp::Add, left, right } = expr {
        flatten_add(left, out);
        flatten_add(right, out);
    } else {
        out.push(expr);
    }
}

pub fn analyze_string_concat<'a>(expr: &'a Expr, env: &TypeEnv) -> Option<StringConcatParts<'a>> {
    let mut parts = Vec::new();
    flatten_add(expr, &mut parts);
    if parts.len() > 1
        && parts.iter().any(|part| matches!(infer_expr_type(part, env), JavaType::String))
    { Some(StringConcatParts { parts }) } else { None }
}
```

Follow-up 2026-09-10: the string-concat analysis, plan, and render helpers are
crate-local. Direct pipeline assertions now live beside the owning modules
instead of being imported by integration tests as public API.

`flatten_add` descends through **every** `Add` node in the tree with no regard
for operand types, then the caller asks only whether *some* part is a `String`.

Java does not work that way. JLS 15.18: `+` is left-associative and the
concat-vs-numeric decision is made **per node, bottom-up, from the operand
types**. In `"total = " + (a + b)` the inner node is an `int` addition; only the
outer node is a concatenation.

Why it usually looks fine: for the unparenthesized `"total = " + a + b`, the
parse tree is left-nested `(("total = " + a) + b)`, and flattening happens to
produce the right answer. The bug fires precisely when an `Add` node appears as
the **right** operand of a string `+` — i.e. whenever the source parenthesises
the arithmetic, which is exactly what a Java author writes when they mean
addition.

Common shapes that are currently mistranslated:

```java
"sum: " + (a + b)
"index " + (i + 1) + " of " + n
log("offset=" + (base + delta))
```

## Fix

Only flatten into a child when that child is itself a string concatenation.
`flatten_add` needs the type environment:

```rust
fn flatten_add<'a>(expr: &'a Expr, env: &TypeEnv, out: &mut Vec<&'a Expr>) {
    if let Expr::Binary { op: BinaryOp::Add, left, right } = expr
        && (matches!(infer_expr_type(left, env), JavaType::String)
            || matches!(infer_expr_type(right, env), JavaType::String))
    {
        flatten_add(left, env, out);
        flatten_add(right, env, out);
    } else {
        out.push(expr);
    }
}
```

Check the resulting behaviour on both shapes:

- `"total = " + a + b` → outer node: right is `int`, left is `String` ⇒ concat;
  recurse left, which is also a concat ⇒ parts `["total = ", a, b]`. Unchanged,
  still correct.
- `"total = " + (a + b)` → outer node is a concat; left is `String` leaf, right
  has type `int`, so it is **not** flattened ⇒ parts `["total = ", (a + b)]`
  ⇒ `format!("total = {}", a + b)`. Fixed.

Note the guard must test the *node* (does either operand have type `String`),
not the flattened part list, or the same conflation returns.

## Regression tests worth adding

```java
"total = " + (a + b)          // -> "total = 3"     for a=1,b=2
"total = " + a + b            // -> "total = 12"
"" + (a + b)                  // -> "3"
"x" + (a + b) + "y" + a + b   // -> "x3y12"
s + (a - b)                   // unaffected: '-' never concatenates
```

The last two are the useful ones — they pin both directions at once.

## Interaction with 0003

Once fixed, `a + b` inside the concat becomes a real integer addition and
inherits the overflow question in
[0003](0003-java-integer-arithmetic-semantics-not-preserved.md). The two fixes
are independent but land on the same expression.
