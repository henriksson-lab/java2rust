# 0021 — `instanceof` pattern variables are never bound

Status: **done for the audited `instanceof` pattern scopes**.
Severity: medium-high — always a compile error when the pattern variable is used
Kind: missing pattern model

## Symptom

Java 16 pattern matching for `instanceof` binds a variable:

```java
public class P5 {
    public int run(Object o) {
        int t = 1;
        if (o instanceof String s) { t = s.length(); }
        return t;
    }
}
```

The type test is translated, but the binding is dropped:

```rust
pub fn run(&self, o: Box<dyn std::any::Any>) -> i32 {
    let mut t: i32 = 1;
    if o.is::<String>() {
        t = s.length();          // `s` was never introduced
    }
    return t;
}
```

```
error[E0425]: cannot find value `s` in this scope
```

## Root cause

The grammar node `type_pattern` (and the Java 21 `record_pattern`,
`record_pattern_component`, `underscore_pattern`, `guard`) is never referenced
in `src/parser.rs`. The `instanceof_expression` handler reads only the `left`
and `right` fields:

```rust
"instanceof_expression" => Expr::InstanceOf {
    expr: … node.child_by_field_name("left") …,
    ty:   … node.child_by_field_name("right") …,
}
```

When `right` is a `type_pattern` rather than a bare type, `parse_type_node`
takes the node's **source text** (`"String s"`) and hands it to the text-based
type parser. The identifier is discarded and no binding is recorded — the IR has
no place to put one.

## Fix

1. Add an optional binding to the IR: `Expr::InstanceOf { expr, ty, binding: Option<String> }`.
2. Parse `type_pattern` in the `instanceof_expression` handler, taking the type
   and the name separately from the node's fields rather than from text.
3. Lower to a Rust downcast that introduces the binding. The natural shape is
   `if let Some(s) = o.downcast_ref::<String>() { … }`, which also removes the
   separate `is::<T>()` + later downcast that the current shape would need.
   Note the scope rule: Java's binding is in scope wherever the pattern is
   *definitely matched*, including `if (!(o instanceof String s)) return; … s …`
   and `&&` chains — a straight `if let` covers the common case but not those.
   Start with the common case and emit a diagnostic for the rest.

## Related unhandled pattern constructs

Same family, all unreferenced in the parser, none yet probed:

- `record_pattern`, `record_pattern_body`, `record_pattern_component` — Java 21
  record deconstruction
- `underscore_pattern` — `_` in patterns
- `guard` — `case X when cond ->`
- `switch_rule` — arrow-form switch cases

Given [0019](0019-unhandled-expressions-emit-raw-java-source.md) and
[0014](0014-labelled-statements-are-silently-deleted.md), expect these to either
inject raw Java or vanish silently rather than produce a diagnostic. Worth one
probe batch to determine which, and to confirm they are not silently dropped.

## Regression tests worth adding

```java
if (o instanceof String s) { return s.length(); }           // basic
if (!(o instanceof String s)) return -1; return s.length(); // negated scope
if (o instanceof String s && s.length() > 2) { … }          // && chain
```

---

## Update 2026-08-03 — closed

Status: **fixed**. Regression test: `tests/differential/Patterns.java`, which
runs both toolchains and compares stdout (`3|3|3|3|true|true|3`).

### The status was wrong in both directions before this pass

`type_pattern` has **0 references in `parser.rs`**, which reads as "unfixed"; in
fact `Expr::InstanceOf` already carried `binding: Option<String>` and the handler
already read the node's `name` field — tree-sitter does not produce a
`type_pattern` node here, so that grep was measuring the wrong thing.
`claude_audits/NEXT-BATCH.md` meanwhile recorded 0021 as *already closed*.

Both were wrong, and only running the probes settled it. What had been
implemented was "an `instanceof` that **is** an `if` condition"; every other
position where JLS 6.3.1 puts the binding in scope dropped it.

### What was broken, confirmed with `rustc`

| shape | before |
|---|---|
| `if (o instanceof T s)` | correct |
| `if (!(o instanceof T s)) return; … s …` | correct |
| `if (o instanceof T s && p(s))` | correct |
| `(o instanceof T s) ? a(s) : b` | binding dropped; `a(s)` emitted as **raw Java** |
| `o instanceof T s && p(s)` in value position | binding dropped → `E0425` |
| `!(o instanceof T s) \|\| p(s)` | binding dropped → `E0425` |
| `while (o instanceof T s)` | binding dropped → raw Java in the body |

### The fix

`lower_instanceof_pattern_value_expr` and `lower_instanceof_pattern_while_stmt`
(`src/lower.rs`) reuse the `plan_instanceof` /
`instanceof_pattern_downcast_ref_projection` / `instanceof_pattern_binding_shape`
machinery the statement path already had:

| Java | Rust |
|---|---|
| `o instanceof T s && p(s)` | `o.downcast_ref::<T>().is_some_and(\|s\| p(s))` |
| `!(o instanceof T s) \|\| p(s)` | `o.downcast_ref::<T>().is_none_or(\|s\| p(s))` |
| `(o instanceof T s) ? a(s) : b` | `match o.downcast_ref::<T>() { Some(s) => a(s), None => b }` |
| `while (o instanceof T s) { … }` | `while let Some(s) = o.downcast_ref::<T>() { … }` |

**Two hooks were needed, not one.** `lower_expr_for_slot_with_refinements` has
its *own* ternary path, so the value-position hook never sees a ternary that
reaches a typed slot — which is every `return cond ? a : b`. That is the same
"two paths for one construct" shape
[0006](0006-move-analysis-is-five-approximations-of-one-relation.md) and
[0010](0010-fallibility-is-decided-three-times-and-disagrees.md) are about, and
it is why the first attempt fixed the `&&` form but not the ternary.

### One case removed from scope, because it is not legal Java

```java
boolean ok = o instanceof String s;
return ok ? s.length() : -1;      // javac: cannot find symbol: variable s
```

The fixture originally included this and asserted a diagnostic was needed. `javac`
rejects it: the binding is in scope only where the pattern is *definitely
matched*. The translator accepts it, which is lenience toward invalid input, not
a defect. Worth recording, because the original issue text listed the negated
form as the hard case and that one already worked.

### Related pattern constructs, re-probed

`record_pattern` and `case … when` guards emit `compile_error!` — loud, which is
the required behaviour, so no action. The adjacent record-component lowering bug
found during this probe is fixed as of 2026-09-09: component accessors now return
their backing fields, and canonical constructors assign the component parameters
after any compact-constructor body. `RecordCanonicalConstructor` pins the Java
oracle.

### Follow-up 2026-09-09 — guarded ternary true-branch scope

The value-position ternary repair handled `(o instanceof String s) ? a(s) : b`,
but not `(o instanceof String s && guard(s)) ? a(s) : b`. The ordinary ternary
path lowered the condition as `is_some_and(|s| guard(s))` and then lowered the
true arm without the pattern binding, producing `s::...` in generated Rust.

Both the value and slot-aware pattern ternary paths now split the condition into
the binding pattern and an optional `&&` guard. The true arm lowers with `s` in
scope, and the guard is checked inside the `Some(s)` arm without re-evaluating
the original `instanceof` receiver. Regression:
`tests/differential/InstanceofPatternExpression.java`
(`4,-1,true,false,bcd,no`).
