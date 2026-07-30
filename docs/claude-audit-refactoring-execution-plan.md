# Claude Audit Refactoring Execution Plan

Date: 2026-07-30

This plan is based on `claude_audits/` plus the current implementation state.
The goal is to stop adding corpus-specific lowering rules and instead converge
on shared identities, analysis facts, and typed fallback surfaces.

## Working Rules

- Measurement comes first: unsupported behavior must be counted, parse-safe, and
  attributable before lower corpus error counts are trusted.
- Java identity comes before Rust naming: semantic keys use Java owners, names,
  erased parameter types, callable kind, and staticness. Rust names are emitted
  names only.
- Analysis must not block future global semantic analysis. Prefer indices,
  explicit keys, compatibility maps, and divergence logs over local heuristics.
- `javalibs` owns library and facade behavior. `src/stdlib.rs` should keep Java
  language rules and stable JDK facts, not package-specific knowledge.
- It is acceptable to take a temporary error-count hit when a change makes
  unsupported or semantically unsafe behavior explicit.

## Audit-To-Refactor Map

| audits | refactor theme | priority | note |
|---|---|---:|---|
| `0012`, `0013`, `0018`, `0019` | Parse-safe unsupported output and parse/type metric split | 0 | Bad diagnostics or raw Java syntax can hide every later error in a file. |
| `0014`, `0015` | Silent-drop prohibition for statements and members | 0 | Missing Java constructs must lower to diagnostics, never disappear. |
| `0009` | Java callable identity before Rust names | 1 | Partially done for inherited source methods; external facts/constructors remain. |
| `0010` | One fallibility fixpoint for signatures and `?` | 1 | Skeleton exists; current slice routes exception wrapping through the fixpoint. |
| `0011`, `0016` | Typed facade fallback and no-op facade accounting | 2 | Prevent `Box<dyn Any>` cascades and distinguish intentional no-op facades from missing models. |
| `0007`, `0008` | Shape-tagged lowering and shared analysis access | 2 | Stop rendered-string inspection and local rediscovery of semantic facts. |
| corpus optional-carrier gaps | Central `SlotConversionPlan` authority | 2 | Most repeated borrow/optional mismatch class in the active corpora. |
| `0006`, `0001`, `0002`, `0023` | Binding identity, CFG liveness, clone/reference discipline | 3 | Larger machinery; do after callable/fallibility proves the shared-analysis path. |
| `0003`, `0004`, `0005`, `0020`, `0021`, `0017`, `0022` | Java semantic completeness slices | 3 | Some are already fixed or isolated; keep them behind explicit semantic policies. |

## Execution Shape

Work in large, compatible batches. Each batch should land with focused tests and
one checkpoint commit if it compiles. For migrations away from legacy rules,
compute the old and new answer side by side for at least one focused corpus run
and store divergences as the next work queue.

Do not change algorithms in a way that blocks future global semantic analysis.
Acceptable changes: indices, stable keys, facts, analysis handles, compatibility
maps, and explicit unsupported markers. Avoid adding local rules whose inputs
are rendered Rust strings, simple Java names, or ad-hoc arity matches.

## Batch 1: Close The Measurement And Safety Loop

Status: partially done. Unsupported expressions/statements, summary counters,
centralized dynamic `compile_error!` rendering, and nested downcast type-name
normalization exist, but the parse gate and value-level probes are not complete.

1. Finish parse-safe diagnostic audit.
   - Done for dynamic diagnostic message renderers:
     `src/unsupported.rs`, `src/coercion.rs`, and `src/slot_render.rs` now
     share escaped compile-error helpers.
   - Inspect all `compile_error!` producers in `src/unsupported.rs`,
     `src/coercion.rs`, `src/slot_render.rs`, `src/call_lowering.rs`, and any
     direct raw string emitters found by `rg 'compile_error!' src`.
   - Centralize diagnostic construction in helpers such as
     `compile_error_expr(message)` and `compile_error_stmt(message)` so escaping
     is unskippable.
   - Require escaped Rust string literals for messages containing quotes,
     braces, backslashes, raw-string delimiters, nested type debug payloads,
     and Java source fragments.
   - Tests: quotes in unsupported expressions, nested type downcast names, and
     one unsupported coercion path must produce parseable Rust.

2. Make the parse gate stricter.
   - Current focused script classifies rustc parse-looking errors. Add a true
     parse-only gate when practical, while retaining the conservative classifier
     as a fallback.
   - Keep `parse_failures` separate from type errors in summaries.
   - Tests: a generated parse error increments `parse_failures` and invalidates
     type-error counts for that file.

3. Add value-level smoke probes for silent-wrong issues.
   - Cover string concatenation, unsigned right shift, labelled loops, static
     initializer visibility, text blocks, and `StringBuilder`.
   - These tests must compile and run Java/Rust where possible; compile-only
     gates cannot catch these audit classes.

4. Normalize nested Rust type names before emission.
   - Done for current downcast helpers in `src/coercion.rs`.
   - Change downcast helpers in `src/coercion.rs` to accept `JavaType` or a
     `RustTypeName` newtype rather than raw strings.
   - Route nested names through `src/java_naming.rs` and
     `slot_render::rust_name_for_java_named_type`.
   - Tests: `GuideTree.Node` downcast emits `GuideTreeNode`, and no
     `.downcast::<...>` type argument contains `.`.

## Batch 2: Java Callable Identity

Status: `CallableAnalysis` exists as a shell, `JavaCallableKey` and
`JavaCallableSlotKey` exist, inheritance override suppression uses erased Java
parameter slots, and source usage/signature plans have Java-key compatibility
maps. Many consumers still rely on emitted Rust names.

1. Add source callable keys.
   - Add `JavaCallableKey` and `JavaCallableSlotKey` in a small module such as
     `src/java_signature.rs`.
   - Fields: owner, Java name, erased parameter types, kind, and staticness.
   - Slot keys omit owner and are used only for override/interface slot
     matching.

2. Migrate inheritance first.
   - Done for source inherited instance methods.
   - Touch `src/inherited_members.rs`.
   - Replace direct/inherited method deduping by `(rust_name, arity)` with
     `JavaCallableSlotKey`.
   - Carry the selected child-side emitted Rust name on
     `InheritedInstanceMethodRef` so inherited overloads cannot be emitted under
     a child override's signature.
   - Tests: `Child.f(int)` overrides `Base.f(int)` while still inheriting
     `Base.f(int,int)`; same-arity `f(int)` and `f(String)` stay distinct.

3. Add compatibility maps keyed by Java callable identity.
   - Done for source methods in `UsageFacts` and `SignaturePlan`; constructors
     and external facts still need migration.
   - Touch `src/signature.rs`, `src/usage.rs`, and `src/callable_analysis.rs`.
   - Add `methods_by_java_key` beside existing Rust-name maps.
   - Populate both maps during migration and expose Java-key accessors through
     `CallableAnalysis`.
   - New consumers must use Java keys when owner and parameter facts are known.
   - `usage::CallableKey` can remain the call-graph compatibility target during
     migration, but the source-facing key must carry erased parameter types.

4. Reduce ambiguous arity lookups.
   - Touch `src/lower.rs` methods such as
     `find_declared_method_by_name_and_arity` and source-summary matching.
   - If parameter types are unavailable, arity fallback may remain, but
     same-name/same-arity ambiguity must return no confident match rather than
     choosing the first method.
   - Diagnostics should report the resolved Java key and selected fact source.

## Batch 3: Fallibility As One Solved Relation

Status: skeleton exists. `ProgramFallibilityPlan` is built over the existing
call graph, seeded from current method/external method throw facts, propagated
to a fixpoint, and exposed through `CallableAnalysis`. The current checkout also
routes `ProgramExceptionPlan` through the fallibility fixpoint so source method
`Result` wrapping and source-call `?` insertion can agree for propagated thrown
facts. Remaining work is to remove legacy fallibility decisions, add invariants,
and broaden seeds.

1. Add `CallableFallibilityPlan`.
   - Initial skeleton is done under `ProgramFallibilityPlan`.
   - Build over the existing `usage::CallGraph`.
   - Store at least a thrown set plus a derived `returns_result` answer.
   - Seed from declared `throws`, explicit `throw`, builtin/external API
     effects, and unchecked exceptions that escape. Reuse or extend
     `semantic_analysis.rs` thrown-type collection instead of adding another
     scanner.
   - Solve to a fixpoint before signature planning.
   - Unknown external or virtual targets should carry an explicit unknown-target
     reason; initially this may conservatively mark the caller fallible.

2. Route both signature planning and call lowering through the plan.
   - In progress for `src/exception_plan.rs`, `src/lower.rs`, and
     `src/callable_analysis.rs`.
   - Next touch points: `src/signature.rs`, `src/call_lowering.rs`, and any
     direct `wrap_returns_in_result` or `returns_result` recomputation sites.
   - Keep `MethodExceptionPlan { wrap_returns_in_result }` as the compatibility
     wrapper while consumers migrate.
   - During migration, compute old and new answers side by side and log
     divergences.
   - Add an invariant: every emitted `?` must be inside a `Result`-returning
     callable and target a `Result`-returning callee.

3. Tests.
   - Unchecked exception propagation through a call chain.
   - Callee with no fallible body must not get `?`.
   - Caller catching the thrown type should not become fallible.

## Batch 4: Typed External And Facade Fallbacks

Status: open. `Box<dyn Any>` and no-op facades still hide missing models or
cause cascading type errors.

1. Replace broad `Box<dyn Any>` fallback with opaque named shims.
   - Generate one Rust type per unmodelled external Java type.
   - Synthesize observed methods with typed slots where known and
     `unimplemented!("... is not modelled")` bodies.
   - Emit one diagnostic per unmodelled type listing observed constructors,
     fields, and methods.
   - Count `unmodelled_types` and `unmodelled_call_sites` separately from
     translator type errors.

2. Make core JDK facade no-ops loud or real.
   - Implement real `StringBuilder`/`StringBuffer` first.
   - Then change remaining generated no-op facade methods from
     `Default::default()` to explicit `unimplemented!()` unless a manifest says
     the no-op is intentional.
   - Add explicit facade body kinds such as `Modeled`, `BenignNoop`, and
     `UnmodelledPanic`; do not infer intent from the returned expression.
   - Keep Hadoop, Spark, and Google Cloud Storage as intentional no-op facades
     for now; GenomicsDB, DISQ, and Picard should not be javalibs facades.
   - Split metrics into benign no-op facade methods and unmodelled fallback
     methods/call sites.

3. Expand signature facts where they remove special cases.
   - Add high-frequency Rust/JDK signature facts for strings, collections,
     iterators, numeric parsing, `Option`, `Result`, `Path`, and `PathBuf`.
   - Prefer facts over tailored rules when they produce the same slot
     conversion.

## Batch 5: Slot Conversion And Rust Shape Tags

Status: open. Borrow/view/optional decisions still inspect rendered Rust text in
places, and nullable carrier adaptation remains a repeated failure source.

1. Make lowered expression shape mandatory at new boundaries.
   - Use `LoweredExpr { expr, shape, actual_slot }` or equivalent for migrated
     lowerers.
   - Keep `RustExpr::Raw`, but require helper construction with a declared
     shape for new code.

2. Centralize borrow/view/optional adaptation.
   - `SlotConversionPlan` should decide `Some(value)`, `Some(&value)`,
     `.as_ref()`, `.as_deref()`, `.cloned()`, borrow recovery, clone recovery,
     or unsupported.
   - Remove one-off optional and `&` rules after each migrated path has tests.

3. Add drift guards.
   - Add tests that reject new `emit_expr(...)` followed by
     `starts_with`/`ends_with`/`contains` in lowering/coercion code, except for
     documented legacy allowlist entries.
   - Track the baseline count of `RustExpr::Raw` and string-inspection sites so
     improvements are measurable.

## Batch 6: Binding Identity, CFG, And Clone Discipline

Status: design-heavy; do after callable/fallibility routing proves the shared
analysis path.

1. Introduce `BindingId`.
   - Cover params, locals, catch params, enhanced-for variables, loop variables,
     and future pattern variables.
   - Resolve `Expr::Var` to binding identity before lowering.
   - Keep name fallback only as a migration compatibility path.
   - Preserve `instanceof` pattern variables as bindings while parsing; flow
     scoping can be phased in after the binding identity exists.

2. Build per-callable CFGs.
   - Include loop back edges, labelled break/continue, switch case edges,
     return edges, catch/finally edges, and constructor delegation boundaries.
   - Solve liveness over `BindingId`.
   - Replace optional name-keyed sets in `LowerCtx` with a non-optional query
     surface.

3. Centralize clone recovery.
   - Add `CloneRecoveryDecision` over source expression, actual shape, target
     shape, liveness, and capability facts.
   - Any emitted clone for a source object must record a `Clone` obligation.
   - Add clone justifications such as `DeadAfterUse`, `CopySemantic`,
     `ImmutableValue`, `RepresentationSharedHandle`, and `AuditRequired`.
   - Do not treat clone insertion as Java aliasing correctness.

4. Object representation follow-up.
   - Use erased/dyn/closed-enum representation for source hierarchies that need
     runtime `instanceof` or downcast.
   - Non-static inner class construction remains blocked until outer capture and
     Java reference representation are modelled together.

## Deferred Semantic Slices

- Java arithmetic policy: wrapping integer overflow, masked shift distances,
  Java-compatible divide/remainder exception behavior.
- Text block normalization according to Java rules.
- `instanceof` pattern variables with real flow-scoped bindings.
- Generics and bounds from parsed type-tree structure instead of source-text
  reparsing.
- Method-level type parameters and trait/facade projection for bounds.

## Parallel Work Split

- Worker A: parse gate, coverage counters, and value-level silent-wrong probes.
- Worker B: remaining Java callable identity consumers: constructors, external
  facts, source-summary matching, and ambiguous arity fallback removal.
- Worker C: fallibility invariant, legacy decision removal, catch boundary
  handling, and divergence report.
- Worker D: opaque external shims, no-op facade accounting, and `javalibs`
  manifest-backed facade body kinds.
- Worker E: slot conversion shape tags, optional-carrier adapter expansion, and
  string-inspection drift guard.
- Worker F: BindingId/CFG design spike with no lowering behavior change except
  divergence logging.

Integrate in batch order. At each checkpoint run:

- `cargo fmt --check`
- `cargo check --quiet`
- focused tests for touched audit files
- `git diff --check`

## Immediate Next Checkpoints

1. Commit the current fallibility routing slice after its focused tests pass.
   This closes the first usable version of audits `0010` and `0008`: lowering
   has one shared fallibility answer available through `CallableAnalysis`.
2. Add the `?` invariant and legacy-vs-fixpoint divergence logging. This makes
   remaining fallibility drift actionable instead of surfacing as Rust type
   errors.
3. Finish callable-key migration for external facts and constructors. This
   directly supports the original stable-callable-key goal and prevents future
   javalibs facts from matching by simple name or accidental arity.
4. Land parse/unsupported gates before the next broad corpus comparison. This
   prevents silent drops and parse failures from making an error count look
   better than the translation really is.
5. Start facade fallback/body-kind work only after the measurement counters are
   in place, so intentional Hadoop/Spark/GCS no-ops are not confused with
   unmodelled libraries.

At batch boundaries, rerun focused corpuses and update the ranked remaining
error causes rather than only raw rustc message counts.
