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
- HEAD `7cfb2a6` (last-use move already committed). **Uncommitted on the tree** (two
  KEEPs, ready to commit; see §3): the **nullable-unwrap last-use move** (clones −78,
  errors −1) and the **read-only-method-receiver `.as_ref()` borrow** (clones −466,
  errors −12) in `src/dump.rs`.
- **12-corpus error baseline** (current working tree; `tools/<name>_check.sh`):
  trim 187 · jaligner 56 · jahmm 408 · varscan 56 · fastq 53 · bjaaprop 98 · vcf 449
  · bjalign 593 · bioformats 15 · jhlabs 1338 · jsoup 2582 · jts 5549  (**= 11384**).
- **Clone-marker baseline** (`grep -rho 'validate added clone'` over fresh translation):
  trim 469 · jaligner 141 · jahmm 214 · varscan 890 · fastq 38 · bjaaprop 436 · vcf 387
  · bjalign 423 · bioformats 1038 · jhlabs 966 · jsoup 1648 · jts 4417  (**= 11067**).
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
**read-only-method-receiver `.as_ref()` borrow** (clones −466, errors −12, uncommitted):
first slice of the §6 use-site-borrow analysis. New `is_readonly_method_receiver` +
`is_readonly_java_method` (conservative `&self` Java-method whitelist) in `dump.rs`; at
the nullable-name read site (3217), a non-Copy read whose parent is a whitelisted
read-only method call emits `.as_ref().unwrap()` (yields `&T`, zero clones; the call
autorefs) instead of `.clone().unwrap()`. Ordering: last-use move > as_ref borrow > clone.

## 4. Open work — in dependency/priority order
1. **Clone-pattern audit (task #40) — IN PROGRESS, the active lever.** A 6-agent
   parallel audit (all 12 corpora) converged on ONE root pattern with the most
   leverage: **a nullable read emitted in a borrow-only use-site should borrow through
   the Option (`.as_ref()`/`.as_mut()`), not clone+unwrap.** This is the §6 use-site
   borrow analysis, applied as a sequence of LOCAL, measured slices. The first slice
   (read-only-method receiver, NAME site only) landed clones −466 / errors −12 (§3).
   **Queued slices (each its own measured KEEP; ordered by confidence × leverage):**
   - (a) **Extend the as_ref-receiver fix to FIELD nullable-read sites 4910
     (`this.field`) and 3097 (inherited field).** Same helper; `as_ref()` borrows
     `&self` immutably so it's safe behind `&self`. Adds the bioformats/jhlabs field
     receivers the NAME-only slice missed. **Do this next — smallest, proven mechanism.**
   - (b) **Index-base nullable read** `x.clone().unwrap()[i]` → `.as_ref().unwrap()[i]`
     (read) / `.as_mut().unwrap()[i]` (write target). ~500+ markers; jhlabs pixel/array
     code dominates. The write-target form (`do_hsv.clone().unwrap()[0] = …`) also fixes
     a **real correctness bug** (mutation lost to a discarded clone). Detect at
     `ArrayAccessExpr` (2634); `as_mut` only when the access is an assignment target AND
     the Option is `&mut`-reachable (local, or `&mut self` field) — else keep clone.
     Caveat: a non-Copy element read into an owned slot still needs the element clone;
     don't strip that.
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
Commit the two uncommitted KEEPs (nullable-unwrap last-use move; read-only-method
`.as_ref()` borrow) + docs, if not done. Then continue the **clone-pattern audit
(§4.1, task #40)** down the queued slice list — **start with slice (a)**: extend the
proven `is_readonly_method_receiver` → `.as_ref().unwrap()` fix to the FIELD nullable
sites (dump.rs 4910 `this.field`, 3097 inherited). Same helper, smallest next step;
build → re-translate → measure clones + all-12 errors → KEEP iff clones down & zero
per-corpus error regression. Then (b) index-base (also fixes a lost-write correctness
bug), (e) LazyLock receiver, (f) foreach last-use subset. Borrowed-returns (§4.3) stays
CLOSED for clones.
