# Handover notes (for the next session)

You are continuing work on a Java→Rust translator. The goal: emit Rust that
**compiles and is semantically faithful**, validated by translating real
bioinformatics/image/geometry corpora and counting `rustc` errors. Work proceeds by
landing small, **measured** changes.

## 0. READ FIRST
- **`SEMANTICS.md`** — the authoritative model of how types are handled (the `Type`
  IR, the two tiers, the four orthogonal overlays `Borrow∘Option∘Route∘Resolve`, and
  the **all-sites invariant** that every type rewrite must obey). This is the map of
  the whole strategy and the open work. Read it before touching type/codegen logic.
- `docs/subtype-research-plan.md`, `docs/structural-engines-plan.md`,
  `docs/tier2-unification-plan.md` — the research/engine plans with recorded
  outcomes (every NO-GO is a documented result — don't re-attempt them).
- Memories (auto-loaded via `MEMORY.md`): `tier2-unification-frontier`,
  `clone-reduction-audit-loop`, `object-slot-enum-synthesis`, `bioformats-test-corpus`,
  `no-commit-prompts` (**the user commits; never offer to commit**).

## 1. Current state
- **UNCOMMITTED on tree (2 increments on top of committed `014edf0`): errors 11114→10865
  (−249), ZERO per-corpus regression.** Both from the nullable/type-resolution frontier:
  • **null-comparison fold (−98):** Java `x != null`/`x == null` always lowered to
  `.is_some()`/`.is_none()` → E0599 when `x` resolves concrete (non-`Option`). Fold to a
  constant when `self.ty(x)` is concrete non-Option/non-Unknown (dump.rs ~4617).
  • **`this.field` type resolution (−151; jts −67, jsoup −58, jhlabs −24):** `TypeResolver`
  returned `Unknown` for `this.field`/inherited/bare-field member reads (`decl_type_node` only
  resolves free lexical names). Added `resolve_self_field_type` (types.rs — walks current_class
  + parents in the symbol map; returns `Option<T>` when `FieldSym.nullable` so a nullable field
  keeps its null-check, concrete otherwise) and wired it into `compute_type_of`'s NameExpr +
  `this.`-FieldAccessExpr arms. This root-cause fix feeds `self.ty` everywhere (dispatch,
  coercion, the null-fold now catches `this.field`), hence the broad win. Validated: golden
  42/42, compilecheck 110/110, tests 0-fail, 0 warnings; jts/jsoup crates built fully.
  **`.unwrap()`-on-concrete cluster — NO-GO via `self.ty` (measured +2503, reverted).** Gating
  the nullable-read unwrap on `self.ty(id)` being concrete is FUNDAMENTALLY WRONG: `self.ty`
  returns the BASE type (e.g. `Str`) for a nullable LOCAL/PARAM too, even though it's emitted
  `Option<String>` — so "concrete" ≠ "not an Option", and the gate broke every nullable local/
  param read. The only reliable "is this emitted as `Option`?" signal is the dumper's `nullable`
  flag (`self.nullable` for locals, `FieldSym.nullable` for fields) — already used. The residual
  failures (`unwrap`/`is_some` on concrete `String`/`Vec`/`Attributes`, ~32) are the SPECIFIC
  cases where the read's nullable flag is TRUE but emission is concrete; fixing them needs the
  nullability analysis itself made consistent (a field/value flagged nullable must be emitted
  `Option<T>`, OR removed from the nullable set when emitted concrete) — NOT a `self.ty` gate.
  NOTE: the committed null-fold uses `self.ty`-concrete safely only because null-comparisons on
  nullable locals are rare AND `this.field` resolves nullable fields to `Opt` via
  `resolve_self_field_type`; do not generalize that pattern to unwrap.
  Other tractable levers from the E0599 histogram: `entry_set` on HashMap (18 — but Map.Entry→
  tuple was a prior NO-GO), `to_array` on Vec (9), collection `size`/`add` on user collection
  types. The big standalone cluster remains raw-collection **E0107** (~1000 jts).
  Two cheap experiments this session were NO-GO (kept guarded/parked): atomics mapping, and
  re-enabling chained-receiver stub recording (jts +9).
- **UNCOMMITTED on tree (type-frontier Phases 1+2+3a+3b): errors 11284→11114 (−170!), ZERO
  per-corpus regression** (jts −104, jsoup −35, jhlabs −11, trim −9, vcf −5, jaligner −3,
  bjalign −2, jahmm −1). Method-call RETURN-TYPE tracking. The rich `TypeResolver`
  (src/types.rs) already recursively types chained calls + project/self returns; it lacked
  (a) stdlib/runtime return types and (b) a path from the SHALLOW dispatch helpers into it.
  Done: **P1** — `ret` column on `StdRule` + `runtime_method_ret(java_type,name,arity)` table,
  Java-name keyed (a `Random` value types as `Named{"Random"}`, NOT `JavaRandom`). **P2** —
  `method_call_type` (types.rs ~571) consults both, ADDITIVELY (only fills `Unknown`). **P3a** —
  `recv_type_name` (dump.rs:6157) falls back to `self.ty()` for a `MethodCallExpr` receiver,
  **String-only** (a name for any `Named` type regressed +7 by flipping `receiver_is_user_type`
  — NO-GO); + fixed the `.equals` rewrite (dump.rs:5550) to `.to_string()`-compare unless both
  sides string-like (was slicing an `Unknown` arg → E0608). **P3b (the big one, −148 alone)** —
  `callee_recv_type` (dump.rs:1726) resolves a `MethodCallExpr` receiver via `self.ty()` so
  `resolve_linked_callee` shapes chained calls `a.foo().bar()` (exact rust name, arg borrowing,
  nullability). GUARD: `record_missing_call` (dump.rs:1683) skips `MethodCallExpr` scopes to
  preserve prior stub shapes (the documented stub-regression trap). Validated: 715-file jts crate
  built fully; golden 42/42, compilecheck 110/110, tests 0-fail, 0 warnings.
  Next (frontier work COMMITTED as `014edf0`; two follow-ups re-tried this session, both NO-GO):
  • **P5 atomics — NO-GO (re-tried):** the no-op `unwrap(&self)->Self` overlay did NOT fix it;
  mapping AtomicInteger/Long/Boolean still regresses (trim +14, jsoup +1) with a different root
  cause — `expected bool, found JavaAtomicBoolean` / `expected i64, found JavaAtomicLong`: the
  carrier flows into a primitive position WITHOUT `.get()` (the field type is inferred as the
  primitive from `.get()` usage, but the value is the carrier). Needs value-vs-primitive
  reconciliation. Arms still omitted (see dump.rs map_type_name comment).
  • **P4 regex — needs manual re-wire:** `docs/parked/regex-pattern-matcher.patch` no longer
  applies (base drifted; `git apply` rolls back). Chained String dispatch now works (3a/3b), but
  `Matcher.group(n)` nullability (`if m.group(3) != null`) is still unsolved — re-wire by hand
  (regex.rs + map arms + `("Matcher","group",_)=>"String"` in runtime_method_ret) AND solve the
  null-compare, then measure jsoup. Best as a focused follow-up.
  • Still open: re-try **P3b stub recording** (guarded off — may help more); the raw-collection
  **E0107** cluster (jts ~1000) needs element-propagation/unification.
  Plan/risks in memory [[stdlib-stub-implementation]] / [[tier2-unification-frontier]].
- **Prior wave-4: 3 KEEP integrated, errors 11327→11284 (−43), ZERO per-corpus regression.** 5 parallel worktree agents; integrated A+D+E (`git apply --3way`). (A) Tier-1
  templates −30 (`stdlib.rs` + new `src/runtime/util.rs`; star win: `String.toLowerCase/
  toUpperCase(Locale)`→`.to_lowercase()`, was emitting non-existent `to_lower_case`). (D)
  java.util.zip −10 (new `src/runtime/zip.rs` via **flate2**; cleared the vcf GZIPInputStream
  residual). (E) exceptions −3 (`dump.rs` ThrowStmt nested-wrapper peel; the 460-ref IOException
  cluster is a non-problem). Combined delta is fully additive. Validated: 99 tests, golden 42/42,
  compilecheck 110/110, 0 warnings. New per-corpus baseline below (**11284**). **2 NO-GO parked
  in `docs/parked/`** (patches + README): regex Pattern/Matcher (jsoup +13) and PriorityQueue/
  EnumSet/WeakReference (jts +52) — both blocked by the translator-core frontier (return-type
  tracking for runtime-mapped method calls + nullable-overlay for mapped value types; also
  project-type-name shadowing + raw-generic placeholders for the collections). That frontier
  now gates regex AND atomics — fix it next to unblock several parked items at once.
- **Prior wave-3 (committed `3142580`): the file-I/O reader+writer STACK, errors 11346→11327
  (−19), ZERO per-corpus regression.** New `src/runtime/io_read.rs`
  (JavaInputStream/JavaReader carriers, `Rc<RefCell>` shared cursor; `IntoBoxedRead` adapter
  trait at the factory boundary) + `src/runtime/io_write.rs` (JavaOutputStream/JavaWriter
  carriers; `IntoBoxedWrite`). dump.rs: map_type_name arms for the read/write families →
  carriers, factory-fn ctor routing in `visit_object_creation`, writer-`println` branch,
  subclass `impl Read` in the Deref block. `src/stubs.rs`: stub `Unknown` now impls a no-op
  `std::io::Write` (satisfies `IntoBoxedWrite` via blanket). Wins: jahmm −10, varscan −3,
  jts −3, trim −2, jsoup −1. New per-corpus baseline below (**11327**). Validated: 99 tests,
  golden 42/42, compilecheck 110/110, 0 warnings. Remaining I/O residual (future): stub
  InputStream SUBTYPES (GZIPInputStream/BlockCompressedInputStream/SeekablePathStream — named
  stubs, not `Unknown`) don't impl Read → needs flate2/named-stub Read impls.
- HEAD `13b150d` (committed: File runtime, Tier-0 structure, System statics, AND wave-1
  Lane-1 templates + Random/BitSet/StringTokenizer; baseline **11346**). **Earlier uncommitted
  wave-2 infra** (folded into the wave-3 tree above): a *general runtime
  ctor-arity rule* (`dump.rs` ~6486: mapped-type ctor = `::new` for arity-0, `::new_<arity>`
  for arity≥1) that REMOVED the per-type ctor special-cases; fragment ctors renamed to match
  (`JavaFile::new_1`/`new_2`, `JavaRandom::new`/`new_1`, `JavaBitSet::new`/`new_1`,
  `JavaStringTokenizer::new_1`/`_2`/`_3`). Net errors 0 (verified all-12 = 11346). PLUS
  **`java.text.DecimalFormat`/`NumberFormat`/`DecimalFormatSymbols` LANDED** (`src/runtime/
  decimal_format.rs` mapped; net-zero + real formatting): a `JavaNum` arg trait accepts
  `&f64`/i64/i32/f32, `getInstance(Locale)`→0-arg `get_instance()` via a `static_rule` arm.
  `src/runtime/atomic.rs` written (now with PartialEq/Eq/Hash) but still UNMAPPED — see below.
- **wave-2 NO-GOs (don't retry blind):** Map-family alias→HashMap is unsafe for generic-keyed
  maps (`EnumMap<E,V>`→`HashMap<E,V>` fails `K:Hash+Eq`, jahmm +13). **Atomics** remaining
  blocker: atomic FIELDS emit concrete (`pub x: JavaAtomicBoolean`) yet read-flagged nullable
  → spurious `.clone().unwrap()` on the concrete type (trim +14); needs the translator
  nullable-inference fix OR a no-op `unwrap(self)->Self` on the atomic types, then map.
- **12-corpus error baseline** (working tree w/ null-fold + this.field resolution; `tools/<name>_check.sh`):
  trim 169 · jaligner 48 · jahmm 390 · varscan 53 · fastq 49 · bjaaprop 91 · vcf 422
  · bjalign 552 · bioformats 15 · jhlabs 1283 · jsoup 2440 · jts 5353  (**= 10865**).
  (committed type-frontier **11114**; pre-frontier wave-4 **11284**; wave-3 **11327**; pre-wave-3 **11346**.)
- **Clone-marker baseline** (`grep -rho 'validate added clone'` over fresh translation):
  trim 469 · jaligner 121 · jahmm 214 · varscan 890 · fastq 38 · bjaaprop 421 · vcf 379
  · bjalign 404 · bioformats 982 · jhlabs 966 · jsoup 1611 · jts 4409  (**= 10904**).
- **MEASUREMENT GOTCHA:** count clone markers only over a *translate-only* dir (e.g.
  `/tmp/audit-<c>`), NEVER over a `tools/*_check.sh` build dir (`/tmp/<name>-rs`) — after
  `cargo build` its `target/` holds vendored/built `.rs` with the marker text and the
  grep over-counts ~2.4×. The `*_check.sh` ERROR counts are fine; only the clone grep is.
- Corpora live under `testdata/` (gitignored; cloned). Translator binary: `cargo
  build --release` → `target/release/java2rust-rs`.
- **Clone markers**: every translation-added `.clone()` carries
  `/* TODO(translation): validate added clone */` (see README ⚠️). Clone-reduction
  work is measured by this marker count, not just errors.

## 2. How to work here (the discipline — non-negotiable)
- **Measure after every change**: all 12 corpora + `cargo test --release` (92) +
  `cargo run --release --example check` (golden 42/42) + `bash tools/compilecheck.sh`
  (110/110) + `cargo build --release 2>&1 | grep -c warning` (0).
  ```
  for t in trim jaligner jahmm varscan fastq bjaaprop vcf bjalign bioformats jhlabs jsoup jts; do
    n=$(bash tools/${t}_check.sh 2>/dev/null | grep "TOTAL ERRORS" | grep -oE '[0-9]+'); echo "$t: $n"; done
  ```
- **Monotone / all-sites** (SEMANTICS §11): a type-representation change must update
  *every* read and write of the value, or not fire. Partial = cascade. Every NO-GO
  this project hit violated this — it's the #1 pre-flight check.
- **KEEP only if net-negative errors with ZERO per-corpus regression** (or, for
  clone-reduction work: clones down + errors not up). Else REVERT; a precise NO-GO
  is a valuable result.
- **Diagnose the pattern before fixing.** The biggest wins came from "this small
  bucket is a symptom of a general bug" (over-wrap −1586, overload-resolution −361).
- **Parallel forks** do the heavy builds (see §5). The user commits at checkpoints —
  do NOT offer to commit.

## 3. What's landed (engines, so you don't redo them)
R0a nested-type resolution (−89) · Eq/Hash-derivability fixpoint · R2 `Box<dyn Trait>`
coercion · **R4 sealed-hierarchy enums** (`<Root>Kind`, dispatched hierarchies →
enum; instanceof/cast work) · **over-wrap fix −1586** (don't re-wrap already-enum
reads) · **overload resolution −361** (arg-type-directed, not base-overload) ·
arg/assignment/Option-composition enum-wrap · hex-float/bitop fix −61 ·
parent-cycle/module-collision/comment robustness · **Tier-2 substrate + Phase-1
leaf-local collection elements** · **last-use move** (clones −180) · **nullable-unwrap
last-use move** (clones −78, errors −1, uncommitted): `is_movable_last_use` extended
to the nullable-name `.clone().unwrap()` site in `visit_name_expr` — an owned local at
its last read moves through the unwrap (`x.unwrap()`) instead of `x.clone().unwrap()` ·
**read-only-method-receiver `.as_ref()` borrow** (cumulative clones −629, errors −12;
NAME site committed, field/LazyLock/logging uncommitted): the §6 use-site-borrow
analysis, landed in measured slices. `is_readonly_method_receiver` +
`is_readonly_java_method` (conservative `&self` Java-method whitelist, incl. logging
methods) in `dump.rs`. A nullable read whose parent is a whitelisted read-only method
call borrows through the Option instead of cloning: emits `.as_ref().unwrap()` (yields
`&T`, zero clones; the call autorefs) — applied at the NAME site (3217, slice 1, −466),
the FIELD sites (4910 `this.field` / 3097 inherited, slice a, −35) and the LazyLock-const
site (3206 drops its `.clone()`, slice e+logging, −128). Ordering: last-use > as_ref > clone.
**`java.io.File` → real `JavaFile` runtime type** (errors −7, zero regression, uncommitted):
first real stdlib type beyond `JavaIter`. `map_type_name` arm `"File" => "crate::java_runtime::JavaFile"`
(dump.rs ~7888) + a `PathBuf`-backed `JavaFile` in `JAVA_RUNTIME` (crate_layout.rs ~1108)
with the full `java.io.File` method surface doing real `std::fs`/`std::path` work. Two
integration fixes were required and are REUSABLE for the next runtime types: (i) ctor
overload arity-suffix for mapped types (dump.rs ~6450: a `crate::java_runtime::` base with
≥2 args emits `::new_<arity>`, since mapped types aren't in the symbol map that normally
disambiguates `new`/`new_2`); (ii) path/string args bounded by `ToString` (not
`AsRef<Path>`) so they accept the same breadth the opaque stub did (`String`/`JavaFile`/
`Unknown` — all `Display`). See §4.2b for the full recipe + remaining gotchas.

## 4. Open work — in dependency/priority order
1. **Clone-pattern audit (task #40) — IN PROGRESS, the active lever.** A 6-agent
   parallel audit (all 12 corpora) converged on ONE root pattern with the most
   leverage: **a nullable read emitted in a borrow-only use-site should borrow through
   the Option (`.as_ref()`/`.as_mut()`), not clone+unwrap.** This is the §6 use-site
   borrow analysis, applied as a sequence of LOCAL, measured slices. Slices 1/(a)/(e+log)
   landed (read-only-method receiver at NAME, field, and LazyLock sites; clones −629
   cumulative, errors −12, zero regression — see §3). **DONE: (a) field sites, (e)
   LazyLock receiver + logging-method whitelist.**
   **Queued slices (each its own measured KEEP; ordered by confidence × leverage):**
   - (b) **Index-base nullable read** `x.clone().unwrap()[i]` → `.as_ref().unwrap()[i]`.
     **ATTEMPTED & REVERTED (NO-GO without element-type info).** The simple version
     (borrow the base at sites 3217/4910/3097/3206 when the read is the base of an
     `ArrayAccessExpr`) gives a HUGE clone win (−510 incl. jhlabs −237, jts −141) but
     regresses errors: **jhlabs +3, jts +4**. Root cause: for a *non-Copy element
     struct* read in a numeric-coercion context (`pts[i].x - 1000` on `Vec<Point>` /
     `Vec<Coordinate>`), borrowing the base through `&Vec` reshuffles/leaks the
     translator's (already-buggy) f32/f64 coercions, netting new errors. Tried gating by
     element projection: excluding `arr[i].field`+`arr[i].m()` zeroed ALL the win (every
     win is on a projected element); excluding only `arr[i].field` ALSO zeroed it (the
     wins ARE the field reads). Only ~7 of ~500 field-reads cascade and they can't be
     separated by a local predicate — the discriminator is *element-Copy-ness × numeric
     context*. **To revive:** thread the array element's `Ty` to the index site and apply
     the borrow only when the element is Copy/scalar (pixel `int[]`/`float[]`), OR fix the
     upstream f32/f64 coercion so the cascade can't happen. Helper scaffolding
     (`is_array_index_base_read`) was removed; see the comment on `use_is_read_borrow`.
     The write-target form (`do_hsv.clone().unwrap()[0] = …`) is also a **real
     correctness bug** (mutation lost to a discarded clone) — fix with `.as_mut()` when
     the access is an assignment target AND the Option is `&mut`-reachable.
   - (c) **Comparison/condition operand** (`==`/`!=`/`if`/`while`) → `.as_ref().unwrap()`.
     ~200. Watch `&T == T` typing on slice-compares (`&(x.as_ref().unwrap())[..]`).
   - (d) **`&`-borrow argument** (jts P1, ~388): when the emitter already prints a
     leading `&` for a by-ref param, the value only needs to borrow — but `&` +
     `.as_ref().unwrap()` = `&&T`, so this needs coordination (suppress the `&`, or emit
     `&*…as_ref().unwrap()`) in `print_one_default_argument` (~1980). Medium effort.
   - (e) **LazyLock-const read receiver** (3206): `(*Self::LOGGER).clone().debug(…)` →
     drop the clone (the deref is already `&T`). ~60-78, whitelist-gated like (a).
   - (f) **foreach iterable** (2975, ~493): only the **last-use-local / owned-temporary**
     subset is no-ripple (move/drop the clone). The general `for v in &iterable` form
     rebinds `v` to `&T` → ripples into the body (MEDIUM); do NOT do the general form.
   - (g) **`Map.get(k).cloned().unwrap()` read-context** (5399, ~200) → `.get(&k).unwrap()`
     (`&V`) when consumed by parse/format/comparison; **`.copied()` for Copy values**
     (drops ~76 false-positive markers — a Copy clone is free).
   For each: gate on the conservative whitelist / use-context, build, re-translate +
   measure clones AND all-12 errors (KEEP only if clones down & zero per-corpus error
   regression). NOTE: borrowed-returns (§4.3) is the CLOSED path — do not restart it.
   **Genuinely-NOT-avoidable (don't chase):** `Vec`-index `[i].clone()` stored owned;
   `Validate::not_null(Some(x.clone()))` (by-value param sig); R4 cast-extract
   `match &x {…=> v.clone()}` (matches `&x`, needs owned); `.iter().cloned()` into
   `JavaIter` (owning wrapper); copy-ctor `self.x = param.clone()` (param is `&T`).
2. **`if`/`switch` as a value-expression (task #41)** — `let r = if c {a} else {b}`
   (clone/temp avoider). A concrete local pattern the audit will surface.
2b. **Implement common stdlib stubs with real Rust equivalents (user-requested) — IN
   PROGRESS.** Externals auto-fall-back to opaque stubs (`pub struct X{}` + generic
   `unimplemented!()`, `stub_<pkg>.rs`); they COMPILE but do nothing, and `Unknown`
   returns sometimes mismatch. **`java.io.File` landed** (errors −7; §3). **Proven recipe
   for the next type** (validated end-to-end):
   - (1) one arm in `map_type_name` (dump.rs ~7888): `"X" => "crate::java_runtime::JavaX"`.
     This alone maps the annotation + ctor, flips `receiver_is_user_type`→false so methods
     emit by snake-case, AND suppresses the stub (via `missing_type_key`).
   - (2) add `JavaX` to `JAVA_RUNTIME` (crate_layout.rs ~1108). NON-generic struct (type
     annotations carry no generic args). Methods snake-cased (Java `readLine`→`read_line`).
     Return **`Option<T>` not `Result`** (no auto-`.unwrap()` for runtime types; fits the
     `as_readline_assign` lowering `while ((l=in.readLine())!=null)` → `while let Some(l)=`).
   - (3) bound ctor/path args by **`ToString`** (matches the stub's permissiveness; `Unknown`
     is `Display`). The ctor arity-suffix fix (dump.rs ~6450) is already in place so
     `new X(a,b)` → `JavaX::new_2(a,b)`; define `new_2`/etc. to match.
   - **MUST cover the FULL called-method surface** of the type (the `stub_<pkg>.rs` file
     lists it exactly) or previously-compiling stub calls regress to E0599/E0061.
   **➜ FULL CHECKLIST: `docs/stdlib-checklist.md`** — every stub type+method across the 12
   corpora (built by a 6-agent audit), with per-type vehicle (runtime-type/template/alias/
   drop/needs-crate), difficulty, and a 5-tier implementation order. Highlights:
   - **Tier 0 (do first): code-structure refactor.** Move the runtime out of the
     `crate_layout.rs` string literal into real `src/runtime/*.rs` fragments assembled via
     `concat!(include_str!(...))` (keeps the flat `crate::java_runtime::Type` module → zero
     generator/`dump.rs` changes) + a `#[cfg(test)] include!` block so `cargo test`
     type-checks the runtime. Full steps in the checklist doc's "Code structure plan".
   - **Tier 1 cheap templates/aliases** (Arrays, Collections, Objects.hash, Map.Entry→tuple,
     Locale, System statics, Logger, Hashtable/Properties→HashMap, const/exception aliases).
   - **Tier 2 easy leaves** (io byte/string streams, atomics, BitSet, StringTokenizer).
   - **Tier 3 the file-I/O stack** — settle abstract supertypes first
     (`InputStream`/`Reader`/… → `Box<dyn Read/Write>`, no Rust subtyping; wrapper ctors take
     `impl Read/Write` so nested ctors compose), then BufferedReader/PrintStream/etc.
   - **Tier 4** Random (bit-match JDK LCG), DecimalFormat, Comparator/function/Stream
     (boxed closures), jts awt.geom (needs real FIELDS).
   - **Tier 5 defer** (engine/dependency-bound; low corpus count).
   - **⚠️ DECISION NEEDED:** regex/flate2/url/reqwest/zip families need Cargo crates —
     pure-std vs deps is a user call (see the checklist's "DECISION NEEDED" section). Most
     of the worklist (io/util/lang/text/awt-geom) is pure-std.
   NOTE the earlier ranking over-counted `java.awt.image` (BufferedImage) — it's NOT a
   stdlib stub; jhlabs resolves it against app-recovered `com.jhlabs.*` types. Out of scope.
   See memory `stdlib-stub-implementation`. Measure all-12 errors + suites each step.
3. **Borrowed-returns / lifetimes — CLOSED as a clone reducer (NO-GO), see SEMANTICS
   §6.** The full stage-1 (getter `&self→&T` + call-site clone-on-demand) was built
   and measured: clones went **UP** (jts +316) — a borrowed return moves the one
   callee clone to its *many* callers, most of which consume the result owned and so
   each clone. The borrow checker does NOT cascade (good), and errors net −48, but
   vcf +2 (a `&&T` let-binding edge) and clones up → **do not keep for the clone
   goal.** A genuine win needs whole-program **caller-read-dominance** analysis (only
   borrow-return a getter if reads dominate) = the global ownership inference the
   fixed-borrow strategy avoids. The diff (cuts errors −48) is reconstructable if
   *errors* are ever prioritized over clones. Stages 2–3 (param/multi-input `'a`) are
   parked behind that.
4. **R1 universal-routing tail** — the enum borrow-shape seams (`&Kind`/`Kind`) and
   reverse leaks; likely subsumed by a use-site borrow analysis (same "wrong borrow
   shape at a use" root).
6. **Tier-2 Phase 2 = R4×Tier-2 fusion (task #39)** — `List<Geometry>` →
   `Vec<GeometryKind>`. Needs universal routing first. Unblocks the JTS `E0107` mass.
7. **Full Tier-2 unification** (`tier2-unification-frontier` memory) — global
   type-var union-find + LUB; the general lever for the largest remaining clusters
   (JTS ~1000 `E0107`, the `Unknown` stub-return tail). Foundational, multi-session.
8. **R3 `Box<dyn Any>` / Object (task #33)** — fallback after R4-enum for Object;
   `Map<String,Object>` heterogeneous slots. Lower priority.
9. **Generic trait objects** — `Box<dyn Tr<S,C>>` object-safety/arity; the one area
   both R2 and R4 defer.

## 5. Gotchas (will bite you)
- **Disk fills up.** Corpus builds create multi-GB `/tmp/<corpus>-rs/target` (jts
  ~1.8G, bioformats target was 5.5G). Both host filesystems run ~full. Clean
  `/tmp/*-rs` / stale `*-target` between heavy runs. Watch `df -h / /data`.
- **Forks branch from a STALE base.** Always brief a fork to `git reset --hard
  <HEAD-sha>` first, then `ln -s /data/henriksson/github/claude/java2rust/testdata
  testdata` (testdata is gitignored, absent in a fresh worktree), then reproduce the
  baseline before changing anything. To build on uncommitted work, save a `git diff`
  to `/tmp` and have the fork `git apply` it.
- **Fork transient failures**: rate-limits and `ENOSPC` look like dead forks (tiny
  token count) — check disk and retry; brief forks to STOP on `ENOSPC`, not half-build.
- **Apply fork diffs serially + re-measure** (forks branch independently; interaction
  matters). Remove fork worktrees after (`git worktree remove --force …; git worktree
  prune`).
- The `interval.long-type-*.txt` files in the repo root are stale (pre-session), not
  ours — leave or ask before deleting.

## 6. Immediate next action
Commit the uncommitted KEEPs (use-site-borrow slices: last-use move, read-only-method
`.as_ref()` at NAME/field/LazyLock sites, logging whitelist) + docs, if not done.
Slice (b) index-base was attempted and **reverted** (NO-GO without element-type info —
see §4.1 b: −510 clones but jhlabs +3 / jts +4 from non-Copy-element numeric cascades).
Continue the **clone-pattern audit (§4.1, task #40)** with the next *type-info-free*
slice — **(g) `Map.get(k).cloned().unwrap()` read-context** → `.get(&k).unwrap()` (`&V`)
when consumed by parse/format/comparison, and **`.copied()` for Copy values** (drops
~76 free-clone markers); then **(f) foreach last-use/owned-temporary subset** and **(c)
comparison/condition operands**. ALWAYS measure clones (over `/tmp/audit-<c>` only — §1
GOTCHA) + all-12 errors; one measurement job at a time (concurrent runs share the
`/tmp/audit-*` dirs and corrupt counts). Borrowed-returns (§4.3) stays CLOSED.
