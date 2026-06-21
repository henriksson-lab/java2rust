# Technical-debt audit & consolidation plan (2026-06)

Output of a 5-worker audit grounded in `SEMANTICS.md`. Theme: replace per-site
special-casing ("lots of edits everywhere") with general, table- or resolver-driven
treatments, and remove dead weight. Each item notes file:line, the proposed general
treatment, error vs. net-zero vs. cleanup, and risk. **All code changes must pass the
usual gate: 12 corpora + tests + golden 42/42 + compilecheck 110/110 + 0 warnings,
zero per-corpus regression.** Baseline at audit time: **10865**.

## 0. Orientation for a fresh session (read this first)

- **Read before starting:** `SEMANTICS.md` (the type-system model — this plan cites its
  §/invariant numbers, esp. P1, N1, N3, U1), and `TODO.md` §1 (current state + the
  authoritative **per-corpus** baseline vector) and §2 (the non-negotiable discipline).
  The auto-loaded project memory has the longer-arc history and NO-GOs.
- **Build:** `cargo build --release` → `./target/release/java2rust-rs`.
- **Measure (the iron rule = reduce total errors with ZERO per-corpus regression):**
  ```
  for t in trim jaligner jahmm varscan fastq bjaaprop vcf bjalign bioformats jhlabs jsoup jts; do
    n=$(bash tools/${t}_check.sh 2>/dev/null | grep "TOTAL ERRORS" | grep -oE '[0-9]+'); echo "$t: $n"; done
  ```
  Total at last commit = **10865**; the per-corpus vector to diff against is in `TODO.md`
  §1 (e.g. jts 5353, jsoup 2440, …). A change KEEPS only if no single corpus goes up.
- **Full gate suite (all must stay green):** `cargo test --release` · `cargo run --release
  --example check` (golden **42/42**) · `bash tools/compilecheck.sh` (**110/110**) ·
  `cargo build --release 2>&1 | grep -c warning` (**0**). `tools/*_check.sh` invocations are
  `timeout`-guarded (a runaway translator self-terminates; jts/bioformats are the slow ones).
- **Work incrementally:** one item, measure, keep-or-revert. Most B-items claim "net-zero" —
  *prove it* (total unchanged, every corpus unchanged). The user commits each checkpoint
  themselves — do not offer to commit.
- **Parked/reusable artifacts:** `docs/parked/*.patch` (regex, collections — runtime written,
  blocked on the frontier work, see TODO); the `atomic.rs` runtime fragment (written, unmapped).
- **The single most-recommended first action is A1** (it both cuts errors and fixes a latent
  bug in already-committed code). Verify `self.expr_nullable` exists (`dump.rs` ~2056) and that
  A1 does not lose the committed null-fold's −98 (it should *gain* on top).

## A. Error-reducing (do first — these cut errors)

1. **Null-fold gate: `self.ty` → `expr_nullable` — ❌ NO-GO (measured 2026-06-21).**
   Implemented exactly as prescribed and measured all 12 corpora: **REGRESSES, total
   +68** (jaligner +1, jahmm +1, jhlabs +6, jsoup +49, jts +11; others flat). Reverted
   (NO-GO comment left at `dump.rs:4619`). Root cause: the *only* behavior change is the
   cell `(self.ty concrete ∧ expr_nullable)` flipping a compiling constant-fold into
   `.is_some()/.is_none()`. For the many locals/params whose `nullable` flag is TRUE yet
   whose emission is **concrete** (the ~32 is_some/unwrap-on-concrete inconsistency — e.g.
   `is_none()` on a concrete `FormatFactory` in jaligner) that produces E0599. There is
   **no error-reducing cell**: the fold→Option-check change is purely *semantic* (it fixes
   a wrongly-folded null comparison, which already compiled), so by construction it can
   only keep-flat or regress under the error gate. A1 cannot land until the nullability
   analysis is made consistent (nullable-flagged ⇒ emitted `Option<T>`, the TODO.md §1 /
   tier-2 frontier). The original prescription's "should *gain*" hypothesis is falsified.

   *(original prescription, kept for the frontier follow-up:)*
   **Null-fold gate: `self.ty` → `expr_nullable` (HIGH, low risk).**
   `dump.rs:4625-4629` gates the `x ==/!= null` fold on `self.ty(other)==Opt`, but
   `self.ty` carries the `N` overlay for *fields only* (via `resolve_self_field_type`),
   NEVER for locals/params (N3). So a nullable local/param resolves concrete and folds
   wrongly — a latent bug in the committed null-fold AND the root of the residual
   `is_some`/`unwrap`-on-concrete cluster (~32). Fix: fold only when
   `!self.expr_nullable(other)` (the `N`-based predicate, `dump.rs:2056`, used by every
   other emit/read decision) AND the type is concrete-non-`Unknown`; emit `.is_some()/
   .is_none()` when `expr_nullable`; leave `Unknown` to the existing path. This is the
   §12-item-7 prescription ("the only sound signal is `N`"). Sound because slot
   emitters gate on the same `N`. Re-mine E0599 after.

2. **Add a `ret` column to `static_rule` — ✅ DONE / net-zero (2026-06-21).**
   Implemented: backfilled certain-type `ret` on the static entries (A3) and wired
   `method_call_type` (`types.rs` ~607) to consult `static_rule(cls,name,arity).ret` for
   class-name (`NameExpr`) receivers. **Measured all 12 corpora: every corpus identical
   to baseline (10865 = 10865), zero regression.** Tests/golden/compilecheck green, 0
   warnings. The wiring *fires and is correct* — verified on a synthetic:
   `Collections.singletonList(x).size()` types, and `NumberFormat.getInstance().format(d)
   .trim()` now resolves `.format()`→String→`.trim()` precisely (the intended chain win).
   It nets **zero error reduction** here only because these static-factory chains are
   rare / already-compiling in the corpus set — the doc's "cuts errors" expectation didn't
   materialize. **Kept as net-zero infrastructure**: it's foundational for B3 (string-ops
   table migration consults `ret`) and B5 (front name-guess with `self.ty`), and improves
   resolver precision generally. Carve-out left to stubs: multi-arg `Arrays.asList(a,b,c)`.

   *(original prescription:)*
   **Add a `ret` column to `static_rule` (MED).** `static_rule` entries are all
   `ret:None`; `TypeResolver` can't type a chained static factory result, so
   `Collections.singletonList(x).get(0)`, `Arrays.asList(a).size()`,
   `NumberFormat.getInstance().format(x)` type as `Unknown` at the chain. Add `ret`
   (the static analogue of the working instance `ret`) keyed to a Java type name the
   resolver records (`"Vec"`/`"HashMap"`/`"NumberFormat"`/`"bool"`/…), and have
   `method_call_type` consult it for static receivers. Conservative, certain types only.

3. **`ret` backfills on existing rules (LOW, do with #2).** Certain-type returns
   currently `None`: instance `Map.containsValue→bool`; static `Character.is*→bool`,
   `*.toHexString/valueOf/getProperty→String`, `Collections.empty*/singleton*→Vec/
   HashMap/HashSet`, numeric `compare/sum→i32`. Aids further `.len()`/`.chars()` chaining.

4. **`rust_type_of` → renderer-backed (MED; also a refactor, see B4).** `dump.rs:1638`
   is a line-for-line shadow of `TypeResolver::type_of_node` but does NOT consult the
   Tier-2 `coll_elem` map → a raw-collection stub-return type can disagree with the
   rendered field type (a latent U1 drift). Backing it with `type_of_node` fixes that
   *and* de-dupes. Blocked only by the missing `Ty→Rust-string` renderer (B4).

## B. Net-zero generalizations (the "patterns instead of many edits" the user asked for)

The recurring pattern: a `(java_type, name, arity)`-keyed **table** replaces N scattered
per-type branches. `stdlib::runtime_method_ret` already proves the shape; extend the
family.

1. **`runtime_method_overload(java_type, name, arity) -> Option<&str>` (HIGH net-zero).**
   Collapses the 4 near-identical arity-overload prologue blocks in
   `try_emit_known_method` (`dump.rs:5394-5505`, ~110 lines: BitSet/CRC32+Deflater+
   Inflater/Random/Writer-family) into one ~15-line dispatch site + a data table next to
   `runtime_method_ret`. Behavior-preserving (copy the suffix decisions verbatim). One
   carve-out: `CRC32.update(1)` disambiguates by arg-type, not arity — keep as a residual
   or make the table value an enum `{Suffix, ByArgType{vec,scalar}}`.

2. **`io_ctor_factory(simple, arity) -> Option<&str>` → move to `stdlib.rs` (HIGH net-zero).**
   The 50-entry I/O ctor factory `match` (`dump.rs:6636-6685`) is already pure data living
   in the dispatcher; relocate behind a `stdlib` fn. Mechanical, net-zero.

3. **Bespoke → `StdRule` table migrations (HIGH net-zero, biggest is free today).**
   - **String search family — ✅ DONE (2026-06-21), −2 errors (better than net-zero!).**
     `startsWith`/`endsWith`/`indexOf(1)`/`lastIndexOf(1)`/`split(1|2)` migrated to
     `instance_rule("String", …)` with `${0:str}` (byte-identical to the old
     `emit_string_pattern`) + `ret` (`bool`/`i32`/`Vec`). Removed the 5 bespoke arms and
     the now-dead `emit_str_arg`. **Measured: total 10865→10863 (fastq −1, jsoup −1), ZERO
     regression**; tests/golden 42/42/compilecheck 110/110/0 warnings green; added the
     `string_search_family_routes_by_category` coverage test. The win is the `ret` the
     bespoke arms lacked (now `s.indexOf(x)` types `i32`, fixing 2 downstream chains). The
     category gate (`recv_category=="String"`) preserves the `("List","indexOf",1)`
     element-search disambiguation. NOTE the bespoke arms had a *broader* gate (no
     category check → also fired for `Object.toString().startsWith(..)` Unknown receivers,
     coercing the arg); that gap exists post-migration (such a call now emits an uncoerced
     arg) but does NOT occur breakingly in the 12 corpora — net win stands. The 2-arg
     `indexOf`/`lastIndexOf` keep their bespoke offset logic.

     *(original prescription:)*
     **String search family** (`startsWith`/`endsWith`/`indexOf(1)`/`lastIndexOf(1)`/
     `split(1|2)`, `dump.rs:5723-5796`) → `instance_rule("String", …)` using the EXISTING
     `${0:str}` placeholder; category-keyed disambiguation vs the `("List","indexOf",1)`
     complement already exists. No new machinery — the largest available net-zero migration.
   - **Optional/stream zero-branch arms** (`orElse`/`orElseGet`/`reduce`/`findFirst`/
     `findAny`/`mapTo*`/`toArray`/`stream`/`count`/`sum`) → table (pure templates).
   - **String value ops** (`trim`/`toCharArray`/`substring`/`charAt`/`equalsIgnoreCase`) →
     ❌ **NO-GO (measured 2026-06-21, +7: bjaaprop +3, vcf +1, jsoup +3; reverted).** Their
     default emission is a NON-existent method (`.substring`/`.char_at`/`.equals_ignore_case`)
     so the bespoke arms' broader no-category gate (which catches Unknown receivers like
     `x.toString().substring(1)`) can't be narrowed to `recv_category=="String"` without
     E0599 on those sites. Blocked on the resolver typing such chain receivers as `String`.
     **Map ops** (`containsKey`/`keySet`) → table with `ret` (not yet attempted).
   - **`Optional.of/ofNullable/empty`, `IntStream.range/rangeClosed`** → `static_rule` —
     ✅ **DONE (2026-06-21), net-zero.** Added 4 `static_rule` entries (`ret:None` — the
     `Some/None`/`Range` results aren't simple named types) and DELETED
     `try_emit_optional_static` + `try_emit_int_range` (~44 lines) and their two call
     sites; `try_emit_stdlib`'s static path now handles them (verified nothing between the
     old call sites and it matches a static `Optional`/`IntStream` receiver). **All 12
     corpora flat (10863), zero regression**; gates green; added the
     `optional_and_stream_static_factories` coverage test. Lowest-risk migration because
     it's class-name-keyed, not Unknown-receiver-gated (the slice-2 failure mode).
   Gate: assert each migrated arm's receiver gate == the `recv_category` key (String:
   `Type::Str ⇔ category String`); extend the table coverage test. Arms that branch on a
   node (`collect`/`filter`/`append`/`add`/`get`/`put`) stay bespoke — justified.
   - Optional new placeholder `${N:disp}` (char-vec-aware stringify) would let `append`
     migrate too; lower value.

4. **A single `Ty → Rust-string` renderer (MED, foundational).** None exists — the
   "Rust-string" derivers (`rust_type_of`, `infer_expr_rust_type`, `infer_call_ret_type`,
   `java_simple_to_rust_static`) all hand-roll fragments of it. One renderer + backing
   these by `self.ty()`/`type_of_node` kills the parallel derivations (enables A4) and
   lets `infer_expr_rust_type` (stub param types) benefit from chain/cast/`new` typing.
   Caveat: the `recv_type_name` NameExpr/FieldAccess arms must STAY AST-name-based
   (routing `Named` flips `receiver_is_user_type` — measured regression, P1).

5. **Front the `append/charAt/substring → String` name-guess with `self.ty(call)`**
   (`infer_call_ret_type` `dump.rs:1394-1400`). Try the resolver first (precise via the
   runtime-ret table), keep the name-match only as the `Unknown`-receiver fallback.

6. **Standardize "ask the category" (LOW).** Three ways exist: `recv_type_name=="String"`
   (5585/5592/5678/5817/5826), `recv_category` (5751…), `self.ty().category()`. They
   DISAGREE: `recv_category` collapses `StringBuilder`/`CharSequence`→String;
   `recv_type_name` returns the raw name. Standardize the String-gates on `recv_category`
   except where a true-`String`-vs-`StringBuilder` distinction is required (comment those).

7. **Trait-boilerplate macros in `runtime/header.rs` (LOW, readability).** ~120-150 lines
   of hand-rolled `Clone`/`PartialEq`/`Eq`/`Hash`/`Display` across atomic/zip/decimal_format/
   io_read. Lift `value_eq_hash!`/`rc_identity_eq_hash!`/`noop_display!` into `header.rs`
   (shared first-include) and reuse the existing `io_write_trivial_traits!`. Macros must
   expand byte-identically (measure generated output).

## C. Dead-code cleanup

**✅ C1+C2+C3 all DONE (2026-06-21), net-zero.** All 12 corpora identical to baseline
(10865 = 10865, zero regression); tests/golden 42/42/compilecheck 110/110/0 warnings green.
- **C1:** dropped `include_str!("runtime/atomic.rs")` from the `JAVA_RUNTIME` concat
  (`crate_layout.rs`); kept the `include!` in `java_runtime_compiles` (still compile-checked,
  verified by `cargo test`). Comments updated there + at `dump.rs` map_type_name.
- **C2:** added `strip_cfg_test_mods()` (`crate_layout.rs`), applied at `java_runtime.rs`
  write time — strips the 8 per-fragment `#[cfg(test)] mod tests{…}` (brace-balanced) from
  the shipped text. Verified on a generated crate: 0 `cfg(test)` / 0 `mod tests`, braces
  balanced, runtime compiles. Fragments keep their tests for `java_runtime_compiles`.
- **C3:** removed the dead `StdRule.mutates` field (set by `r`/`rm`/`rr`, read nowhere; the
  operational `&mut` signal is `id_tracker::is_mutating_method`). Kept `rm` as a
  self-documenting alias for mutating table entries. `name_mutates` + its subset coverage
  test retained (the `name_mutates` vs rm-entries drift on putAll/removeAll/retainAll is a
  separate, behavioral concern — left untouched; `name_mutates` is test-only).

*(original prescriptions:)*

1. **`atomic.rs` is shipped-but-unreachable (~7.4 KB/crate).** Nothing maps it
   (`map_type_name` has only the parked comment). **Keep it in the `java_runtime_compiles`
   compile-check** (so it stays sound for the §12-item-7 resurrection) but **drop it from
   the `JAVA_RUNTIME` concat** (`crate_layout.rs:1083`) so it's not pasted into every
   generated crate. Update the `dump.rs:8229` comment. Zero behavioral change (measure).
2. **Per-fragment `#[cfg(test)]` modules ship as ~13 KB/crate of inert text** (concat
   pastes raw source). Strip them from the shipped text. Disk hygiene; measure.
3. **`StdRule.mutates` is dead** (read nowhere; `name_mutates` is a separate hand-list
   with existing drift — table marks `putAll/removeAll/retainAll` mutating, `name_mutates`
   omits them). Either delete the field+function or regenerate `name_mutates` from `rm`
   entries; strengthen the coverage test (`tests/stdlib_table.rs:78`) from subset to exact.
4. **NO-GO comment blocks in `dump.rs` are documentation — KEEP** (they encode measured
   regression deltas + root causes that SEMANTICS §11 depends on).

## Suggested order
A1 (close the residual + fix latent bug) → A2/A3 (static `ret` + backfills) →
B4 (renderer) → A4 (rust_type_of, needs B4) → B1/B2/B3 (the big net-zero consolidations)
→ C1/C3 (cheap cleanups) → B6/B7/C2 (polish). A-items reduce errors; B-items pay down the
"many edits" debt the user flagged; C-items remove weight.
