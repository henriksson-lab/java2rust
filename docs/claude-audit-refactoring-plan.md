# Claude Audit Refactoring Plan

Date: 2026-07-30

This plan triages `claude_audits/` as design input, not as unquestioned truth.
It should guide the next refactoring batches before more corpus-specific fixes.

## Direction

The audits strengthen the current convergence plan: compile-error count is a
necessary signal, but it misses silent wrong translations. The next batches
should therefore prioritize making omissions loud, then route repeated
borrow/clone/fallibility/identity decisions through shared facts or analyses.

Keep the standing constraint: do not make algorithm changes that block future
global semantic analysis. Prefer indices, explicit identities, and reusable
analysis handles.

## Phase 0: Make The Metric Honest

Goal: unsupported or unmodelled behavior must be visible as a diagnostic,
parseable Rust, or an explicit coverage counter. It must not silently disappear
or return `Default::default()` as if implemented.

1. Add a generated-Rust parse gate to focused corpus scripts.
   - Report parse failures separately from type-check errors.
   - Treat parse failures as invalidating per-file type-error counts.
   - Touch points: `scripts/translate-focused-package.sh`,
     `scripts/sweep-generated-idioms.py`, summary/status writers.

2. Make unsupported diagnostics parse-safe and regression-tested.
   - Current `compile_error!({:?})` uses Rust string escaping in several paths,
     but keep a regression for messages containing quotes and nested type debug
     output.
   - Touch points: `src/unsupported.rs`, `src/coercion.rs`,
     `src/slot_render.rs`, `src/call_lowering.rs`, invalid-pattern scanner
     tests.

3. Stop silent statement/member drops.
   - Any `Stmt::Unsupported` or unsupported member must lower to a parse-safe
     `compile_error!`, not be filtered out.
   - Add tests using one deliberately unsupported statement and one unsupported
     member.
   - Touch points: `src/parser.rs`, `src/lower.rs`, `src/unsupported.rs`.
   - In current code, class/interface/anonymous body parsing still has ignore
     paths for unknown member nodes. Convert those into `Member::Unsupported`
     so they are counted and surfaced.

4. Add coverage counters.
   - Track `parse_failures`, `unsupported_statements`, `unsupported_members`,
     `unmodelled_types`, `unmodelled_call_sites`, and `noop_facade_methods`.
   - This keeps unmodelled AWT/Swing and no-op facade coverage from being
     mistaken for translator quality.

## Phase 1: Cheap Silent-Correctness Fixes

These are small enough to batch, and they convert silent wrong output into
correct output or explicit diagnostics.

1. Labelled statements.
   - Parse `labeled_statement`, attach the label to `Stmt::For`,
     `Stmt::While`, `Stmt::DoWhile`, `Stmt::EnhancedFor`, or block equivalents.
   - Rust IR already carries loop/break/continue labels, but Java labelled
     blocks differ from Rust loop labels: `break label` can target a labelled
     block, while `continue label` can target only loops. Labelled blocks need a
     synthetic one-shot labelled loop wrapper.
   - Review `loop_lowering::assemble_for_loop`; current Java `for` lowering
     becomes a `while`/block shape, and labelled `continue` must still target
     the correct Rust loop.
   - Tests: labelled `break`, labelled `continue`, labelled loop with no jump,
     and labelled block with `break`.

2. Unsigned right shift.
   - Add `BinaryOp::UShr`, parse `>>>` and `>>>=` distinctly, lower through
     unsigned carrier casts.
   - Keep `>>` arithmetic.
   - Tests: `-1 >>> 8`, `-1 >> 8`, `-1L >>> 8`, compound `>>>=`.

3. String concat flattening.
   - Flatten only `+` nodes whose own operands make that node a string concat.
   - Tests: `"x" + (a + b)` versus `"x" + a + b`.

4. Static initializer blocks.
   - Minimum first batch: parse and emit a loud diagnostic rather than dropping.
   - Full follow-up: represent static initializer blocks as ordered class-init
     members and emit a `__class_init` guarded by `Once`/thread-local state.

5. Core facade no-ops.
   - Implement real `StringBuilder`/`StringBuffer` first.
   - Then make remaining generated no-op facade bodies loud with
     `unimplemented!("... is not modelled")` plus counters.
   - Keep broad GUI frameworks out of `javalibs` unless explicitly scoped.

## Phase 2: One Solved Relation End To End

Start with fallibility, because it is smaller than liveness and already has a
call graph.

1. Build a `CallableFallibilityPlan` keyed by stable callable identity.
   - Seed from declared `throws`, explicit `throw`, builtin/external pack
     effects, and unchecked exceptions that escape.
   - Solve to a fixpoint over `usage::CallGraph`.
   - Virtual/interface calls join over possible implementations; unknown
     dispatch is conservative.

2. Make signature planning and call lowering read only this plan.
   - Return `Result<T, JavaException>` iff the callable is fallible.
   - Emit `?` iff the callee is fallible and the exception is not caught.
   - Add a debug assertion: every emitted `?` must be inside a `Result`-returning
     callable and target a `Result`-returning callee.

3. Keep old exception heuristics temporarily as divergence logging.
   - Corpus divergences become the work queue.
   - Delete local fallibility decisions in batches after parity.

## Phase 3: Identity Before More Heuristics

The audits point to two identity defects: method identity and binding identity.
Do method identity before inherited/overload corpus work. This phase comes
immediately after Tier 0/cheap silent-correctness work because later corpus
fixes built on Rust-name identity will keep drifting.

1. Java method signature identity.
   - Introduce a Java method key using owner, Java name, erased parameter types,
     staticness, and kind.
   - Use it for inheritance, override detection, interface implementation, and
     source-summary matching.
   - Assign overload-safe Rust names after the resolved Java signature set is
     known.
   - Touch points: `src/inherited_members.rs`, `src/signature.rs`,
     `src/usage.rs`, selected call lookup helpers in `src/lower.rs`.
   - Primary test: child overrides `f(int)` while inheriting `f(int,int)`.

2. Add a `CallableAnalysis` shell before deeper dataflow.
   - Introduce `src/callable_analysis.rs` or equivalent and pass one analysis
     handle through `LowerCtx`.
   - Initially wrap existing move-use, null-refinement, shape, and fallibility
     queries without changing behavior.
   - Make missing analysis explicit rather than treating
     `Option<&HashSet<_>>` absence as `false`.

3. Binding identity.
   - Add `BindingId` for params, locals, catch params, loop variables, and
     fields.
   - Resolve `Expr::Var` to binding identity before lowering.
   - Keep name-based liveness as a compatibility path until the CFG analysis is
     ready.

## Phase 4: Replace Move/Clone Guesswork With CFG Liveness

Goal: one non-optional query surface replaces `later_read_names`,
`later_mutated_names`, `catch_future_use_names`,
`exclusive_terminal_branch_read_names`, and repeated current-statement scans.

1. Build per-callable CFGs from Java IR.
   - Include loop back edges, labelled break/continue, switch case edges,
     returns, catch/finally edges, and constructor delegation boundaries.

2. Solve backward liveness over `BindingId`.
   - Expose `is_live_after(binding, program_point)` and
     `use_count_at(program_point, binding)`.
   - Initially compute old and new results in parallel and log divergences.

3. Centralize clone recovery.
   - Add `CloneRecoveryDecision` over source expression, actual shape, target
     shape, liveness, and capability facts.
   - Decisions: none, borrow, clone, copy, to_vec, clone_box, unsupported.
   - Any emitted clone for a source type must record a `Clone` obligation, but
     this is a compile fix only and must not be confused with Java aliasing
     correctness.

## Phase 5: Optional Carriers Through Slot Plans

This remains the largest active corpus defect family and matches recent VCF
work. Continue it, but make each new fix reduce local optional rules.

1. Make nullable producers return structured lowered values.
   - Map get, field getter, optional facade return, constructor recovery,
     ternary/null branches, array/index reads, and source getter calls should
     return `LoweredExpr { expr, actual }` or the existing equivalent.

2. Give `SlotConversionPlan` final authority.
   - Local code may identify the Java semantic producer, but final spelling for
     `Some(value)`, `Some(&value)`, `.as_ref()`, `.as_deref()`, `.expect(...)`,
     `.cloned()`, and clone recovery should be planned centrally.

3. Keep recent VCF enum `valueOf` work as a migration pattern.
   - The final fix was not an owner-specific table; it added slot-aware
     lowering so optional `Map.get` into enum `valueOf` uses the borrowed string
     slot even during optional assignment lowering.

## Phase 6: Fallback Shapes And Unmodelled Libraries

Avoid broad `Box<dyn Any>` fallbacks that fail later and hide the cause.

1. Generate opaque named shims for unmodelled external types.
   - Synthesize observed methods with typed arguments/returns where known and
     `unimplemented!()` bodies.
   - Emit one diagnostic per type with the observed call surface.
   - Count these separately from translator type errors.

2. Do not apply this blindly to source hierarchies.
   - A broad source `ConcreteStruct -> ErasedObject` fallback was tested for the
     reduced VCF hierarchy issue and regressed the focused subset because
     erased-object method facade emission was missing.
   - Source hierarchy runtime `instanceof` needs either concrete runtime-check
     support or a paired erased-object source method facade.

## Phase 7: Generics And Bounds

Defer this until method identity is stable, then do a dedicated probe pass.

1. Replace type-parameter name lists with typed bounds.
   - `TypeDecl` and `MethodDecl` currently carry type parameter names; model
     declared bounds explicitly.
   - Keep method type parameters in emitted function generics.

2. Project Java bounds to Rust traits/facades.
   - Interface-as-bound must use the trait surface, not the value struct.
   - Inferred bounds such as `Clone` are additive, not replacements.

3. Focus tests.
   - `T extends Comparable<T>`
   - method `<U> U identity(U u)`
   - multiple type params with interdependent bounds.

## Phase 8: Rust IR Shape Tags

Do not start with a big-bang `RustExpr::Raw` rewrite. Add mandatory tags at
new lowering boundaries and migrate consumers.

1. Introduce `LoweredExprShape`/`RustExprShape`.
   - Tags: borrowed, mutable borrowed, owned recovery, optional projected,
     slice view, string view, result propagated, arithmetic expression, cast.

2. Make new raw expression constructors require a tag.
   - Existing raw constructors can be legacy-compatible temporarily.
   - New decisions must query tags or slot shapes, not rendered text.

3. Add a drift guard.
   - Track `emit_expr` followed by `contains`/`starts_with`/`ends_with` in
     lowering/coercion and fail on new occurrences once the first migration
     slice lands.

## Audit Mapping

| Audit | Plan owner | Why it belongs there |
|---|---|---|
| 0018 compile metric blind spots | Phase 0 | The corpus metric must stop rewarding erased code before the remaining counts are trustworthy. |
| 0012 invalid unsupported diagnostics | Phase 0 | Parse failures hide every downstream type error in that generated file. |
| 0013 nested downcast type names | Phase 0 | This is another parse gate contaminant, and should be fixed with central type-name normalization. |
| 0014 labelled statements deleted | Phase 1, then Phase 4 | First preserve/diagnose labels; CFG liveness later needs labelled control-flow edges anyway. |
| 0015 static initializer blocks dropped | Phase 1 | Loud diagnostic first; ordered class-init lowering can follow once omissions are visible. |
| 0005 unsigned right shift | Phase 1 | Parser currently destroys the operator distinction, so the fix must start in Java IR. |
| 0004 string concat absorbs arithmetic | Phase 1 and Phase 8 | Exact local fix now; long-term prevention is expression shape tags rather than rendered-string probing. |
| 0016 no-op JDK facades | Phase 1 and Phase 6 | Implement bounded core facades, then count/report deliberately unmodelled facades. |
| 0010 fallibility disagrees | Phase 2 | Best first end-to-end solved relation because the call graph already exists. |
| 0009 overload identity collapse | Phase 3 | Inheritance and override matching need Java callable identity before more corpus fixes. |
| 0006 move analysis fragmented | Phase 3 and Phase 4 | Binding identity comes first; CFG liveness replaces name-keyed repeated scans. |
| 0001 loop back edges invisible | Phase 4 | It is a direct consequence of no CFG liveness. |
| 0002 object aliasing/clones | Phase 4, then later object model work | Clone obligations can reduce compile failures, but preserving Java aliasing needs a larger representation change. |
| 0011 dyn Any fallback cascades | Phase 6 | Opaque named shims make unmodelled external APIs explicit and stop one missing type from cascading. |
| 0017 generic bounds dropped | Phase 7 | Bounds should be addressed after callable identity is stable. |
| 0022 source-text type reparsing | Phase 7 | Typed bounds require preserving type parse-tree structure. |
| 0019 raw Java expression fallback | Phase 8 | Unhandled expressions need typed unsupported nodes, not raw source strings. |
| 0007 opaque Rust strings | Phase 8 | Shape tags are the guard against recreating borrow/operator/string heuristics. |
| 0008 inference does not reach lowering | Cross-cutting | Each phase should route a relation through a shared analysis handle rather than adding another lowering predicate. |
| 0003 integer arithmetic semantics | Arithmetic semantics follow-up | Needs a policy/fact layer for Java arithmetic; do after `>>>` so shifts do not stay collapsed. |
| 0020 text block processing | Literal semantics follow-up | Small, self-contained correctness fix; lower priority than silent deletion and identity/fallibility. |
| 0021 instanceof pattern binding | Pattern semantics follow-up | Needs binding introduction in expression lowering; do near BindingId work. |
| 0023 inner-class outer capture | Object model follow-up | Runtime correctness depends on the object aliasing/reference representation decision. |

## Near-Term Batch

Recommended next implementation batch:

1. Phase 0 parse/unsupported visibility and safe diagnostics.
2. Phase 1 labelled statements, unsigned right shift, string concat, and static
   initializer diagnostics.
3. Phase 3 Java method signature identity and `CallableAnalysis` shell.
4. Phase 2 fallibility plan skeleton with divergence logging, not full deletion
   of old heuristics.

Parallel split:

- Worker A: diagnostics normalization, parse gate, nested downcast type-name
  normalization.
- Worker B: labelled statements and silent unsupported member/statement
  surfacing.
- Worker C: Java method signature key design, inherited overload regression,
  and `CallableAnalysis` shell.
- Worker D: unsigned right shift parser/lowering slice and string concat
  flattening tests.
- Main checkout: integrate in that order and keep corpus reruns at batch
  boundaries.

After that batch, rerun:

- focused audit repro tests;
- `cargo fmt --check`;
- `cargo check --quiet`;
- relevant `idiom_gaps` filters;
- one reduced VCF rerun and one FastQC subset rerun.

## Non-Goals For This Plan

- Do not model AWT/Swing deeply to make FastQC GUI code work.
- Do not model true Spark/DISQ distributed execution or GenomicsDB native
  behavior in this Java translator.
- Do not replace Java reference semantics with clone insertion and call it
  complete; clone-obligation tracking is only a compile-quality stopgap.
- Do not let a lower corpus error count override silent correctness checks.
