# 0043 — Rust-keyword escaping still needs collision-aware renaming

Status: **fixed** — keyword policy is centralized, and field/static/callable
bindings now allocate emitted names from preserved source identity.

Recorded 2026-08-21.

Codex follow-up 2026-08-21: validated. The silent `type`/`type_` field collapse
is fixed by removing `type`, `match` and `loop` from `parser::to_snake_case`'s
lossy rename arm, leaving only `crate` and `self` there. Emission now renders
those three as raw identifiers (`r#type`, `r#match`, `r#loop`), with regression
coverage for both local `match` and the `type`/`type_` field collision. The
larger audit remains open: keyword policy is still duplicated across parser,
emit, stdlib and coercion, and non-raw names such as `self`/`crate` still need
collision-aware renaming.

Codex follow-up 2026-08-22: the duplicated keyword tables are now centralized in
`java_naming::rust_keyword_escape`. Emit and stdlib both use the shared rawable
keyword predicate; coercion uses the same policy and now renames `self`/`crate`
style field projections instead of emitting invalid `r#self`/`r#crate`; and
`parser::to_snake_case` derives its lossy fallback from the shared rename policy.
The audit remains open for the remaining silent collision class: Java names that
must be renamed (`self`, `crate`, `Self`, `super`) can still collide with an
already-present suffixed spelling unless allocation is done with scope context.

Codex follow-up 2026-09-10: `parser::to_snake_case`,
`parser::to_snake_case_unescaped`, and the centralized `java_naming`
keyword-policy helpers are crate-local. They remain shared internal
compatibility/name-normalization primitives, but are no longer exported as
standalone public API.

Codex follow-up 2026-08-22, second pass: the non-rawable keyword fallback now
uses a double-underscore spelling (`self__`, `crate__`, etc.) through
`java_naming::rename_non_rawable_rust_keyword`, and
`non_rawable_rust_keyword_and_underscore_field_names_do_not_collapse` covers the
previous `self`/`self_` silent collision. This is still not a full allocator:
`self` can theoretically collide with a source `self__`, so scope-aware
allocation remains the final cleanup.

Codex follow-up 2026-08-22, parallel audit: still valid. The remaining loss
comes from `parser::to_snake_case` applying the fixed non-rawable rename before
field, local and parameter scopes can allocate a unique spelling; once `self`
and `self__` have collapsed, later lowering cannot recover the original field.
The smallest useful implementation slice is instance fields only: preserve a
non-escaped field key, allocate emitted field names per type, and route struct
fields, constructor literals/deferred field writes, and `this.field` projections
through that map. Locals/params/lambdas should remain a later slice because
they flow through signature and liveness planning.

Codex follow-up 2026-08-22, instance-field slice: source instance-field keys now
use `parser::to_snake_case_unescaped`, while emitted Rust field names are
allocated per type by `java_naming::source_instance_field_rust_names`. Struct
field emission, constructor struct literals/deferred self-field writes, and
explicit `this.field` reads route through that allocator. The regression now
covers `self` plus `self__`, forcing `self` to render as `self___` instead of
silently collapsing. Remaining open scope at that point: locals, params, lambda
params, bare field names, and static fields still needed scope-aware allocation.

Codex follow-up 2026-08-22, bare-field audit: a `ShapeEnv` access-name map is
not enough for bare source fields. By the time lowering sees `Expr::Var`, Java
bare identifiers have already gone through `to_snake_case`, so bare `self` and
bare `self__` both arrive as `self__`. The failed experiment confirmed explicit
`this.self` is fixed, but bare `self + self__` still cannot be distinguished
without preserving source identifier spelling earlier in parsing/binding. The
next real slice is parser/binding-level identity preservation, not another
lowering-only map.

Codex follow-up 2026-08-22, static-field slice: qualified static fields now use
the same split between semantic keys and allocated emitted names. Static
storage accessors, backing storage symbols, associated const names, and
qualified static reads/writes use `java_naming::source_static_field_rust_name`
or the per-type static field map, while shape/fact lookup keeps the Java field
identity. Regression coverage pins both `static int self; static int self__;`
and `static final int self; static final int self__;`, including generated-crate
compilation. Remaining open scope is locals, parameters, lambda parameters and
bare field names, all of which require parser/binding-level source identifier
preservation.

Codex follow-up 2026-08-22, bare instance-field slice: bare identifier
expressions now preserve the unescaped converted spelling, and `ShapeEnv` keeps
an instance-field access-name map seeded from the same per-type allocator used by
struct field emission. `lower_bare_name` now projects bare instance fields
through that map after exact local shadowing, but before compatibility aliases,
so `self` and `self__` stay distinct even when referenced without `this.`. Alias
entries are compatibility-only: semantic field entries win regardless of
declaration order. Regression coverage pins bare reads, bare constructor writes,
reverse declaration order, and the `self__` local-shadowing boundary. Remaining
open scope is local, parameter and lambda-parameter allocation; a Java parameter
named `self` still needs source binding identity rather than the compatibility
alias path.

Codex follow-up 2026-08-22, callable-binding audit: the remaining scope should
not be fixed as a lowering-only keyword escape. Params, locals, enhanced-for
variables, and lambda params are still normalized in the parser before lowering
can distinguish `self` from `self__`; the fix needs semantic binding identity
preserved through the IR plus a callable-scope emitted-name allocator. The
emission boundaries are method/constructor parameter assembly, local `let`
plans, enhanced-for and `forEach` loop patterns, general closure params, and the
stream-lambda lowering path. `RustStmt::For.pattern` and closure patterns are raw
strings today, so the allocator has to run before `mut`/pattern strings are
assembled.

Codex follow-up 2026-08-22, callable source-identity slice: params, local
declarations, enhanced-for variables, and lambda params now carry an inert
source-name field in the Java IR (`source_name` / `source_var_name`) populated
from `to_snake_case_unescaped`. The existing `name` / `var_name` fields remain
unchanged, so semantic analysis, liveness, signature solving, and lowering keep
current behavior while the lost `self` versus `self__` distinction is available
for the later callable-scope emitted-name allocator. Parser tests pin method
params, local declarations, enhanced-for variables, and lambda params.

Codex follow-up 2026-08-22, formal-parameter allocator slice: ordinary method
and constructor parameters now consume the preserved source names through one
shared `source_binding_rust_names` allocator. `item_assembly` uses that allocator
when rendering parameter declarations, `ShapeEnv` records the same emitted names
for parameter reads, and `lower_bare_name` consults those names before the legacy
snake-case fallback. Regression tests pin `int self, int self__` for both method
and constructor parameters and compile the generated Rust. Remaining callable
scope is local `let` declarations, enhanced-for / stream loop patterns, and
lambda closure params.

Codex follow-up 2026-08-22, callable binding allocator slice: local
declarations, enhanced-for binders, ordinary callback lambda params, and
`forEach` loop-lowered lambda params now use the same preserved source names and
shared emitted-name allocator. Blocks pre-seed direct local/loop binders so
`self` cannot claim `self__` when a user-written `self__` exists in the same
block; compatibility aliases no longer overwrite pre-seeded real source
bindings. Regression tests pin and compile `self` / `self__` collisions for
locals, enhanced-for, and JDK functional callback lambdas. Remaining callable
scope is stream pipeline plans, which still store normalized lambda parameter
names before `stream_lowering`.

Codex follow-up 2026-08-22, stream pipeline slice: `LambdaPlan` and
`ReduceLambdaPlan` now keep both the source binding name and the allocated Rust
binding name. Stream map, match, collect-to-map, flat-map, nested stream-chain
and reduce rendering use the emitted name for closure binders and direct
lambda-parameter values, while body matching still uses the source name. The
receiver helper was also fixed to return `RustExpr::Var(param_rust.to_string())`
instead of re-running lossy snake-case conversion for receiver-shaped lambdas.
Regression tests pin and compile stream map and reduce lambdas where `self` and
`self__` would otherwise collide.

Codex follow-up 2026-08-22, stream block-local closure: the remaining stream
block-lambda gap is fixed. Stream plans allocate lambda parameters together with
direct block-local declarations, and stream block lowering pre-seeds the scoped
environment with those emitted names before lowering `let` statements,
initializers and assignment bodies. Compatibility aliases for parsed fallback
names no longer overwrite real source bindings, so Java locals `self` and
`self__` render as distinct Rust locals instead of duplicate `self___` plus an
undefined `self__`. Regression tests pin and compile both parameter/local and
local/local collisions.

Severity: **medium** — live defect, and the worst case is *silent*: it compiles and
computes the wrong answer
Kind: non-rawable Rust keywords still need full scope-aware collision allocation

## Symptom

```java
public class Kw5 {
    private int type;
    private int type_;
    public Kw5() { this.type = 1; this.type_ = 2; }
    public int sum() { return this.type + this.type_; }
}
```

translates to

```rust
pub struct Kw5 { type_: i32 }

impl Kw5 {
    pub fn new() -> Kw5 { return Kw5 { type_: 1 }; }
    pub fn sum(&self) -> i32 { return (self.type_).wrapping_add(self.type_); }
}
```

`rustc --edition 2021 --crate-type lib` — **zero errors**. `sum()` returns `2`. Java
returns `3`. One of the two fields was deleted and every read of it silently retargeted
to the other. This is exactly [0018](0018-the-compile-error-metric-cannot-see-silent-failures.md)'s
blind spot: the compile-error metric sees nothing.

Locals are worse, because Rust shadowing hides it too:

```java
int type = 1; int type_ = 2; return type + type_;
```
```rust
let type_: i32 = 1;
let type_: i32 = 2;
return (type_).wrapping_add(type_);   // 4, not 3 — compiles clean
```

Methods at least fail loudly:

```java
public int type() { return this.type; }
public int type_() { return this.type_; }
```
```
error[E0592]: duplicate definitions with name `type_`
```

The transpiler already knows how to escape a keyword without renaming it. In the *same
struct*, it does:

```rust
pub struct Kw3 {
    type_: Option<String>,     // Java field `type`   — renamed
    match_: i32,               // Java field `match`  — renamed
    loop_: i32,                // Java field `loop`   — renamed
    r#move: Option<Vec<String>>,  // Java field `move` — raw identifier, lossless
}
```

`move` survives as `r#move` and can never collide. `type` does not, for no reason
visible at the call site.

## Original four lists

| # | symbol | file:line | entries | strategy |
|---|---|---|---:|---|
| P | `to_snake_case` tail match | `src/parser.rs` | **5** | append `_` (**lossy rename**) |
| E | `is_raw_identifier_keyword` | `src/emit.rs` | 47 | `r#` prefix (lossless) |
| S | `is_rust_raw_identifier_keyword` | `src/stdlib.rs` | 47 | `r#` prefix |
| C | `rust_projection_ident_needs_raw` | `src/coercion.rs` | 38 | `r#` prefix |

2026-08-22 status: E, S and C have been removed as separate tables. Their shared
answer now lives in `src/java_naming.rs`. P still exists as a lossy fallback, but
only for names classified as `RustKeywordEscape::Rename`.

Measured membership:

```
P                    = {type, match, loop, crate, self}
E == S               : true  (47 entries, byte-identical sets)
E \ C  = {abstract, become, box, do, final, macro, override, priv, try,
          typeof, unsized, virtual, yield}          # the reserved keywords
C \ E  = {Self, crate, self, super}
P ∩ E  = {type, match, loop}                        # ← the defect
P \ E  = {crate, self}
```

### P ∩ E is the bug

`type`, `match` and `loop` are in **both** P and E. P runs first (at name-conversion
time, `parser::to_snake_case`), so it renames them to `type_`/`match_`/`loop_` and E — the
lossless stage that would have produced `r#type` — never sees a keyword. The rename is
not injective: Java `type` and Java `type_` are distinct identifiers that map to the same
Rust name, and nothing checks for the collision.

`crate` and `self` genuinely *cannot* be raw identifiers, so P's strategy is the only
option for them:

```
error: `self` cannot be a raw identifier
error: `crate` cannot be a raw identifier
```

That is why P exists. But it was written as a flat five-name list rather than as "the
keywords for which `r#` is illegal", so three names that `r#` handles perfectly got
swept into the lossy path with them.

### C was a third opinion that was wrong in both directions

The removed `rust_projection_ident_needs_raw` table omitted all thirteen
*reserved* keywords that E and S carried — so a field named `box` or `override`
reaching `constructor_self_field_projection` was left unescaped — and added
`self`/`Self`/`crate`/`super`, for which `r#` is a hard rustc error. The fixed
path now asks `java_naming::rust_keyword_escape`, so rawable keywords get `r#`
and non-rawable path keywords get a suffixed spelling.

## Root cause

Originally there was no single answer to "what is a Rust keyword, and how do we
spell a Java identifier that is one?" There were four lists and two strategies,
and the strategy was picked by *which list fired first in the pipeline* rather
than by a property of the keyword. The shared table is fixed now, but one design
problem remains: lossy renames still happen before the target scope can prove the
new name is unused. Concretely:

- P is inside `to_snake_case`, a **name-conversion** function that is also used to build
  *derived* names — `parser::type_suffix` (`src/parser.rs:3416`) composes overload
  suffixes like `format!("{}_array", …)` out of its result. So P cannot simply switch to
  `r#`: a composed `r#type_array` is not a valid identifier. Escaping and case conversion
  are two different jobs sharing one function, which is why the escape had to be the
  lossy one.
- E, S and C used to be separate tables. They now delegate to
  `java_naming::rust_keyword_escape`, with tests pinning the rawable-vs-renamed
  split.

## Why it was not noticed

A Java field named `type` is ubiquitous and works fine — `type_` is a perfectly good Rust
name. The defect needs *both* `type` and `type_` (or `match`/`match_`, `loop`/`loop_`) in
the same scope, which is rare enough that no fixture has it, and when it happens the
output **compiles**, so no gate, no `cargo_check_generated_modules` call and no
compile-error count can see it. Nothing in `tests/` asserts anything about keyword
escaping at all; the behaviour is only visible by reading the emitted struct.

The E/S/C divergence is invisible for the opposite reason: emit is the last stage and
re-escapes, so an upstream list can be wrong without consequence — until something
consumes an unescaped name for a purpose other than emission.

## Fixed direction

The shared table is done. The collision-checked allocation slices below are now
covered by source guards and generated-crate regression tests:

1. **Split escaping out of `to_snake_case`.** `to_snake_case` should do case conversion
   only, so that `type_suffix` and the other name-composers keep getting a plain
   `"type"` to concatenate, and escaping happens once, at the point an identifier is
   emitted. That is what makes `Raw` usable for `type`/`match`/`loop` and closes the
   rawable-keyword collision by construction — `r#type` and `type_` are distinct
   spellings of distinct Java names. This is achieved for the rawable keyword
   collision class that triggered the audit.
2. **`Rename` should become collision-checked rather than fixed-spelling.**
   `crate`/`self` start from `crate__`/`self__`, but fields, static fields,
   params, locals, enhanced-for binders, ordinary callback lambdas and stream
   lambda params/block locals now allocate against the names in scope and can
   pick `self___` when `self__` is taken.

Regression fixtures now cover:

- `self`/`self__` instance fields, bare instance-field reads, static fields,
  method and constructor params, locals, enhanced-for vars, ordinary callback
  lambdas, stream expression lambdas, stream reduce lambdas and stream
  block-local collisions, all with generated-crate checks where applicable.

Remaining cleanup, not part of the fixed correctness defect: a generated
keyword-table fixture would make the coverage more systematic than hand-written
cases.

Codex follow-up 2026-09-15: bare constructor assignments to non-rawable keyword
fields now render through the owning `TypeDecl`'s allocated field-name table
instead of rejoining through an owner string and falling back to one-name
allocation. This fixes the current generated-Rust failure where fields
`self`/`self__` were declared as `self___`/`self__` but a bare `self = 1`
initializer emitted `self__: 1`, colliding with the real `self__` field. Guards:
`idiom_gaps::bare_non_rawable_keyword_field_names_do_not_collapse`,
`idiom_gaps::bare_non_rawable_keyword_field_alias_does_not_overwrite_semantic_field`,
`idiom_gaps::bare_non_rawable_keyword_field_preserves_local_shadowing`, and the
`java_naming` keyword-collision unit tests.

## Checked and clean, for the record

Two nearby things that look like the same shape and are not:

- `parser::parse_primitive_type_node` (`src/parser.rs:2474`) lists seven primitives and
  omits `boolean`, while `parse_type_text` (`src/parser.rs:2719`) lists all eight. Not a
  defect: `boolean` arrives as a distinct tree-sitter node kind (`boolean_type`) handled
  at `src/parser.rs:2451`, and `parse_primitive_type_node` is only reached for
  `integral_type | floating_point_type`.
- `PrimitiveType → Rust type name` is written out five times —
  `operator_lowering::rust_type_for_java_primitive` (`src/operator_lowering.rs:99`),
  `representation_lowering::rust_type_for_primitive` (`src/representation_lowering.rs:329`),
  `coercion::rust_primitive_type_name_policy` (`src/coercion.rs:11388`),
  `conversion_plan::boxed_primitive_runtime_rust_type` (`src/conversion_plan.rs:1181`),
  `lower::primitive_rust_cast_type` (`src/lower.rs:2059`). All five **agree**, on all
  eight primitives, today. It is five copies of an eight-row total function over a
  closed enum, so the exhaustiveness check makes an *omission* impossible and only a
  wrong *value* could drift — a much smaller hazard than the open-ended string lists
  above. Worth collapsing when one of those files is touched anyway; not worth a change
  on its own.
