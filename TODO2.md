# TODO2 — what audits 0054–0065 still owe

> Closure update 2026-09-21: audit 0057 is now fixed/fenced for its audited
> exception/finally matrix and lives under `claude_audits/done/`. The mixed
> plain catch/finally return/fallthrough cell is implemented and Java-oracle
> gated; the analogous resource-bearing cell is a javac-validated, precise loud
> fence until a unified completion/suppression carrier is built. Older 0057
> “still open” notes below are retained as historical recheck evidence.

> **Progress 2026-09-02. The differential gate is green again: 2 passed, 0
> failed.** Fixed and verified against the Java oracle: P0.1's runtime abort
> (`Map.get` into a declared local), P0.2 (the interface-default overload that
> compiled clean and panicked), the doubled-unwrap that failed `LinkedNode` and
> `NullableArrayLengthCatch`, and 0055 finding C (batch diagnostics
> cross-contaminating every file in the batch). P0.3 was **measured and
> deliberately not changed** — see the note in 0057.
>
> Regressions checked by *name*, not count, in an isolated worktree at HEAD:
> `idiom_gaps` **2 fixed, 0 newly broken** (117 vs 119);
> `type_coercion` / `object_model` / `semantic_facts` / `slot_shape_design`
> byte-identical to HEAD at 3 / 2 / 6 / 122; `--lib` green;
> `batch_translation` 21/2, both failures present at HEAD.
>
> `slot_shape_design` needed two expectations rewritten, both recorded rather than
> quietly bumped: the facade `Map.get` local now clones instead of borrowing (the
> price of not aborting on a miss — written into 0056 as open work), and the
> map-entry centralization guardrail names the merged projection instead of one of
> the three call sites it replaced.
>
> 0055, 0062, 0063, and 0065 are back in `done/`. Audit 0062's reopened
> zero-caller/wrapper censuses were re-run cleanly after the 2026-09-06 cleanup.

Re-verified 2026-09-02 at `6312cb8`, by five parallel agents re-running each
audit's own repros against a freshly built binary, with `javac`/`java` as the
oracle and `rustc` for compilation. Raw evidence, including every command and
its real output, is in `claude_audits/recheck-2026-09-02/`.

**Read this first.** The release binary was **stale** when the recheck started:
`src/` changed in the 09-02 commits, but `cargo build --release` reported
`Finished in 0.03s` and left the 08-30 binary in place. Every verification would
have tested three-day-old code. Forcing it (`touch src/*.rs && cargo build
--release`) rebuilt in 62s. If you are checking your own work with
`cargo build && ./target/release/new-java2rust`, confirm the binary's mtime is
newer than HEAD before believing the output.

## The recorded baseline is stale, so the gate under-reports

Measured 2026-09-02 at `6312cb8` with **no local changes** (my fixes stashed):

| suite | `suite-results.tsv` says | HEAD actually is |
|---|---|---|
| `type_coercion` | 126 passed / **3** failed | 126 passed / **3** failed after the 2026-09-06 wrapper-test deletion |
| `object_model` | 28 passed / **0** failed | 27 passed / **2** failed |
| `semantic_facts` | 174 passed / **0** failed | 174 passed / **6** failed |

Eleven failures exist at HEAD that the baseline does not record, including
`object_model_input_usage_records_instanceof_dispatch_requirement`,
`records_non_null_field_from_all_constructors` and
`source_method_usage_is_keyed_by_erased_overload_signature`. They are **not**
caused by the fixes in this file — verified by stashing them and re-running, which
gives the identical 3/2/6.

This compounds 0053: the gate compares counts against a file that has drifted from
the tree, so it will report these as *new* regressions the next time anyone
touches those areas, and attribute them to whoever is unlucky. Re-record the
baseline, or fix the eleven, before trusting any gate delta.

## Status: the gate is green — fixed 2026-09-02

`cargo test --release --test differential` → **2 passed, 0 failed.**

It had been failing on three fixtures, all of them previously-passing promotions:

```
LinkedNode:               error[E0599]: no method named `expect` found for reference `&LinkedNode_Node`
MapGetNull2:              error: unsupported Java null comparison: operand is reference-typed
                                 but has no nullable representation
NullableArrayLengthCatch: error[E0599]: no method named `ok_or_else` found for reference
                                 `&Vec<Option<String>>`
```

`MapGetNull2` was a lowering that gave a `Map.get` initializer the element
*borrow* shape, discarding the optionality — so a miss panicked where Java prints
`null`. The other two were one bug: `apply_optional_receiver_slot_projection`
discharged the optionality and then let the conversion discharge it again.
`strip_redundant_java_null_expect_after_unwrap` already existed for exactly that
and was applied at **one of the four** call sites; it now runs inside the
projection, so no caller can forget it. See 0056.

## Verdicts

Six audits were marked done. **Two of the six are actually done.**

| audit | marked | verdict | what is left |
|---|---|---|---|
| 0054 switch arms | done | **FIXED** | nothing — 5/5 sub-cases, 11 nearby variants probed, none silently deleted |
| 0060 catch/finally invisible | done | **FIXED** | nothing at symptom level; 2 of 6 walkers still elide `catches`/`finally_block`, no symptom found |
| 0055 syntax errors | done | **FIXED** | nothing — batch cross-contamination fixed 2026-09-02, back in `done/` |
| 0065 overload suffix | done | **FIXED** | reopened cross-file/interface/transitive/subclass-added-overload scopes rechecked 2026-09-06 |
| 0062 dead code | done | **FIXED** | reopened zero-caller/wrapper censuses now clean |
| 0063 test dead weight | done | **FIXED** | rechecked source-layout scope cleared 2026-09-06 |
| 0056 null | open | **PARTIAL** | headline fixed; `Map.get` and the doubled projection fixed 2026-09-02; broader null work open |
| 0057 exceptions/finally | done | **FIXED/FENCED** | supported completion and exception paths are oracle-gated; unsupported mixed TWR completion is an exact loud fence |
| 0058 inheritance upcast | open | **PARTIAL** | silent → loud; carrier still absent |
| 0059 concat/collections | open | **PARTIAL** | concrete queue/collection regressions rechecked; unknown erased-display capability remains |
| 0061 binding identity | open | **FIXED** at symptom level | architectural residue only |
| 0064 name map | open | **PARTIAL** | 7/7 listed fixed; new sibling collision |

## P0 — silent wrong answers and the red gate

### 1. ~~`Map.get` into a declared local aborts where Java prints `null`~~ — FIXED

`native_map_get_required_declared_value_local_shape` gave the local the element
*borrow* shape, discarding optionality. Removed with its dead function (57 lines);
`let miss: Option<String> = m.get(..).cloned()` and the program now prints
`reached:null`, matching Java. `MapGetNull2` no longer emits `compile_error!`.
The other two gate fixtures fail for a *different* reason — see the doubled-unwrap
section in 0056.

Original report:

```java
Map<String,String> m = new HashMap<>();
String miss = m.get("b");
return "reached:" + miss;          // Java: reached:null
```

Translation **panics** — `Java null receiver` — because `Map.get` into a
*declared local* now plans `&String` with an eager `.expect(...)`. Reproduced for
`String`, `Integer` and `List<String>` values. The get-or-create bucket idiom is
fully broken. Inline `m.get(k) != null` and `getOrDefault` still work, which
points at the 2026-08-30 borrowed-map-element pass over-applying.

This is the root of all three gate failures: `MapGetNull2` (the `== null` becomes
`compile_error!`), and `LinkedNode` / `NullableArrayLengthCatch` (the doubled
`.expect(..).expect(..)` projection that 0056 §5c recorded as fixed is back).

### 2. ~~Interface-default overload: compiles clean, panics at runtime~~ — FIXED

Candidates are gathered by *name*, so `f(String)` was offered as the implementation
of the default `f(int)`, and the code took `candidates[0]` regardless and then
declared them incompatible. A `default` that nothing overrides keeps the
interface's body — the no-candidates branch already knew this; the fix applies it
when a namesake is in the way. Verified: Java `1002,102`, Rust `1002,102`, exit 0.

Original report, now fixed/rechecked 2026-09-06:

0065 had been marked done while this scope was still stale in the recheck.
The current tree now has focused passing checks for the interface-default,
cross-file, transitive-superclass, and subclass-added-overload shapes.

```java
interface I { default int f(int a) { return a + 1; } int f(String s); }
class Impl implements I { public int f(String s) { return s.length() + 1000; } }
I x = new Impl(); x.f("ab") + "," + x.f(101)
```

Java prints `1002,102`. The translation compiles with **0 rustc errors**, emits
**0 translator diagnostics**, and aborts with `exit=101` on:

```rust
impl I for Impl {
    fn f_int(&self, a: i32) -> i32 { panic!("incompatible Java interface method implementation: ...") }
    fn f_string(&self, s: &str) -> Result<i32, JavaException> { Impl::f_string(self, s) }
```

— nine lines below the `Impl::f_string` that exists. Calling through `Impl`
directly was fine; only the trait object path was wrong, which is why it
survived. Same defect cross-file. 0065's §3-mirror (a subclass *adding* an
overload) is now covered by
`idiom_gaps::subclass_added_overload_keeps_inherited_method_callable_by_signature`.

**Why a green test named for this did not catch it:** the three tests at
`idiom_gaps.rs:13676/13726/13777` assert only
`translation.rust.contains("pub fn f_int…")` — spellings, never values, and never
the trait-impl slot. Making 0009's `panic!` a translator diagnostic has now been
asked for four times.

### 3. `Collections.unmodifiableList` / `unmodifiableMap` / `Arrays.asList` are erased — MEASURED, NOT CHANGED

All 11 uses in the repo are `Arrays.asList` consumed read-only as an argument to
`addAll`/`removeAll`, and there are **zero** uses of `unmodifiable*`. A blanket
refusal would break six working fixtures, including differential ones, to fix a
divergence none of them exhibits. The real fix carries "came from an unmodifiable
view" on the value and refuses the *mutating call*. Recorded in 0057.

Original report:

0057's "no silent wrong answers in this group" is now false. These lower to a
plain `mut Vec`/`HashMap`, so Java's `UnsupportedOperationException` never
happens: Java `uoe` vs translated `added:2`, `put:2`, `added:3`. A dead
`catch (UnsupportedOperationException)` arm is emitted alongside.

## P1 — regressions in areas previously verified correct

From 0059's correctness sample. Re-measured 2026-09-02 against the Java oracle.

| | shape | status |
|---|---|---|
| R1 | `Object b = Col.BLUE` | **fixed** — gated by `differential/EnumConstantAsObject.java` |
| R2 | `Col.values()[i]` into `Object` | **fixed** — gated by `differential/EnumValuesElementIntoObject.java` |
| R3 | `Map<String,List<Integer>>` read | **fixed** — gated by `differential/NestedMapValueRead.java` |
| R4 | `sum += e.getValue()` in an entry-set loop | **fixed** — gated by `differential/BorrowedElementIntoPrimitiveLocal.java` |
| R5 | the gate's doubled unwrap | **fixed** under 0056 |

R1 needed two fixes, not one. Making the enum path the only lowering that can
claim a variant turned the `E0599` into a **silent** wrong answer — `b.toString()`
printed `""` where Java prints `BLUE`, because enums were excluded from the
erased-object downcast table. Both halves are in; a loud failure was not traded
for a quiet one.

R2 and R4 are fixed as families, not as single assertion rewrites: source enum
`values()` now lowers both whole-array storage and indexed element reads through
slot-aware enum shapes, and borrowed collection elements are dereferenced when
stored into primitive slots.

Re-measured since this note was written:
- `sb.append(map.get(k))` is covered by
  `string_builder_append_map_get_borrowed_string_value_typechecks`.
- `List.subList` range failures are covered by `ListSubListInvalidRangeCatch`
  and the focused list-exception differential loop.
- nullable map values are covered by `MapPutNullValue` and
  `nullable_map_get_fixtures_match_java_output`.

Still open from 0056/0057:
- nullable array element/source-method nullability should stay under watch, but
  the promoted `NullableArrayLengthCatch` row now passes the focused exception
  loop.
- broader nested/labeled `finally` completion and panic-based Java exception
  surfaces remain design work rather than easy-row cleanup.

## ~~P1 — 0055 finding C~~ — fixed 2026-09-02

Batch mode pooled every *primary* file's parse diagnostics into a batch-wide bag
and appended that bag to each file, so one bad file marked every clean sibling
`errors` and was itself double-counted. Measured on a three-file batch:
`ok/0`, `ok/0`, `errors/1` after; all three `errors` with the bad one at 2
before. Fixed by deletion rather than filtering — a primary file's parse
diagnostics already reach it through `parsed.diagnostics`, so the bag is left to
context and summary sources, the invariant the single-file path always kept.
Pinned by `translate_batch_keeps_parse_diagnostics_with_the_file_that_caused_them`.

## P2 — unfixed by design, or newly found

- **0058 carrier.** The concrete D1/D2 leftovers in this note are no longer
  current. Nested erased-source aliases are rendered with Rust-safe names, and
  the focused mutating-super copyback slice passes
  `cargo test --test idiom_gaps source_mutating_super` (27 tests). The remaining
  work is the broader class-hierarchy carrier/dynamic-dispatch model described
  in `claude_audits/0058-class-inheritance-upcast-discards-the-object.md`.
- **0057 `finally` (historical; superseded by the 2026-09-21 closure).** Still a closure carrier: `E0267 break inside of a closure`
  for a `switch` in the guarded body and for a conditional `return` in the
  `finally`; `E0308 expected Option<String>, found String` for a `return` inside a
  nested `try/finally`. JLS 14.20.2 needs all five abrupt-completion modes.
  `(String) obj` downcasts still `.expect(..)`, so a generated
  `catch (ClassCastException)` arm can never fire.
- **0064 package/type identity.** The sibling `pub type Item = AItem;` /
  `pub type Item = BItem;` collision called out here is fixed and guarded by
  `batch_translation::package_qualified_source_type_references_keep_package_identity`.
  The audit remains open only for the broader first-class package/type identity
  model, not for this concrete E0428 path.
- **0060 residue.** `src/types.rs:130` and `src/expr_shape.rs:1566` still elide
  `catches`/`finally_block` behind `..`. No symptom found; the structural point
  stands. Also a live E0308 on javac-clean Java: a unit-returning method whose
  last statement is a `try` with a `catch`-originated `Result` promotion gets the
  promoted signature but no tail `Ok(())`.

## P2 — the census audits regrew

- **0062.** Every *named* item verifies as gone, and where a live replacement was
  claimed it is genuinely constructed and reachable. ≈**1,669 lines** actually
  deleted against the ~1,700 estimate. The 2026-09-02 recheck found the two
  open-ended closing counts had regrown; the named source-grep keepalive
  examples and additional wrappers (`receiver_constraints`,
  `project_optional_element_borrow_carrier`, `solve_signatures`) have since been
  removed. Later one-symbol passes also deleted no-facts hierarchy/object-model
  wrappers, unused representation storage accessors, unused shared-handle path
  helper, three test-only rendered projection helpers, and the inlined
  exception-message helper. The current wrapper scan reports zero test-only base
  wrappers, and the remaining zero-reference names are generated runtime-support
  template functions excluded by the audit method.
  `src/` net grew 215,270 → 261,600 lines over the window.
  - **`RewriteNeed` was deleted against the audit's explicit instruction.** It was
    in the "never finished, do not delete" list; its four unimplemented rewrites
    now survive only inside the closed audit file.
  - The `DynCloneBoxAdapter*` design was replaced by a live `CloneBoxDynTrait`
    producer, but the adapter *generation* it deferred is still missing — the only
    `clone_box` body emitted for a plain source-class carrier is
    `panic!("requires a cloneable Java object carrier")`
    (`representation_lowering.rs:1020`). The admission moved from `src/` into
    generated output.
- **0063.** Fixed for the rechecked source-layout scope on 2026-09-06. The
  descriptor headline still holds (`46 -> 0`), seven remaining pure source-layout
  guards were moved from `slot_shape_design.rs` into `tests/layering.rs`, and
  the conservative `slot_shape_design.rs` source-only scanner now reports
  `candidate_count 0`. The moved guards pass in `layering` (`66/0`); two stale
  expected-failure names were removed from `slot_shape_design`.

## What is genuinely fixed — do not re-litigate

- **0054** switch arms: all five sub-cases, plus 11 nearby variants probed
  (arrow-with-block-body, 3-way comma labels, nested switches, no-default arrow,
  colon fallthrough, String labels, throw arms, type patterns with `when`). None
  silently deleted; unsupported ones refuse loudly. `count-invalid-patterns` 0 → 1.
- **0060** catch/finally: all four repros, plus eight deeper variants
  (nested try-in-catch, finally-in-catch, multi-catch, loop-in-catch,
  lambda-in-catch, twice-nested shadowed local, try-with-resources).
- **0056 headline**: `new String[3]` is `Vec<Option<String>>`; the `== null` fold
  is now *sound*, not merely absent — nullable field, method-returning-null,
  null-as-argument and reassign-to-null all match the oracle across ten shapes.
- **0059 headline**: `java_erased_object_to_string` is a real downcast table with
  a loud fallback, never `""`. `LinkedHashMap`/`TreeMap` deterministic across
  8 runs each.
- **0061**: both repros produce byte-identical output with and without the
  disjoint shadow. (Note: both staged fixtures are rejected by `javac` — Java
  forbids shadowing an enclosing-block local — so they were re-run as valid Java.)
- **0064**: 7 of 7 listed repros.
- **0065 §1**: the cross-file translator panic is gone, in `translate-batch` too.
- **CLI**: 16 flags (not 15), all connected; 15 diffed empirically.
- 35 fixtures behind 0057's "do not spend time here" table all still match.

## Suggested order

1. 0058's class-hierarchy carrier/dynamic-dispatch model.
2. 0059's genuinely unknown erased-display capability model. The bounded queue
   and priority queue slices now pass focused generated-crate checks.
3. 0064's broader first-class package/type identity model.
4. 0062 is back in `done/`; remaining easy-work candidates move to 0061's
   explicit id-less compatibility surface.
