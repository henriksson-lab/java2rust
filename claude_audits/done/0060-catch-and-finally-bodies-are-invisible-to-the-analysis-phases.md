# 0060 — `catch` and `finally` bodies are invisible to the analysis phases

Status: **fixed** — `usage::analyze_stmt` now visits `catch` and `finally`
bodies, with catch parameters in scope; signature local-name counting, local
constraint collection and local-plan collection now visit `catch` and `finally`
bodies as well.
Fixed: 2026-08-24
Verified: `cargo test --release --test idiom_gaps catch_and_finally_field_mutations_make_receiver_mutable`;
`cargo test --release --test idiom_gaps catch_self_call_mutation_makes_receiver_mutable`;
`cargo test --release --test idiom_gaps catch_and_finally_locals_receive_signature_plans`;
`cargo test --release --test idiom_gaps simple_try_catch_return_generated_rust_compiles`;
`cargo test --release --test idiom_gaps simple_try_finally_cleanup_generated_rust_compiles`;
`cargo test --release --test idiom_gaps jaligner_try_catch_finally_cleanup_gets_combined_exception_plan`;
`cargo check --release`

Follow-up 2026-09-06: the adjacent final-statement `try/catch` unit-`Result`
tail gap from the 2026-09-02 recheck is fixed. A unit-returning method promoted
to `Result<(), JavaException>` now appends `return Ok(());` when its final
`try/catch` can fall through via a catch body. Regression:
`idiom_gaps::final_try_catch_unit_result_method_gets_ok_tail`.

Original status: **open**

Dimension: phase architecture / information lost between passes.

## Symptom

A statement that appears inside a `catch` clause or a `finally` block is
lowered and emitted, but it does not exist as far as usage analysis, local
signature planning, or the lowering type/shape environment are concerned. The
emitted Rust is therefore planned as if the catch body were empty, and then the
catch body is emitted into it anyway.

Three separate wrong outputs, all silent (exit code 0, zero diagnostics):

1. A method that mutates a field only inside a `catch`/`finally` is emitted
   `&self` and then assigns through it — hard compile error.
2. A method that calls a `&mut self` method only inside a `catch` is emitted
   `&self` — hard compile error.
3. A local declared inside a `catch`/`finally` is given the Rust type of an
   *unrelated, out-of-scope* local of the same name from the `try` body — hard
   compile error, plus mistranslated calls on that local.

`try`/`catch` is not an exotic corner of Java: 21 of the 196 files in
`examples/` contain a `catch` clause, and mutating state in a catch handler
(setting an error flag, appending to a log list, incrementing a counter) is one
of the most common things a handler does.

## Verified evidence

All repros are plain `javac`-valid Java, run through
`./target/release/new-java2rust <file>.java` at commit `b391645`. Only the
generated `impl` for the input class is quoted; the JDK-facade prelude is
elided.

### 1. Receiver mutability is lost — direct field write

```java
public class N {
    int n;
    List<String> items = new ArrayList<>();

    public void inCatch() {
        try { throw new RuntimeException("x"); }
        catch (RuntimeException e) { n = n + 1; items.add("a"); }
    }

    public void inFinally() {
        try { System.out.println("t"); }
        finally { n = n + 1; items.add("b"); }
    }

    public void plain() { n = n + 1; items.add("c"); }
}
```

emits

```rust
pub fn in_catch(&self) {
    match (|| -> Result<(), JavaException> { ... })() {
        Ok(value) => { return value; },
        Err(e) if e.is_instance(&[...]) => { self.n = (self.n).wrapping_add(1); self.items.push("a".to_string()); },
        Err(ex) => { panic!(...) }
    };
}

pub fn in_finally(&self) {
    { let __java_try_result = (|| -> Result<(), JavaException> { ... })();
      self.n = (self.n).wrapping_add(1); self.items.push("b".to_string()); ... };
}

pub fn plain(&mut self) {
    self.n = (self.n).wrapping_add(1);
    self.items.push("c".to_string());
}
```

`plain` gets `&mut self`. The two identical bodies inside `catch` and `finally`
get `&self` and then assign through it: four `E0594`/`E0596` errors.

### 2. Receiver mutability is lost — through a self-call

```java
public class O {
    int n;
    void mutate() { n = n + 1; }
    public void viaCatch() {
        try { throw new RuntimeException("x"); }
        catch (RuntimeException e) { mutate(); }
    }
    public void viaPlain() { mutate(); }
}
```

emits `pub fn mutate(&mut self)`, `pub fn via_plain(&mut self)`, and
`pub fn via_catch(&self)` whose body calls `self.mutate()`. `usage`'s
`mutates_self` fixpoint over self-calls (`src/usage.rs:282-332`) never sees the
call, because the call site was never recorded.

### 3. A catch-block local is typed from a shadowed local in the `try` body

```java
public class K {
    public String f(Map<String,String> m) {
        try {
            List<String> v = new ArrayList<>();
            v.add("a");
            return v.get(0);
        } catch (RuntimeException e) {
            String v = m.get("k");     // legal Java: the try scope has ended
            return v;
        }
    }

    public String g(Map<String,String> m) {   // identical catch, no shadow
        try { return "ok"; }
        catch (RuntimeException e) { String v = m.get("k"); return v; }
    }
}
```

`g`'s catch arm is correct:

```rust
let _java_borrowed_v_0 = "k".to_string();
let mut v: Option<String> = m.get(&_java_borrowed_v_0).cloned();
return v.expect("Java null receiver");
```

`f`'s catch arm, from the *same Java source lines*, is:

```rust
let _java_borrowed_v_0 = "k".to_string();
let mut v: Vec<String> = m.get(&_java_borrowed_v_0);
return v;
```

A `String` local declared `Vec<String>`, initialised from `Option<&String>`, and
returned from a `-> String` function. The `Vec<String>` is the *try body's*
`List<String> v`, which is out of scope at that point in Java.

The same input with `finally` instead of `catch` (`M.java`) additionally
mistranslates the use:

```rust
let mut v: Vec<String> = m.get(&_java_borrowed_v_0);
println!("{}", java_erased_object_to_string(&v));
```

— `System.out.println(v)` on a `String` becomes the erased-object `Debug`
stub, because the shape environment believes `v` is a collection.

### 4. Every catch/finally local is unconditionally `mut`

```java
public String inCatch(List<String> xs) {
    try { return xs.get(0); }
    catch (RuntimeException e) { String s = xs.get(1); return s; }
}
```
emits `let mut s: Option<String> = ...` where the same declaration in the `try`
body emits `let s: Option<String> = ...`. `mut` here comes from the *fallback*
path, not from analysis (see below).

## Root cause

`java_ir::Stmt::Try` has four child positions
(`src/java_ir.rs:197-202`):

```rust
Try {
    resources: Vec<Stmt>,
    body: Block,
    catches: Vec<CatchClause>,
    finally_block: Option<Block>,
},
```

Six independent statement walkers pattern-match it as
`Stmt::Try { resources, body, .. }` and recurse into only the first two:

| site | function | what it feeds |
| --- | --- | --- |
| `src/usage.rs:1630` | `analyze_stmt` (`src/usage.rs:1190`) | **all of `MethodUsageFacts`**: `bindings`, `call_sites`, `constructor_call_sites`, `field_uses`, `mutates_self` |
| `src/signature.rs:9965` | `collect_declared_local_name_counts` | the duplicate-name set |
| `src/signature.rs:10165` | `collect_local_constraint_sets_from_stmts` | `LocalConstraintSet` per local |
| `src/signature.rs:10366` | `collect_local_plans_from_stmts` | `MethodSignaturePlan::locals` |
| `src/types.rs:120` | `TypeEnv::learn_stmt` | the Java type of each local |
| `src/expr_shape.rs:1259` | `learn_stmt_shapes` | `ShapeEnv` local shapes during lowering |

`analyze_stmt` is the *only* producer of `MethodUsageFacts` for a callable body
(entered at `src/usage.rs:384` for methods and `:434` for constructors, and
recursively from within itself), so item 1 is total: no
binding use, call site, field use or self-mutation inside a handler exists in
the usage facts at all. `signature::receiver_constraints_with_usage`
(`src/signature.rs:13104-13116`) reads `usage.mutates_self` and is the sole
source of `ReceiverConstraint::MutableReceiver` when no external effect fact
applies, which is exactly repros 1 and 2.

Repro 3 is the *interaction* between this drop and the name-based recovery
described in `0061`: the catch local has no `LocalPlan`, so lowering asks
`expr_shape::next_local_plan_for_declared_type`
(`src/expr_shape.rs:706-743`) for one. That function first looks for a
`(name, java_ty)` match; finding none, it falls through to a **name-only**
match (`src/expr_shape.rs:731-742`) and returns the try body's
`List<String> v` plan. A dropped fact therefore does not surface as "no plan"
— it surfaces as "a confidently wrong plan belonging to a different variable".

Repro 4 is the other half of the same recovery: when even the name-only match
fails, lowering calls `conservative_lambda_local_plan`
(`src/lower.rs:10457-10470`), which hardcodes `binding_mutable: true`.

The architectural cause is that **`Stmt` has no canonical child traversal**.
`grep -rho 'Stmt::Try' src/*.rs | wc -l` reports 92 mentions across 25 modules,
and each match arm independently decides which of the four children to visit. Rust's
exhaustiveness check covers *variants*, not *struct-variant fields*, so `..`
elides two whole sub-trees with no warning. The same walker family already had
this bug for lambda bodies —
`claude_audits/done/0033-receiver-mutability-not-propagated-through-lambdas.md`
— and that fix added an arm rather than removing the possibility of a missing
arm, so the next missing sub-tree was inevitable.

## Why it went unnoticed

- The gap is *silent by construction*. Nothing computes "the set of statements
  the analysis visited" and compares it to "the set of statements lowering
  emitted", so a sub-tree that only lowering sees produces output with no
  diagnostic. This is the failure mode
  `0018-the-compile-error-metric-cannot-see-silent-failures.md` predicted.
- The three visible symptoms look like three unrelated bugs (a receiver
  mutability bug, a `mut` bug, a local typing bug), and each is individually
  patchable at its own site, which is what a symptom-level audit would produce.
- Most `catch` bodies in the small fixtures are `return`/`throw`/`println` only,
  which need no usage facts. The failure needs a handler that *mutates* or
  *declares*, which is common in real code and rare in a minimal fixture.
- `examples/` is asserted on but never compiled
  (`claude_audits/0052-...`), so the `E0596` never appears.

## Concrete redesign

**One traversal, in one place.** Move child enumeration into `java_ir` and make
"which children does a `Stmt` have" a question with exactly one answer.

1. Add to `src/java_ir.rs`:

   ```rust
   pub enum StmtChild<'a> { Stmt(&'a Stmt), Block(&'a Block), Expr(&'a Expr) }

   impl Stmt {
       /// Every nested statement, block and expression of this statement, in
       /// source order. The only place in the crate that knows a `Try` has
       /// four sub-trees.
       pub fn children(&self) -> impl Iterator<Item = StmtChild<'_>> { ... }
   }
   ```

   plus a `Stmt::scopes()` for consumers that need declaration scopes rather
   than raw children.

2. Rewrite the six walkers above in terms of `children()`. They keep their own
   per-variant *actions* (a `VarDecl` arm, an `Assign` arm) but lose their own
   *recursion*: the recursive default becomes
   `for child in stmt.children() { self.walk(child) }`.

3. Make the omission unrepresentable in the pattern: change
   `Stmt::Try { .. }` to `Stmt::Try(Box<TryStmt>)` with

   ```rust
   #[non_exhaustive]
   pub struct TryStmt { pub resources: Vec<Stmt>, pub body: Block,
                        pub catches: Vec<CatchClause>, pub finally_block: Option<Block> }
   ```

   `#[non_exhaustive]` on the struct forbids exhaustive construction outside
   the module and, more importantly, makes `TryStmt { resources, body, .. }`
   in a pattern *unremarkable* rather than load-bearing — so the escape hatch
   is the explicit `try_stmt.children()` call, not a `..`.

4. Enforcement while the migration lands: a source guard test in the style of
   `tests/slot_shape_design.rs` (which already uses
   `include_str!("../src/facts.rs")` for exactly this) that forbids the literal
   `Stmt::Try {` pattern outside `src/java_ir.rs` unless it binds all four
   fields. That single test would have caught all six sites today.

**What becomes unrepresentable.** A phase that recurses through statements can
no longer visit *some* of a statement's sub-trees: the recursion is not written
per-consumer, so there is no place to forget one. A `LocalPlan` lookup can no
longer resolve to a different variable, because the plan for the catch local
will exist (see `0061` for removing the name-based fallback itself).

**Migration order.** Step 2 alone fixes all four repros and is mechanical —
each walker's `Stmt::Try` arm becomes the default recursion. Step 4 is one
test. Steps 1 and 3 are the durable part and can follow. Regression fixtures to
pin: `N.java` (`in_catch`/`in_finally` must be `&mut self`), `O.java`
(`via_catch` must be `&mut self`), `K.java` (`f`'s catch `v` must be
`Option<String>`, identical to `g`'s), and the catch-local `mut` case, all four
of which are three-line Java files.

## Related

- `0033` (done) — the same walker family, missing the lambda sub-tree. Fixed by
  adding an arm, not by removing the ability to miss one.
- `0018` (done) — the compile-error metric cannot see this class of failure.
- `0061` (this round) — the name-based recovery that turns "fact missing" into
  "fact belonging to a different variable".
