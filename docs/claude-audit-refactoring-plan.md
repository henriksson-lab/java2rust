# Claude Audit Refactoring Plan

Date: 2026-07-30

This plan treats `claude_audits/` as design input, not as unquestioned truth.
The audits are most useful when they steer us away from corpus-instance rules
and toward reusable facts, identities, and lowering plans.

## Current Read

Several audit findings are already fixed in the current checkout:

- `0004-string-concat-absorbs-nested-arithmetic.md`: parenthesized numeric `+`
  is no longer flattened into string concatenation.
- `0005-unsigned-right-shift-collapsed-to-arithmetic-shift.md`: `>>>` and
  `>>>=` now survive parsing and lower through unsigned carrier casts.
- `0014-labelled-statements-are-silently-deleted.md`: labelled loops and
  labelled blocks are preserved and lowered.
- `0001-loop-back-edge-invisible-to-move-analysis.md`: method/body lowering now
  consumes a shared liveness result for live-out reads, so LoopMove-style
  repeated owned consumes across a loop back edge receive clone recovery.
- `0015-static-initializer-blocks-are-silently-dropped.md`: static initializer
  blocks are parsed into the Java IR and lowered through a guarded
  `__java_class_init()` that preserves textual ordering for static field
  initializers and `static {}` blocks.

Those fixes reduce the silent-wrong-output risk, but the audit set still points
to a larger refactor: make omissions measurable, then route call, borrow,
fallibility, shape, and facade choices through shared facts instead of local
string/name heuristics.

Keep the standing constraint: do not make algorithm changes that block future
global semantic analysis. Prefer explicit keys, indices, compatibility shims,
and divergence logs.

Operating invariant: a Claude audit is not done until its issue file has been
moved to `/home/mahogny/github/claude/new_java2rust/claude_audits/done`.
Whenever an audit-backed fix is completed, move the corresponding
`claude_audits/*.md` file into that `done/` directory in the same checkpoint as
the code, tests, and plan update. Do not leave completed audit files in the
active audit directory.

Standing closeout checklist: every audit-closing batch must include the audit
file move to `claude_audits/done/` before commit; partial or inspiration-only
audit files remain in `claude_audits/` until their full issue is verified.

## Audit File Definition Of Done

Audit file housekeeping is part of the work, not a later cleanup step. Once a
Claude audit has been fully implemented and focused verification has passed,
move its issue file from `claude_audits/` to
`/home/mahogny/github/claude/new_java2rust/claude_audits/done` in the same
checkpoint as the implementation, plan-status update, and verification notes.

Do not mark an audit row complete, describe an audit as closed, or commit an
audit-closing checkpoint while its issue file still lives directly under
`claude_audits/`. Leave partial, exploratory, or inspiration-only audit files in
the active directory and record the remaining open scope instead.

## Next Batch: Measurement And Unsupported Safety

Goal: unmodelled behavior must be visible as a diagnostic, parseable Rust, or a
coverage counter. It must not silently disappear or look implemented.

Landed in this batch:

- Parser expression fallback now emits `Expr::Unsupported` for unknown
  expression node kinds, malformed expression nodes, and missing required
  expression children that previously used plausible raw placeholders.
- Parser fallback collectors in both parser-focused test suites now record
  `Expr::Unsupported`, so unsupported expressions are visible to tests instead
  of only raw snippets being counted.
- Direct `compile_error!` producers in `src/coercion.rs`,
  `src/slot_render.rs`, and `src/call_lowering.rs` route through the shared
  escaped `unsupported::compile_error_*` helpers. Focused regressions cover
  quoted and backslash-containing diagnostic text.
- Follow-up source audit moved the remaining unsupported-expression
  `compile_error!` producers in `src/conversion_lowering.rs`,
  `src/stream_lowering.rs`, and `src/expr_render.rs` through the same helpers.
  A source-inspection regression now allows direct `compile_error!` literals
  only in `src/unsupported.rs`, the batch metric scanner, and typed
  static-constant sentinel fallback generation.
- Generated-Rust parse-failure scanning now lives in
  `scripts/focused-parse-failures.sh`, with fast helper coverage and an
  end-to-end focused-package wrapper test proving parse failures increment
  `parse_failures` and set `cargo_type_errors_valid=0`.
- Unknown named block statements are surfaced as `Stmt::Unsupported`; focused
  parser probes cover both `assert_statement` and `yield_statement`.
- Parser-local Java expression fallbacks no longer construct `Expr::Raw` for
  invalid numeric literals, invalid hex floating literals, or expression-helper
  parse failures; these now surface as `Expr::Unsupported`.
- Java switch expressions with simple arrow expression arms or colon/yield arms
  now parse as expression IR and lower to Rust `match` expressions. Switch
  expression selectors, labels, and arm values are traversed by type, usage,
  resolution, semantic-analysis, signature, facade-support, and stream-support
  visitors instead of becoming raw Java or disappearing from analysis.
- `StringBuilder`/`StringBuffer` now use stateful JDK facade support backed by
  shared buffer storage instead of unit structs with default-returning
  `append`/`toString`/`length` stubs. Constructor lowering uses explicit arity
  names (`new_0`, `new_1`) for these overloaded Java constructors.
- `Pattern`/`Matcher` now use lightweight stateful JDK facade support for
  literal `Pattern.compile(...)`, `matcher(...)`, `matches()`, and `group(...)`
  paths instead of always emitting inert `Default::default()` facades.
- `Arrays.copyOf(...)` and `Arrays.copyOfRange(...)` now lower through native
  shared copy projections, including Java-style padding with default element
  values for longer requested lengths/ranges, instead of depending on an
  unresolved `Arrays` facade call.
- `Arrays.toString(...)` now lowers through a native shared formatting
  projection with Java-style bracket/comma rendering for primitive, string, and
  empty array values, instead of depending on a generated no-op `Arrays`
  facade.
- `Arrays.deepToString(...)` now lowers nested array carriers through a shared
  recursive formatting helper instead of a generated no-op `Arrays` facade.
- `Arrays.deepEquals(...)` now lowers nested array carriers through a distinct
  recursive equality helper, preserving the shallow/deep distinction from
  ordinary `Arrays.equals(...)`.
- `Arrays.deepHashCode(...)` now lowers nested array carriers through a
  recursive Java-seeded hash helper, matching deep traversal without a generated
  no-op `Arrays` facade.
- `Arrays.stream(array, from, to)` now carries explicit source-range facts in
  stream planning and renders Java range checks before iterating the selected
  slice.
- `Arrays.fill(...)` now plans both whole-array and range statement overloads
  as native mutating Rust operations, and `java.util.Arrays`/`JavaUtilArrays`
  canonicalize to the builtin `Arrays` identity so facade support does not
  synthesize duplicate no-op overloads.
- `Arrays.equals(...)` now lowers whole-array and subrange overloads through a
  Java-compatible array equality helper instead of plain Rust `==`, preserving
  floating `NaN` equality and signed zero distinction.
- `Arrays.sort(...)` now plans both whole-array and subrange statement
  overloads as native mutating Rust operations, with signature mutation
  detection marking the target array for both arities.
- `Arrays.parallelSort(...)` now aliases to the same native sequential Rust sort
  projections for whole-array and subrange overloads, intentionally erasing JDK
  parallel execution while preserving sorted results and range validation.
- `Arrays.binarySearch(...)` now lowers whole-array and subrange overloads
  through native Rust slice binary-search projections with Java's negative
  insertion-point result instead of depending on a generated no-op `Arrays`
  facade.
- `Arrays.compare(...)` now lowers whole-array and subrange overloads through
  a native lexicographic helper that returns Java-compatible sign values and
  handles floating `NaN` plus signed zero ordering.
- `Arrays.mismatch(...)` now lowers whole-array and subrange overloads through
  the same Java element comparison helper, returning the first relative
  mismatch index or `-1` without a generated no-op `Arrays` facade.
- Native `Arrays.copyOf`, `Arrays.copyOfRange`, range `Arrays.fill`, and range
  `Arrays.sort` projections now reject Java-invalid negative, reversed, or
  out-of-bounds ranges with explicit runtime panics instead of clamping or
  saturating them into valid Rust ranges. Valid padding and empty no-op range
  cases remain covered by generated idiom tests.
- Focused corpus summaries now emit all planned measurement keys:
  `parse_failures`, unsupported statement/member/expression counts,
  `unmodelled_types`, `unmodelled_call_sites`, `noop_facade_methods`, and
  `stubbed_constants`.
  Unsupported member counts include batch/non-batch translation diagnostics,
  so skipped unsupported members are visible even when no Rust marker is
  generated.
- Java text block literals now run content derivation in the parser before
  lowering: initial line terminators, incidental indentation, trailing
  whitespace, and text-block escapes are normalized into the resulting string
  literal value.
- JDK wrapper numeric limits, size/byte constants, boolean constants,
  `Float`/`Double` special constants, character limits, and `Math.PI/E` now
  emit Java-compatible values instead of primitive zero defaults when their
  facade constant surfaces are generated.
- JDK `GZIPInputStream.GZIP_MAGIC` and AWT `Font.PLAIN/BOLD/ITALIC` now emit
  Java-compatible values instead of generic primitive zero defaults.
- External-pack field facts can now carry an explicit Rust constant value via
  `external-pack-field ... value=...`; built-in HTSJDK/VCF/SAM value facts cover
  `BCF2Codec.SIZEOF_BCF_HEADER`, `IOUtil.GZIP_HEADER_READ_LENGTH`,
  `GeneralUtils.DEBUG_MODE_ENABLED`, `CommonInfo.NO_LOG10_PERROR`,
  `VariantContext.NO_LOG10_PERROR`, and
  `SAMSequenceRecord.UNKNOWN_SEQUENCE_LENGTH`.
- Unknown primitive external static constants now emit a `compile_error!`
  initializer instead of silently using `0`, `0.0`, `false`, or `'\0'`.
- Unknown named/reference external static constants now use the same loud
  `compile_error!` initializer policy unless they are explicit enum variants or
  carry an explicit field value fact.
- `Math.floorDiv(int,int)` and `Math.floorMod(int,int)` now lower through
  explicit Java floor-semantics helpers instead of falling through to an
  undefined lowercase `math` receiver or Rust euclidean remainder semantics.
- `Math.max`, `Math.min`, `Math.round`, and `Math.abs` now carry primitive
  overload shape in `NativeJavaStaticMethod`, selected from argument types when
  expression planning has them. Floating `max/min` and `round` lower through
  Java-semantics helpers; integer `abs` preserves Java overflow behavior with
  `wrapping_abs`.
- `Math.round` now uses the JDK edge-safe floor/difference rule for double and
  float inputs, so values just below `.5` no longer round up accidentally.
- `Math.floorDiv` and `Math.floorMod` now carry overload shape and lower to
  i32/i64 helpers as appropriate, with Java-compatible `MIN_VALUE / -1`
  behavior instead of Rust debug overflow.
- `Math.floorMod(long,int)` now preserves Java's `int` return overload while
  evaluating through the long-width helper, including explicit Rust operand
  casts for mixed-width calls.
- `StrictMath` common numeric calls now use the same native Math owner alias as
  other static math facades, while `StrictMath.IEEEremainder` remains an
  explicit support method.
- Java platform support no longer duplicates the recursive `JavaArraysDeep*`
  helper definitions owned by the string/Arrays runtime support item.
- Common floating `Math` methods `sqrt`, `pow`, `log10`, `ceil`, and `floor`
  now lower through the native static-call planner at call sites, alongside the
  existing `log` and `exp` paths. Receiverless static imports for those native
  Math methods use the same planner, and the broad JDK support generator no
  longer emits fallback `Math` surfaces for native-handled methods.
- Additional floating `Math` methods `sin`, `cos`, `tan`, `asin`, `acos`,
  `atan`, `sinh`, `cosh`, `tanh`, `cbrt`, `rint`, `signum`, `toRadians`,
  `toDegrees`, `atan2`, and `hypot` now use shared native unary/binary f64
  call-plan variants, so Apache Commons `FastMath` aliases get the same
  lowering without new package-specific code.
- Read-only modeled receiver calls now classify as known content reads in usage
  facts, and declared non-null `String` returns keep owned native `String`
  signatures instead of widening to erased `Any` because of nested unknown call
  heuristics.
- External constructor argument facts now participate in actual-binding-type
  source parameter inference for JDK `File`/`FileOutputStream` overloads, and
  owned optional arguments/returns can unwrap through the shared slot conversion
  planner before applying external facade `.into()` adapters. Focused Base64
  proof also keeps the exact `JavaIoInputStream -> InputStream` adapter alive
  through shared-support strip/merge. This narrows audit `0011` but does not
  close it.
- Source signature argument slots now beat looser external-pack projections
  while explicit facade ABI plans keep precedence; declared source `String`
  parameters shed erased-object/slice pressure before signature solving; exact
  `JavaIoByteArrayInputStream(byte[])` constructor facts and
  `JavaIoObjectOutputStream.writeObject(...)` handoff facts route through the
  shared argument-slot planner. Focused FastQC Base64 proof
  `.tmp/corpus-fastqc-base64-single-after-source-string-and-writeobject-r1`
  has `cargo_check=ok` and `cargo_check_errors=0`. This further narrows audit
  `0011`, but the active audit file stays open.
- Apache Commons `FastMath` ownership is modelled as a `javalibs` native static
  owner alias to the existing JDK `Math` planner, so package-specific knowledge
  stays out of `stdlib.rs` while call sites reuse the same math projections.
- Compound assignment values targeting `byte`, `short`, or `char` now apply the
  JLS implicit narrowing cast after the promoted operation. The shared
  assignment-value hook is used by statement, expression, array, and static
  storage assignment paths.
- Integer binary `+`, `-`, `*`, `/`, `%`, `<<`, and `>>` now lower through Rust
  wrapping operations for Java `int`/`long` promoted operands, and `>>>` uses a
  wrapping unsigned carrier shift. This removes debug/release drift for binary
  overflow and masks shift distances. Divide-by-zero still surfaces as a Rust
  panic rather than a catchable `ArithmeticException`.
- Local integral `++`/`--` update expressions now use primitive-aware wrapping
  projections, including the Java char/u16 wraparound path, instead of Rust
  `+= 1`/`-= 1` in debug builds.
- Checked integer division/remainder fallibility now has a shared lowering
  context policy for argument lowering. Shaped and unshaped call arguments use
  the same `Result` propagation versus uncaught-panic decision as returns,
  locals, assignments, and conditions, so native/facade call arguments in
  catchable `try` scopes no longer hardcode `?` outside the shared policy.
  This narrows audit `0003`; keep the audit active until the fuller
  unchecked-exception propagation model is covered.
- Java index-slot lowering now uses the same shared result-propagation policy.
  Array reads, writes, ordered assignment temporaries, ndarray row/column
  access, and sequence `.get(index)` projections no longer lower integer
  divide/remainder inside indexes as uncaught raw Rust operations. This further
  narrows audit `0003`, but does not close it.
- Result-returning facade calls and external constructors now use the same
  context policy instead of hardcoded `?` in direct call/new and slot-conversion
  paths. In a `Result` carrier they propagate; outside one they render the
  shared uncaught-Java-exception panic recovery. This also reduces audit `0010`
  drift, but `0003` and `0010` remain active until the broader fallibility model
  is unified.
- Callable fallibility fixpoint seeding now treats any modeled source/external
  thrown exception as fallible, including unchecked `RuntimeException`-style
  facts. This removes the core `0010` mismatch where call-site lowering emitted
  `?` for an unchecked throw fact while signature/exception planning kept the
  caller/callee as plain-returning methods.
- External callable fallibility now resolves facade Rust-name mappings back to
  their Java API effect facts before seeding the fixpoint or extracting checked
  exception types. This keeps `rust=...` external-pack names from splitting
  throw facts away from the solved callable relation.
- Lowering-side call exception propagation now checks the solved callable
  fallibility relation for external receiver calls before falling back to direct
  fact probes, and the fallback external thrown helper uses the same
  rust-name-aware API effect lookup.
- One-dimensional array compound assignments with side-effecting indices now
  hoist the Java index once, save the old element value before evaluating the
  RHS, and reuse the shared numeric binary/narrowing planners for writeback.
- Parsed-source concrete object representations now reject non-user erased
  hierarchy overrides from external packs, corpus source-summary boundaries,
  and dispatch fallback pressure. Signature planning applies the same selected
  representation principle, so a declared parsed-source concrete peer parameter
  no longer widens to `Box<dyn Any>` solely because stale summary/fallback facts
  mention erased object dispatch.
- 2026-07-31 reduced VCF rerun:
  `.tmp/corpus-rerun-20260731-vcf-signature-erasure-guard-r1` translated 9/9
  files with `parse_failures=0`, `stubbed_constants=0`, and
  `cargo_check_errors=22`, down from the prior 41-error
  parsed-concrete-fallback rerun. The `other.name`/`other.line_type` on
  `Box<dyn Any>` family is gone. Remaining VCF blockers are mainly nullable
  enum switch/match comparisons, value-vs-borrowed concrete argument
  adaptation, concrete/concrete `instanceof`, enum `hash_code`, and the
  existing unsupported member markers.
- 2026-07-31 nullable enum switch checkpoint:
  nullable source enum switch labels now render before bare enum label fallback,
  so match arms and fallthrough `__java_switch_value == ...` guards wrap labels
  as `Some(Enum::Variant)` when the selector storage is optional. Reduced VCF
  rerun `.tmp/corpus-rerun-20260731-vcf-nullable-switch-r1` remains 9/9
  translated with `parse_failures=0`, `stubbed_constants=0`, and
  `cargo_check_errors=12`. Remaining VCF blockers are concrete/concrete
  `instanceof`, stale concrete downcast handover temporaries, enum `hash_code`,
  and subclass-to-super concrete argument conversion.
- 2026-07-31 source enum hash checkpoint:
  source enum instance `hashCode()` now routes through the existing
  `java_value_hash_code` projection even when the enum also has external-pack
  facts, using expression-shape facts to recognize optional enum field
  receivers. Reduced VCF rerun
  `.tmp/corpus-rerun-20260731-vcf-enum-hash-r2` reports
  `cargo_check_errors=9`, with enum `hash_code` errors gone.
- 2026-07-31 source subtype argument checkpoint:
  current-receiver and shaped call argument lowering now apply source
  subtype-to-supertype conversions after clone recovery, including borrowed
  downcast locals that must be cloned before flowing into an owned supertype
  parameter. Focused regressions cover owned, borrowed, and Object-downcast
  source-subtype arguments. Reduced VCF rerun
  `.tmp/corpus-rerun-20260731-vcf-subtype-arg-r3` reports
  `cargo_check_errors=7`, down from 9; the `equals_excluding_description`
  subclass-to-`VCFCompoundHeaderLine` mismatch family is gone. Remaining VCF
  errors are the concrete/concrete `instanceof` runtime-carrier problem and the
  paired stale concrete downcast handover temporaries in
  `VCFStandardHeaderLines`.
- Static initializer blocks now carry member-position metadata in `TypeDecl` and
  lower through `__java_class_init()`. Static storage getters, constructors, and
  static methods trigger class initialization before active use. Focused parser,
  idiom, and design regressions cover parse retention, initializer order, and
  active-use triggers. Audit `0015` is closed and moved to `claude_audits/done`.
- Loop lowering now prefers `LivenessAnalysis::live_out_for(stmt)` over the
  textual suffix scan when deciding clone recovery, and `forEach` lambda bodies
  that lower to loops get their own loop liveness result. The focused
  LoopMove-style regression passes. Audit `0001` is closed and moved to
  `claude_audits/done`; the broader `0006` BindingId/CFG cleanup remains open.
- Fallibility planning now equalizes source override sets in
  `plan_program_fallibility`, with focused coverage for interface/implementation
  signature agreement. This narrows audit `0010`, but the audit remains active
  because lowering still has compatibility fallibility probes that must either
  be retired or checked against the solved relation.
- `RejectTraitObject` projection now preserves already-concrete dispatch shapes
  instead of erasing them to `Box<dyn Any>`. This narrows audit `0011`, but does
  not close the fallback-design issue for genuinely unmodelled externals.

1. Add a generated-Rust parse gate to focused corpus scripts.
   - Report parse failures separately from type-check errors.
   - Treat parse failures as invalidating per-file type-error counts.
   - Touch points: `scripts/translate-focused-package.sh`,
     `scripts/sweep-generated-idioms.py`, corpus summary/status writers.
   - Tests: a generated file with a parse error must increment
     `parse_failures` and not be counted as a clean translation.

2. Finish parse-safe unsupported diagnostics.
   - `src/unsupported.rs` already uses escaped Rust string literals for its
     direct helpers; audit the remaining `compile_error!` producers in
     `src/coercion.rs`, `src/slot_render.rs`, and `src/call_lowering.rs`.
   - Add regressions for messages containing quotes and debug-form nested type
     payloads.
   - Related audits: `0012`, `0013`, `0019`.

3. Replace raw Java expression fallback with unsupported expression nodes.
   - Add an `Expr::Unsupported { reason, span }` or equivalent.
   - Make `parse_expr_node` fallback produce that marker instead of
     `Expr::Raw(text(node, source).to_string())`.
   - Lower it via `plan_unsupported_expression` to parse-safe `compile_error!`.
   - Keep intentional Rust raw snippets behind named helper constructors.
   - Tests: switch expressions and one parser-unknown expression must name the
     unsupported Java node instead of leaking Java syntax.

4. Close remaining silent dropped statements.
   - Audit `parse_block_like` whitelist paths and route unknown named statement
     nodes to `Stmt::Unsupported` instead of ignoring them.
   - Add focused probes for `assert_statement` and `yield_statement`.
   - Related audits: `0014`, `0018`, `0019`.

5. Add coverage counters.
   - Track `parse_failures`, `unsupported_statements`, `unsupported_members`,
     `unsupported_expressions`, `unmodelled_types`, `unmodelled_call_sites`,
     and `noop_facade_methods`.
   - This keeps unsupported AWT/Swing, no-op facades, and generated opaque
     shims from being mistaken for translator quality.
   - Related audit: `0018`.

## Next Batch: Shared Callable Analysis

Goal: create the path that lets later fixes replace local rules with shared
answers. This should be behavior-preserving at first, with divergence logging
where we compute both old and new answers.

Landed in this batch:

- `CallableAnalysis` is present in `LowerCtx` and carries usage, call graph,
  signatures, exception plans, and fallibility plans.
- Lowering has started migrating from direct fallibility/fact probes to shared
  `CallableAnalysis` queries. External method Result-return checks now ask the
  analysis first and keep the legacy external-index scan as a compatibility
  fallback.
- Source method Rust-name projection in usage, call graph, exception planning,
  and lowerer compatibility helpers now goes through
  `java_signature::source_method_rust_name`.
- `CallableAnalysis` now exposes source-method usage, signature, exception, and
  fallibility convenience queries, and the lower-time source Result-propagation
  check uses `source_method_returns_result`.
- The lower-local call exception-propagation wrapper now asks source-call
  fallibility through the shared analysis before delegating to legacy
  external/unresolved call planning.
- Lowering no longer performs direct external API effect lookup when retargeting
  external receiver overload names. It delegates the modeled-overload query to
  `call_plan`, keeping the lowerer on the shared call-planning surface while
  preserving the existing external-index and platform-fallback behavior.
- Source-method usage lookups used by signature solving now read
  `UsageFacts::source_method_usage`, backed by `JavaCallableKey`, for concrete
  method declarations. This prevents same-name overloads from sharing
  name-keyed usage facts while broader compatibility maps are still present.
- Fallibility planning now seeds only from escaping checked exception facts and
  skips call-graph propagation across checked exceptions caught by an
  assignable non-rethrowing catch. The call graph carries a handled-boundary
  edge kind for these cases, so caught `Appendable.append` `IOException` paths
  do not force `Result` signatures when the catch rethrows only known unchecked
  exceptions.
- `LoweredType::OptionalElementBorrow` display and string-concat adaptation now
  flows through `plan_lowered_slot_conversion` via an explicit
  `OptionalBorrowedToJavaDisplayValue` policy. `HashMap.get` display lowering no
  longer carries its own optional-borrow display branch in `lower.rs`.
- Rust function IR now carries structured function generic parameters, and
  source method/constructor type parameters emit as `fn name<T>(...)` instead
  of leaving return/argument `T`/`U` identifiers undeclared. The previous static
  borrowed-return lifetime spelling also uses this structured generic list.
- Ordinary emitted source methods now project simple source interface/class
  bounds into Rust function generics, including nested source interfaces such as
  `<N extends Named>` lowering to `fn call_name<N: OuterNamed>(...)`.
- Raw trait signatures for source interfaces and source-class-bound traits now
  preserve method-level type parameters and resolvable source bounds. Generic
  trait methods are emitted with `where Self: Sized` so existing boxed dyn-trait
  support remains object-safe, and identity-style method type-param returns no
  longer borrow from owned parameters.
- Method and constructor function shells now receive emitted source-bound
  context when lowered, so type parameters with resolvable in-crate source
  interface/class bounds emit Rust bounds such as `fn new<N: Named>(...)`
  without introducing missing trait names from analysis-only context sources.
- Source type parsing now dispatches common tree-sitter type node kinds
  (`array_type`, `generic_type`, `annotated_type`, wildcard, primitive, and
  scoped identifiers) into structured `JavaType` values before falling back to
  the legacy text parser. Parser tests pin annotated generic arguments, scoped
  generic names, generic type-parameter bounds, and the current primary-bound
  behavior for Java multi-bounds.
- Borrowed method receiver parenthesizing and redundant shared-borrow collapse
  now prefer typed `RustExpr::Unary` shapes and keep rendered-string checks only
  as compatibility fallbacks for legacy `RustExpr::Raw` carriers.
- External-pack constructor facts now copy typed
  `external-pack-constructor-argument ... type=...` metadata into the
  constructor fact's `arg_types`, so constructor callable keys can distinguish
  typed slots instead of collapsing to arity-only `Any` patterns. Built-in pack
  constructor indexing stores each fact's callable key and keeps the old
  `(type, arity)` map as compatibility fallback.
- Resolved emitted-source call argument planning now asks
  `SignaturePlan::method_signature_by_java_key(JavaCallableKey::source_method(...))`
  for the selected declaration before falling back to the legacy Rust-name
  required-signature lookup.
- Resolved emitted-source static fallback planning now selects the source
  declaration with the same resolved owner/staticness/arity helper and asks
  `SignaturePlan::method_signature_by_java_key(...)` before the legacy
  `(owner, rust_name)` compatibility lookup.
- Resolved source varargs lowering now asks the same Java-key source-signature
  helper before falling back to the legacy `(owner, rust_name)` signature map.
- Current-callable binding use/count queries now carry the current source
  `MethodDecl` through `LowerCtx` and ask `UsageFacts::methods_by_java_key`
  before falling back to Rust-name overload scans. This removes one more clone
  and move-analysis dependency on emitted overload suffixes.
- Resolved call-argument planning in the fact-aware paths now reconstructs a
  `JavaCallableKey` from the resolved owner, Java call name, staticness, and
  erased target parameter types before consulting legacy Rust-name signatures.
  This keeps overloaded argument slot selection tied to the selected Java
  declaration when the call name is available.
- `forEach` lambda receiverless call lowering now uses resolved overload names
  when available, falls back to signature argument-type selection when lambda
  calls are unresolved, and lowers arguments through the shared
  `ArgumentLoweringPlan` path. Interface-super default-call arguments now use
  the same shared argument lowering instead of a deleted local same-object
  helper.
- Stream lambda receiver-call argument lowering now treats
  `plan_call_arguments_with_facts_and_args` as authoritative for shaped/slotted
  arguments. The old post-plan `external_call_arg_shapes` retry has been
  removed, leaving only generic borrowed-parameter move recovery for genuinely
  unshaped calls.
- Stream lambda argument adaptation no longer retries
  `external_call_arg_shapes_for_resolved` after the shared planner either; the
  resolved external case is expected to arrive as a shaped
  `ArgumentLoweringPlan`.
- Stream method references to `Objects::nonNull`/`Objects::isNull` over
  optional elements now render through the shared `CallTargetPlan`/argument-plan
  path. The older method-reference fallback remains only for non-optional
  inputs where nullness has already been proven by shape.
- Stream call-target rendering now centralizes the external-index-vs-facts
  target planner choice in one `plan_stream_lambda_call_target` helper shared
  by native stream lambda calls and method-reference call-plan lowering.
- External field fact lookup for enum/static-field lowering now goes through
  `member_access_plan::external_field_type_for_owner`; `lower.rs` no longer
  probes external-pack field facts directly.
- External constructor support checks for concrete-slot lowering now go through
  `call_plan::external_constructor_is_modeled`; `lower.rs` no longer probes
  external constructor facts directly.
- External result propagation from shape-env slots now asks
  `call_plan::external_call_returns_rust_result_for_receiver`, so lowering no
  longer opens facade ABI and legacy Rust-result facts directly for that path.

1. Add a `CallableAnalysis` shell.
   - Introduce `src/callable_analysis.rs`.
   - Wrap existing `UsageFacts`, `CallGraph`, `SignaturePlan`,
     `ProgramExceptionPlan`, and selected local shape/nullability lookups.
   - Pass one analysis handle through `LowerCtx`.
   - Make absence explicit. Do not treat `Option<&HashSet<_>>` missing as
     equivalent to "not live" in newly migrated code.
   - Tests: source-level guard that `LowerCtx` carries `CallableAnalysis`, plus
     one behavior-preserving borrow/clone fixture.
   - Related audits: `0006`, `0008`, `0010`.

2. Add Java callable signature identity as the primary source key.
   - Key source methods by owner, Java name, erased parameter types,
     staticness, and kind.
   - Keep Rust names as late emitted names, not semantic identities.
   - Use the key first in inheritance, override detection, source-summary
     matching, usage, and selected lowerer lookups.
   - Compatibility path: continue projecting the key to the current
     `(owner/rust_name/arity)` maps until each consumer is migrated.
   - Tests: child overrides `f(int)` while inheriting `f(int,int)`; unrelated
     same-name method in another package must not match.
   - Related audit: `0009`.
   - Current open precision gap: GATK/HTSJDK source-summary helper signatures
     can still broaden known external facade parameters to erased object before
     call fallback planning runs. Fix this in signature/source-summary type
     resolution rather than by local call-site normalization.

3. Add a fallibility plan skeleton.
   - Build `CallableFallibilityPlan` over the existing `usage::CallGraph`.
   - Seed it from declared `throws`, explicit `throw`, builtin/external effects,
     and unchecked exceptions that escape.
   - First run old and new fallibility answers side by side and log divergence.
   - Then make signature planning and `?` insertion read the same answer.
   - Add an invariant: any emitted `?` must be inside a `Result`-returning
     callable and target a `Result`-returning callee.
   - Related audit: `0010`.

## Next Batch: Slot And Facade Generalization

Goal: reduce the rule count by giving shape/facade facts the authority that
lowering currently re-derives from rendered Rust strings.

Landed in this batch:

- Preconverted optional wrapper construction is now planned by
  `conversion_plan::preconverted_optional_slot_construction_plan`. `lower.rs`
  no longer constructs `OptionalSlotConstructionPlan` directly for optional
  assignment rewraps or optional source-enum field values.
- External builder `build()` support without explicit return facts now falls
  back to an inferred named opaque product shim (`FooBuilder -> Foo`) instead
  of broad `Box<dyn std::any::Any>`.
- Getter-like external API facts without return types now emit named opaque
  result shims with `unimplemented!("Receiver.method is not modelled")` bodies
  instead of silently lowering as `()` or broadening the erased-object facade.
  Void-like side-effect/mutator facts still emit `()` surfaces.
- Imported static getter facts with missing return types now use the same named
  unmodelled result shim path instead of generic `R: Default` fallback methods.
- Imported static methods without return facts now keep their generic return
  shape for type inference but emit a loud `unimplemented!(...)` body instead
  of `R: Default` plus `Default::default()`.
- Typed fact-backed external static/instance methods now use the shared loud
  unmodelled method body unless they hit an explicit modeled helper path, so
  signature facts no longer imply silent default semantics by themselves.
- Builder `get_*_builder` fallback support now uses named opaque result shims
  with loud unmodelled bodies instead of generic `T: Default` fallback methods.
- Inherited external getter facts without return types now use the same named
  unmodelled result shim policy instead of silently generating `()` inherited
  support methods.
- JDK platform facade support now uses typed returns for `URL::open_connection`,
  `Enumeration::next_element`, and AWT `Graphics` getter methods that were
  previously generic `T/R: Default` shims. Casted external call assignment also
  preserves the source call shape when a fact-backed `Into` adapter exists for
  the optional element target, so optional construction can apply the shared
  conversion plan.
- Reused optional unwrap projection now goes through a named
  `ReusedOptionalUnwrapPlan` and `conversion_lowering` renderer before slot
  conversion, so `lower.rs` no longer chooses the `.expect(...)`/element-borrow
  spelling locally for this path.
- Optional assignment rewraps now unwrap structured `RustExpr::Call(Some, ...)`
  values before falling back to raw `Some(...)` parsing, so the shared optional
  construction path no longer depends on rendered text for ordinary optional
  carriers.
- Optional erased-object clone recovery and try-with-resources cleanup panic
  rendering now delegate their Rust spellings to `coercion.rs` helpers, reducing
  the rendered-string surface that `lower.rs` owns while preserving the current
  Java null/uncaught-exception behavior.

1. Expand rust signature facts.
   - Prefer function signature facts over tailored call rules whenever the same
     slot conversion follows from the signature.
   - Cover high-frequency Rust stdlib signatures incrementally, not the whole
     stdlib: string slices/views, collections, iterators, numeric parsing,
     `Path`/`PathBuf`, and common `Option`/`Result` helpers.
   - Use javalibs manifests for facade ABI and package-specific behavior.

2. Centralize borrow/view/optional adaptation in `SlotConversionPlan`.
   - Nullable producers should return a structured lowered value with actual
     slot shape.
   - `SlotConversionPlan` should choose `Some(value)`, `Some(&value)`,
     `.as_ref()`, `.as_deref()`, `.cloned()`, clone recovery, or unsupported.
   - The current optional Java type construction borrow mismatch is a signal
     that this system needs wider coverage, not another one-off borrow rule.
   - Related audits: `0007`, `0008`, htsjdk optional-carrier corpus gaps.

3. Replace broad `Box<dyn Any>` fallbacks for unmodelled externals.
   - Generate opaque named shims per unmodelled external type.
   - Synthesize observed methods with typed args/returns where known and
     `unimplemented!()` bodies.
   - Emit one diagnostic per type with the observed call surface.
   - Count these separately from translator type errors.
   - Related audit: `0011`.

4. Make core JDK facade no-ops loud or real.
   - Implement real `StringBuilder`/`StringBuffer` first.
   - Make remaining generated no-op facade methods emit
     `unimplemented!("... is not modelled")` and increment coverage counters.
   - Keep AWT/Swing out of deep `javalibs` modelling unless explicitly scoped.
   - Related audit: `0016`.

## Next Batch: Binding, CFG, And Clone Discipline

Goal: replace repeated name-keyed move/clone predicates with one query surface.

Landed in this batch:

- `BindingId`/`BindingKind` now exist as explicit binding identity types.
  `ShapeEnv` assigns compatibility ids for fields, static fields, params,
  locals, catch params, and enhanced-for loop variables while preserving the
  existing name-keyed type/shape maps. Catch-body lowering records catch
  variables as `CatchParam` bindings. `MoveUseContext` starts projecting its
  binding use-count query through a `BindingId` before using the legacy
  name-keyed usage facts, creating the migration path for CFG/liveness without
  changing lowering behavior yet.
- Loop-body lowering now threads loop-carried read names through the existing
  later-read query for `while`, `do while`, and `for` bodies, including the
  `claude_audits/repro/LoopMove.java` shape where an owned value is consumed on
  every iteration. This is still a compatibility approximation, not the planned
  per-callable CFG/liveness result.

1. Introduce `BindingId`.
   - Cover params, locals, catch params, loop variables, and fields.
   - Resolve `Expr::Var` to binding identity before lowering.
   - Keep name-based lookup as a compatibility path only while migrating.
   - Related audits: `0006`, `0021`.

2. Build per-callable CFGs from Java IR.
   - Include loop back edges, labelled break/continue, switch case edges,
     returns, catch/finally edges, and constructor delegation boundaries.
   - Solve backward liveness over `BindingId`.
   - Expose `is_live_after(binding, program_point)` and
     `use_count_at(program_point, binding)`.
   - Compute old and new answers in parallel first; divergence is the work
     queue.
   - Related audits: `0001`, `0006`.

3. Centralize clone recovery.
   - Add `CloneRecoveryDecision` over source expression, actual shape, target
     shape, liveness, and capability facts.
   - Decisions: none, borrow, clone, copy, `to_vec`, `clone_box`, unsupported.
   - Any emitted clone for a source type must record a `Clone` obligation. This
     is only a compile-quality stopgap and must not be treated as Java aliasing
     correctness.
   - Related audit: `0002`.

4. Decide object reference representation separately.
   - Java source object aliasing needs a representation that preserves mutation
     through aliases. This is larger than clone obligations.
   - Non-static inner class construction remains blocked until outer capture and
     reference representation are modelled together.
   - Related audits: `0002`, `0023`.

## Deferred Semantic Slices

These are important, but they should follow identity and analysis plumbing so
the fixes do not become more isolated rules.

1. Java arithmetic semantics.
   - Add an arithmetic policy/fact: wrapping overflow for Java integer
     arithmetic, masked shift distances, and Java-compatible divide/remainder by
     zero handling where catchable exceptions matter.
   - Related audit: `0003`.

2. Java text blocks.
   - Preserve Java text-block normalization: remove incidental indentation and
     the initial line terminator according to Java rules.
   - Related audit: `0020`.

3. `instanceof` pattern variables.
   - Parse the pattern variable as a binding.
   - Lower it with the same cast/instanceof plan and bind it only in the valid
     Java flow scope.
   - Related audits: `0021`, `0022`.

4. Generics and bounds.
   - Stop reparsing type nodes from raw text; preserve typed parse-tree
     information for bounds, wildcards, method type params, and nested types.
   - Emit method-level type parameters and project Java bounds to Rust
     traits/facades.
   - Related audits: `0017`, `0022`.

5. Rust IR shape tags.
   - Add mandatory shape tags at new lowering boundaries.
   - Add a drift guard for new `emit_expr(...).contains/starts_with/ends_with`
     patterns once the first migration slice lands.
   - Related audit: `0007`.

## Audit Mapping

Completion checklist for any audit marked `fixed locally`:

- Fresh-context reminder: an audit is not closed by code alone. Its issue file
  must be moved from active `claude_audits/` into
  `/home/mahogny/github/claude/new_java2rust/claude_audits/done` as part of
  the same verified checkpoint as the implementation.
- Note the move in the plan row and, when committing, in the commit summary.
- Do not move partial or exploratory audit files.
- Treat the audit as still open if its file remains outside `claude_audits/done`.
- During batch work, include this file move in the same patch/commit that marks
  the audit fixed; otherwise leave the active audit file in place.

| Audit | Status | Plan owner |
|---|---:|---|
| `0001` loop back edges invisible | done | Closed by shared liveness use in the block walkers; remaining BindingId/CFG/query-surface work belongs to `0006` |
| `0002` object aliasing/clones | open | 0002a done and the divergence is reported (`observable_alias`). The representation work is tracked in `0002b`; the current landed state is still report-only, with storage facts/plumbing kept and the handle representation switch reverted to preserve the gate **The handle representation is now landed** (see `0002b`): source-class slots are `Rc<T>`/`Arc<T>` for the `SharedImmutable` tier, gate at baseline. Aliasing is therefore represented for that tier; what remains is identity lowering (`0028`) and the mutable tiers |
| `0002b` shared representation design | open | The design document `0002` calls for, tracked as its own row because several other audits are blocked on it rather than on `0002`'s measurement: `0023` (inner-class construction panics), `0028` (reference equality lowered structurally), and the residual of `0034`. Steps 1–2 are landed under `0002`; steps 3–5 stay open Attempting step 3 found **two wrong inputs in the plan itself**, both over-approximations on very common shapes. (1) `mutated` counted *constructor* field writes, though the bit is meant to ask whether a value is mutated **after being shared** and construction precedes any alias — so `final`-field value classes, the exact population the design names for `SharedImmutable`, were landing in the harder `RefCell` tier. (2) `escape` charged the **owner** for `this.v = v` "for want of a type environment", but the field's declared type is available right there — so every class whose constructor assigns a parameter to a field was marked `Stored`, including when `v` is an `int`. Fixed: constructor bodies still contribute escape, not mutation; `this.f = x` charges `f`'s type, while qualified fields and array elements keep the conservative charge. Re-measured over the same corpus: distinct types needing sharing **47 -> 22**, and the split inverted — `SharedImmutable` 14 -> **19**, `SharedLocal` (the tier with the `RefCell` hazard and the never-hold-a-borrow-across-a-call rule) **34 -> 4**. Less than half the costed work, with the depth concentrated in four types. Pinned by three tests; `scripts/gate.sh --all` zero deltas, the plan still being report-only. Step A also landed: the storage plan now runs before `plan_representations` and is carried on `RepresentationPlan`, so signature solving and lowering can both read it — verified inert Step 3 attempted and **reverted**, with the blocker measured: wrapping a `SharedImmutable` type in `Rc<T>` at the representation-aware rendering entry point produced **zero deltas** — not "no regressions" but no effect, because the Rust type of a source class is never rendered through a representation-aware path. Counted: 49 call sites use the facts-only rendering variant against 11 that have the plan, and `item_assembly.rs` — which renders **struct fields, parameters and return types** — mentions `representations` zero times. The switch is also atomic: render a field as `Rc<Key>` while a parameter is still `Key` and the output does not compile, so unlike every other migration here there is no monotone half-done state. Next unit of work is therefore threading the plan through all 49 sites **without consulting it** (provably inert, gate-verifiable), and only then flipping the switch for the 19 `SharedImmutable` types together with `0028` identity lowering. The inert hook was removed rather than left in place, since it would read as "started" when nothing reaches it **Step 3 implemented end to end and reverted, with the cost now a list rather than an unknown.** The prior "49 sites" diagnosis was wrong: `FactSet` already reaches all of them and already carries per-type decisions, so the plan moved there for one field and one population line. The acceptance test `tests/differential/known-gaps/Identity.java` **passes against the Java oracle** (both `5,true,false,false`) — 0002's aliasing, 0028's `==` and 0028's `.equals()` half, all satisfied for the `SharedImmutable` tier, in four small pieces. It exposed two soundness defects, both fixed and kept: `mutated` **under**-approximated (it saw only writes through `this`, so `arr[i].f = v` was invisible and the type became `Rc<T>` without a `RefCell` — `error[E0594]: cannot assign to data in an Rc`, which is not the "over-sharing costs an indirection" tradeoff the design assumes), now gated on `mutation_provably_absent`; and enums/interfaces were in the lattice at all, which broke `match` and derived equality and was 10 of the initial 32 failures. Residual cost after both fixes: `idiom_gaps` 538 -> 521, seventeen **diffuse** regressions, each a separate place assuming a source class is a bare struct (subtype upcasts, clone recovery, super-constructor init, self-referential `Box` storage, borrowed getters, facade shells) — enumerated in the audit as a work queue. Switch reverted to keep the gate green; the plumbing and both soundness fixes are kept Seventh pass resolved the `Rc`/`Arc` conflict with a **hierarchy** rather than by picking one: `StoragePlacement` carries three independent axes, and handle and cell are both chosen by the same `sync` bit so they cannot disagree — an invariant the previous `StorageForm` match maintained by hand. `sync` is `threaded ∨ static_reachable`; measured, **zero** corpus types need `Arc`, so the cheap rung covers everything and the atomic rung exists for correctness. Type and value forms are derived together (`wrap` / `wrap_construction`) after a hand-written construction string got the nesting wrong, with laws asserted exhaustively over the 8-element domain. Acceptance test `known-gaps/Identity.java` **passes against the oracle** (`5,true,false,false` both sides). Residual **17 -> 14**, all conversions — and the reason a `HandleAdaptation` beside the plan did not help is now known: `apply_slot_conversion_plan` has **four** callers and two take a bare plan, so a sibling field reached only one. Fixed by moving the crossing **inside** the plan as `SlotConversionPlan::Handled`, landed and inert (gate zero deltas). The next pass flips the four pieces on and re-measures rather than investigating Re-measured with all five pieces on: **15**, so the in-plan crossing did not reduce it, and tracing one failure gives the corrected shape — the downcast is built by the instanceof/cast binding path, which constructs its `RustExpr` **directly** without consulting a plan. `apply_slot_conversion_plan` was never the single choke point. The residual is therefore "teach each expression-building path that produces a value for a slot", of which the cast/instanceof binding is one; a bounded search with a known query. `SlotConversionPlan::Handled` remains the right home for the paths that do plan, kept and inert; gate back to zero deltas Followed the query: the erased-object downcast **does** go through a plan, and the gap was one level up again — the crossing was decided in `plan_slot_conversion` while plans are also built directly by `slot_conversion_plan_with_facts`. Moving the decision into that builder reaches every producer; with step 3 on the residual then got **worse** (15 -> 16), which says the adaptation *rule* is wrong, not the plumbing: `Wrap` when the target is handled and the source is not, `Unwrap` in reverse, is too coarse and damages sites not yet characterised. That is the remaining work and it needs the failure list read one by one, not another structural move. Five attempts, each correcting the diagnosis — the table of what each believed versus what it was is in the audit **Step 3 is landed.** The shared handle is on: source-class slots render `Rc<T>` (`Arc<T>` when reachable from a Java `static`, which must be `Sync`), with `scripts/gate.sh` at baseline on all 18 targets and `idiom_gaps` 538/146 with **identical failure names** — zero new failures. What broke the eight-revert cycle was `0041`: every earlier pass converged against assertion text, so each fix revealed another unwrapped path; with 101 `slot_shape_design` and 323 `idiom_gaps` tests compiling their output, each pass named a *type error* and the list ran dry. The wrap has exactly two type positions (slot top level, ownership-aware; generic argument, unconditional — both recursing at the `Option` **element**) and eight value positions, each with a structural reason the value is known to be bare rather than a guess. Four defects surfaced and were fixed: a handle nested in a collection cost its struct the `Clone` derive; a type parameter sharing a name with a source class got wrapped (`JavaType` has no type-variable variant, so such names are now excluded from the plan); the two slot-type renderers disagreed on `Option` recursion; and the return-slot wrap double-wrapped values that were already handles. Still `SharedImmutable` only — `SharedLocal`/`SharedSync` and their reentrancy discipline remain off, and `0028` identity lowering (`Rc::ptr_eq`) is the next step |
| `0003` integer arithmetic semantics | done | Moved to `claude_audits/done`; wrapping arithmetic, shift masking, catchable `ArithmeticException`, and unchecked propagation through a transitive caller are verified |
| `0004` string concat absorbs arithmetic | fixed locally | Moved to `claude_audits/done`; Rust IR shape tags prevent recurrence |
| `0005` unsigned right shift | fixed locally | Moved to `claude_audits/done`; arithmetic slice should add shift-distance masking |
| `0006` move analysis fragmented | done | Moved to `claude_audits/done`. `LowerCtx` holds **no** name-set approximation of the move relation: the two expression-lambda overrides are gone, because `lower_iterable_for_each_stmt` now synthesizes the `Stmt` the body *is* and keys the analysis on it. The block-union fallback is replaced by `liveness_covering_block`, which rebuilds a real per-statement analysis, so the safety net is more precise than what it replaced |
| `0007` opaque Rust strings | open | Shape tags exist and 8 producer families are migrated (27 `Shaped` sites). The emitted `/* TODO(translation): validate added clone */` comment is **no longer load-bearing syntax**: its three matching arms are deleted and `strip_trailing_block_comments` handles annotations structurally. `coercion.rs` syntax fragments 56 → 52. Remaining: the `OptionalProjected`/`MutBorrowed` family, then the anti-regression lint Third pass: the optional-projection family this audit named as "next" was attempted and **deliberately not migrated**, which is a correction to the design. Its five arms ask *which of five adapters was applied*, not *what kind of thing this is*, and `RustExprShape` has one name (`OptionalProjected`) for all of them. Tagging them anyway would emit **wrong code**: in these arms `true` means "skip the adapter", so a tag right about the family and wrong about the member suppresses an adapter that was needed. That exposes an unstated limit on the monotone rule — `tag || text` is safe only where `true` means "already correct", not where it means "do nothing". `RustExprShape` is `Copy` and `RustSlotShape` is not, so the fix is the audit's own step 1, `actual: RustSlotShape` alongside the coarse tag, which this family is the first consumer of and which should be built before it. Re-measured: 609 `Raw`, round trips 52 -> 49, fragments unchanged at 52 (correct — nothing was deleted because nothing was migrated) Fourth pass **built the prerequisite and migrated the family it blocked**. `RustExpr::Shaped` gained `actual: Option<Box<RustSlotShape>>` with `shaped_as`/`recorded_slot_shape` — the audit's own step 1 — and `apply_optional_borrowed_element_target_view` now asks `actual == target_element` instead of matching five rendered suffixes. The asymmetry that made this need a precise shape rather than the coarse tag is restated in the audit: here `true` means *skip the adapter*, so a family-level tag would emit a **missing** adapter, unlike the earlier migrations where `true` meant *already correct*. Text checks stay as the fallback so untagged producers behave exactly as before. Gate: **zero deltas**; one `type_coercion` assertion compared `RustExpr` structurally and now compares through `into_unshaped()`, the tag being invisible to the emitter by design. Also recorded: printing `actual` exposed a **stale `type_identity`** (`java_ty: int[]` with `erasure: Some(Optional)`), the invariant an independent survey flagged as the top representation-level finding in `slot_shape.rs` |
| `0008` inference does not reach lowering | partial | Steps 1–3 done in earlier rounds. Step 4 started: the JDK exception-supertype and input-stream-facade name sets moved to `javalibs`. **Rescoped** — the issue conflates library knowledge with core-language knowledge (`String`, `getClass`), and `javalibs.rs` is Rust, not a manifest; expressing the rest as data is a design task. A third whole-program relation (`storage_strategy`) is now solved once and consumed Third pass surveyed the *enumerated* decisions and found several encode a rule they have drifted from. **Fixed**: `is_known_java_subtype`'s exception arms were membership lists, so `catch (IOException e)` matched three types and a thrown `ZipException`/`SocketException`/`UnsupportedEncodingException`/`InterruptedIOException` silently escaped its handler — now `JDK_EXCEPTION_PARENTS`, parent edges with the relation as their reflexive-transitive closure, so one edge reaches every ancestor (the tests caught the same list-shaped mistake reappearing in the *dispatch* of the replacement, which now derives its target set from the table too); and four nullability entries claimed `DefinitelyNonNull` for methods that return null (`Attributes.getValue`, `JarFile.getManifest`, `Graphics.getClip`, `Class.getClassLoader`) — a wrong non-null is a **dropped null check**, since `Option` is only applied for `MaybeNull`/`DefinitelyNull`. **Found and recorded, not fixed**: the JLS 5.1.2 widening relation encoded four times with three rank scales that all put `char` on the numeric chain (so `char`->`short` scores as an exact overload match, `byte`->`char` emits a conversion Java rejects, and `conversion_plan`'s scale has no boolean guard while `resolve`'s does); two 150+-key enumerations of one key space with three receiver-matching disciplines mixed between them; a 193-arm hand-written inverse of `to_snake_case` with one dead arm (`"type"`, which the producer spells `type_`); a functional-interface grammar missing six members of its own product; and `DoubleParseDouble` absent behind a `_ => None` All but one of the recorded items are now **fixed**. `types::primitive_widens_to` is the single definition of the JLS 5.1.2 widening relation; the three rank scales and the 19-pair table delegate to it, and the pairs survive as the *check* on the rule. The three facts the ranks got wrong each have a test: nothing widens to `char` (the rank form scored `char`<->`short` as **exact** matches, so `f(short)` tied with `f(char)` for a `char` argument), `char` widens only to `int` and above, and `boolean` participates in nothing (`conversion_plan` had no guard where `resolve` did). Widening is additionally asserted as a strict partial order, and the distance defined exactly where widening is — which is what stops an illegal conversion outranking a legal one. Also fixed: the dead `"type" => "Type"` arm (the producer spells it `type_`), eleven missing members of the functional-interface grammar, and `DoubleParseDouble`. Verified **not** a defect: `to_snake_case` escapes only five keywords, but `rust_raw_ident_if_needed` raw-escapes downstream, so a Java field named `move`/`ref`/`mut` compiles as `r#move` — checked end to end. One item remains: the two 150+-key enumerations of the external return-type/nullability key space, whose answers mostly fall out of the return kind plus `ExternalApiRole` The last recorded item is now fixed too: the return-type and nullability tables enumerated one key space and had drifted (14 keys apart, three receiver-matching disciplines, so `java.lang.System` got a type and `Unknown` nullability). Closed by **deriving** — a `primitive`/`void` return cannot be null, which is a property of the type rather than a fact about the API — with the receiver canonicalised once. The test caught the drift immediately (`Integer.parseInt` non-null vs `java.lang.Integer.parseInt` unknown). One suite test moved and was a **stale expectation**: three of four numeric getters have a modelled primitive return and now correctly derive non-null; the fourth exposed that `ExternalApiEffectFact` is keyed by `(receiver, method)` with no arity while nullability is per-arity — noted in the audit rather than papered over |
| `0009` overload identity collapse | open | Java callable signature identity; focused inherited-overload/override probe is covered by `overloaded_override_preserves_child_body_and_inherited_overload`, current-callable binding facts now prefer `methods_by_java_key`, fact-aware resolved call-argument planning tries Java keys before Rust-name signatures, and receiverless `forEach` lambda calls select overload targets through resolved/signature facts before snake-case fallback. Interface trait impls now handle slots satisfied by inherited methods, pinned by `tests/differential/InterfaceOverloads.java`; broader downstream matching/rendering audit remains open Second pass: the reproduced instance/static disagreement is **fixed**, and the cause was a missing call rather than a missing key. `assign_type_rust_names` was already correct but was never run on context or summary programs, so every `MethodDecl` from one carried `rust_name: None` and `source_method_rust_name` fell back to the bare snake-cased name, collapsing `inst(int)` and `inst(String)` onto `u.inst(...)`. The static path escaped only because it re-solves signatures on an isolated program, which does assign names — one question, two implementations, one of them unfed. `resolve::assign_rust_names_to_context_program` now runs on summary declarations in both the single-file and batch paths (deliberately without `qualify_nested_type_names`, since those programs are looked up by the names they already carry). Pinned by `summary_source_overloads_get_one_rust_name_from_every_path`, which also asserts neither collapsed spelling survives; `scripts/gate.sh --all` shows zero corpus movement. Still open: the facade for a summary type emits a generic arity-based stub with no `new` and unsuffixed names, so the program still fails `E0599` — a fourth site that names methods, and a `0016`-family completeness gap rather than an identity one. The `usage::CallableKey::Method` adoption step remains the residual work, but the remaining `.name ==` sites should be checked for this same shape first Third pass fixed the **facade half**. `add_imported_static_owner_calls` synthesised an unmodelled-import stub for a type whose *source the translation is reading*, naming methods by arity — a fourth namer, which could not agree with the other three because it was not resolving overloads at all. The guard now withholds only the method stubs; two earlier versions suppressed the **type declaration** too (`E0412` is worse than `E0599`) because other guards share the key set and the `add_opaque_type` lives in the same branch, and both are recorded. Owner matched by simple name as well, since it is spelled as the importing file writes it (`dep.Util`) while summary keys are simple. Pinned by `a_summary_source_type_gets_no_unmodelled_import_stub` (no stub, type still emitted, call sites keep `pick_int`/`pick_string`); gate zero deltas. Still open: the facade **withholds** a wrong answer rather than providing the right one — emitting a summary type's real static/constructor surface is the next step |
| `0010` fallibility disagrees | done | Moved to `claude_audits/done`. One writer (the call-graph fixpoint), one store (`MethodExceptionPlan`, derived from it), one call-site decision, and one check that they agree (`src/fallibility_invariant.rs`, 0 violations over the corpus). `plan_method_exceptions`/`plan_program_exceptions` deleted; the trait-implementor `|=` removed once its cause was found (`override_groups_for_program` keyed on the **Java** method name while the plan keys on the Rust name, so equalization silently never matched); `plan_call_exception_propagation` split so lowering reaches an external-only entry point with no source-method probe |
| `0011` dyn Any fallback cascades | done | Moved to `claude_audits/done`; typed opaque/facade fallbacks replace late `Box<dyn Any>` cascades, and focused AWT probes compile with only intentional loud unmodelled constants |
| `0012` invalid unsupported diagnostics | fixed locally | Moved to `claude_audits/done` |
| `0013` nested downcast names | fixed locally | Moved to `claude_audits/done` |
| `0014` labelled statements deleted | fixed locally | Moved to `claude_audits/done`; CFG liveness still needs labelled edges |
| `0015` static initializer blocks dropped | done | Parsed as `TypeDecl::static_init_blocks` and lowered through guarded textual-order class initialization triggered by static getters, constructors, and static methods |
| `0016` no-op JDK facades | done | Moved to `claude_audits/done`; the silent-no-op class is closed for the audited core surfaces, regex facades compile, and placeholder upcasts are at least labelled as unmodelled |
| `0017` generic bounds dropped | done | Moved to `claude_audits/done`; scoped type-parameter identity replaces name-shape/global guesses, class/method/constructor bounds share one projection path, JavaComparable support is gated from projected bounds, and class/method/local/return collision regressions are covered |

2026-08-06 Codex follow-up for active rows: `0002b` storage facts now include
records, for/enhanced-for scoped locals, and wildcard generic bounds; `0009`
now emits source-summary static methods and constructors instead of only
withholding wrong stubs, and inherited summary method matching substitutes
generic supertype arguments before erased-signature comparison; `0018`'s
`noop_facade_call_sites` scanner is
boundary-aware and focused-package summaries fall back to the idiom sweep when a
stale analysis profile lacks the metric.
| `0018` compile metric blind spots | partial | Marker text owned by `src/output_markers.rs`; counters moved into the library so they are testable; `tests/output_markers.rs` requires a fixture per marker and asserts the counter rises. Audit found **two further dead detectors** (`"unsupported Java lambda"`, `"unsupported Java method reference"` matched text nothing emits) — deleted. Guard verified by mutation: a lowering that stops emitting its marker fails the suite. Remaining: differential coverage breadth, and triage of the 146 known `idiom_gaps` failures Third pass: the `noop_facade_methods` counter was **97% a different phenomenon** — 78 of 80 matches on a `StringBuilder`-only program were synthesised `From` impls for unmodelled Java upcasts, now counted separately as `unmodelled_upcast_conversions`; the counter drops 80 -> 2 there and 145 -> 130 on the htsjdk corpus, where the 15 removed were struct-field initialisers and local defaults the old regex matched because it looked for `Default::default()` anywhere in the file. Found on the way: `scripts/sweep-generated-idioms.py` and `output_markers` are two independent detectors for the same named signals across a language boundary and they disagreed, so `the_python_sweep_and_the_rust_counters_agree_on_shared_signals` now runs the real sweep script over a translation and requires them to match; and the sweep covers `generated/*.rs` only, so every signal it reports is a lower bound on anything appearing in the crate's shared support module (68 upcasts sit there, swept as 0). Also found and fixed: **the gate itself was reporting wrong numbers for a reason invisible in any diff.** `idiom_gaps` keeps failing generated crates in `/tmp` for inspection, but kept them whole including `target/`; at ~146 failures a run with no bound on runs the leftovers reached 10,525 directories / 31 GB and filled the root filesystem. One gate run came back `537/147` against a `538/146` baseline — a one-test delta, the exact size of signal this audit set treats as meaningful — and a direct re-run of the same tree then reported 280/404. Now only `Cargo.toml` and `src/` are kept and `target/` is dropped, so a failing crate still reproduces by running `cargo` in its directory; a full run costs 1.9 MB instead of ~450 MB, measured, with the suite still at 538/146 `noop_facade_call_sites` — the last item — is now **built**, and the earlier "unbuildable from text" reasoning was half wrong in a useful way: the obstacle (surviving no-op methods are named `to_string`/`clone`/`into`, so `.to_string(` would match unrelated calls) is real for **methods** and does not apply to **constructors**, which are called through a qualified path `Foo::new_2(` that cannot collide. The counter matches, per file, calls to exactly those facade constructors whose body is `Default::default()` — so a *modelled* type is excluded by construction. Measured: three `new Font(...)` give **1 definition, 3 call sites**, which is the ratio the audit exists to expose (a definition count reports 1 for a cost of 3); on a corpus file it reports 5 definitions and 1 call site, so it tracks the two directions independently. Reported in the analysis profile. Stated limit: no-op *methods* remain uncounted at their call sites and still need a facade-call lowering diagnostic, so the number is a lower bound whose constructor share is exact |
| `0019` raw Java expression fallback | fixed locally | Moved to `claude_audits/done`; simple switch expressions now lower to Rust `match` |
| `0020` text block processing | fixed locally | Moved to `claude_audits/done` |
| `0021` instanceof pattern binding | done | Moved to `claude_audits/done`. Bindings now bound in every position JLS 6.3.1 scopes them: `&&` and `||` in value position, ternary, and `while`. Regression test `tests/differential/Patterns.java` runs both toolchains. Note the escaping-scope case the issue implied is **rejected by javac**, so it is not a defect. Record/underscore/guard patterns emit `compile_error!` — loud, correct, unchanged |
| `0022` source-text type reparsing | done (scoped) | Moved to `claude_audits/done`. Closed **for the source route**: instrumented over the corpus plus a dense type-shape probe, the source-route fallbacks fire 0 times, and the ~4.9M remaining `parse_type_text` calls are all `javalibs`/manifest input — the use the audit says to keep. Four defensive `parse_type_text` fallbacks remain in `parser.rs` by design and are documented with their measurement; they are not converted to diagnostics because the sample is 248 files and the real corpora are larger. Found and fixed on the way: `parse_array_type_node` dropped a dimension on `int[][]`, which the cast handler's text re-parse had been papering over |
| `0023` inner-class outer capture | open | Object reference representation |
| `0024` numeric limit constants stubbed to zero | fixed locally | Moved to `claude_audits/done`; JDK wrapper/Math/GZIP/Font values fixed and unknown primitive constants now fail loudly |
| `0025` interface default methods discarded | done | Moved to `claude_audits/done`. Five distinct defects, closed together: an inherited default registered no return type (silent `""`); a locally declared type treated as an external super, synthesising facade shells for types already emitted (`E0592`/`E0428`); external inherited signatures registered before source ones under `or_insert` (no return type); a super-interface neither lowered as a supertrait nor counted as implemented (`E0599`/`E0277`); and `Iface.super` failing to resolve a nested interface (`E0433`). Four shared one cause — a nested type is stored qualified but named simply, and several lookups compared only the qualified form. Regression test `tests/differential/InterfaceDefaults.java`, seven cases. Still unprobed: `static`/`private` interface members |
| `0026` assignment evaluation order | done | Both residual surfaces the earlier pass left are now fixed. (1) The right-hand side can invalidate an index that has no side effect of its own — `b[m] = m++` writes `b[1]` under Rust's order and `b[0]` under Java's, a defect whose only symptom is *which element* was written. The gate moved from "the index has a side effect" to `assignment_order_is_observable`. That first over-fired on every `a[i] = f()` and cost three `idiom_gaps` tests; the precise rule is that a Java method cannot reach a caller's local, so a local index is disturbed only by an explicit write to that same name, while a field or array-element index keeps the conservative rule — which needs the shape environment, since a bare field reference and a local are the same `Expr::Var`. (2) A **field** assignment place has target subexpressions too: `arr[j++].field = j` is not an array assignment, so no ordered-array path saw it; `lower_ordered_field_place_assignment_stmt` hoists the index and re-lowers the place with the temporary substituted, reusing the ordinary field projection rather than hand-building a second one. Regression tests: `tests/differential/EvalOrder.java` against the Java oracle plus three focused tests including the negative case. Still open, and stated in the audit: ragged multi-level receivers, array-store exception ordering, and `pick(e).field = v` which is blocked on `0002` **Closed**: moved to `claude_audits/done`. All three exit criteria are met — the ragged residual is fixed and now pinned against the Java oracle (its blocker `0038` landed); array-store exception ordering has no work to do because no `ArrayStoreException` exists to order; and `pick(e).field = v` is explicitly reassigned to `0002`, since the receiver is passed as `e.clone()` so the write cannot be observed whatever the order. The sibling orders the audit asks be checked systematically — argument order, binary operand order, `&&`/`||` short-circuiting — are correct and now pinned by `tests/differential/EvalOrderSiblings.java` rather than merely asserted |
| `0027` compound assignment narrowing | fixed locally | Moved to `claude_audits/done`; narrow primitive compound assignment casts back through the shared assignment-value hook |
| `0028` reference equality | done | Reference identity now lowers through `plan_equality_comparison_for_source`: shared source values use `Rc::ptr_eq` or `Arc::ptr_eq`, nullable arms preserve Java `null == null`, and `EqualityComparisonSource` keeps `==` distinct from `.equals()`. `tests/differential/Identity.java` is promoted out of `known-gaps/` (`5,true,false,false`), and `tests/differential/IdentityCustomEquals.java` pins the override boundary (`true,false,true,false,true`). The pass also fixed the erased-object downcast parenthesisation bug and made erased-object `instanceof` use the same shared-handle carrier as downcast. Structural equality remains intentional for primitives, strings, enums, overridden `.equals()`, and unshared value classes. |
| `0029` static field mutation invalid Rust | fixed locally | Moved to `claude_audits/done`; static `++`/`--` now route through storage setters for current and qualified source statics |
| `0030` Java statics are thread-local | done | Moved to `claude_audits/done`. Static storage was `thread_local!`, so every thread got its own copy of a Java variable the language defines as shared: a counter, cache or registry silently became per-thread, a lazily initialised singleton was built once *per thread*, and — because the class-initialisation guard had the same defect — a `static { … }` block ran once per class **per thread**. Storage is now `std::sync::Mutex<Option<T>>` with an `AtomicBool` init guard, and a poisoned lock is recovered rather than propagated so a panicking initialiser cannot make the field permanently unreadable. Regression tests: `tests/differential/StaticSharing.java` pins the observable behaviour against the Java oracle, and `static_storage_is_shared_across_threads_not_per_thread` pins the storage *identity* a single-threaded run cannot see — the weaker check the audit itself proposes while the harness has no threaded fixture. `Send`/`Sync` propagation through the object graph stays with `0002` |
| `0031` stubbed constant census | done | Moved to `claude_audits/done`. Groups 1 and 2 (JDK mathematical and numeric-limit constants) were closed earlier with a value check against the Java oracle and a shape check that fails if any emitted `pub const` is a plausible zero. Group 3 turned out not to reproduce: each of its five constants has a hand-written `modeled_semantic_field_typed_value` entry, which fixes those five and does not scale — and one census row (`SAMSequenceRecord.UNKNOWN_SEQUENCE_LENGTH`) was a false positive, since Java declares it `= 0`. The general fix is `src/source_constant_values.rs`: a `static final` field declared in an available context-summary source now contributes its **declared value**, folded as a JLS 15.28 constant expression at Java's widths and overflow, with logical `>>>`, Java narrowing casts, constant string concatenation, and a program-wide fixpoint for cross-file references such as `VariantContext.NO_LOG10_PERROR = CommonInfo.NO_LOG10_PERROR`. Anything not provably constant folds to nothing and the loud `compile_error!` stands. Corpus reach, measured over five corpora (htsjdk, GATK, picard, FastQC, biojava): **5928 of 10442** initialised `static final` declarations now carry their declared value, where the source route previously supplied none. Honest negative: re-running `htsjdk_vcf` before and after is byte-identical, because every group-3 constant it reaches already has a pack entry — the change is a generalisation, and the reach table is its justification |
| `0032` java.lang.Math mistranslated | fixed locally | Moved to `claude_audits/done`; native `Math`/`StrictMath`/javalibs `FastMath` lowering now covers overload-safe `round`, `abs`, float `max/min`, `floorDiv`, `floorMod`, common f64 helpers, receiverless static imports, and argument-sensitive fallback return selection; generated-runtime coverage pins the audit's edge cases including negative/positive half rounding, `Float.NaN`, `Integer.MIN_VALUE` abs, signed `floorMod`, and NaN-propagating floating max |
| `0033` receiver mutability through lambdas | done | Moved to `claude_audits/done`. The evidence walk already descended into lambda bodies; what was missing is that `resolve_expr_calls` does not, so the call arrived at the self-mutability fixpoint with no resolved callee and no receiver type, and every arm of the condition needed one. An **unqualified** call needs neither — the owning type is the key of the map the facts live in — so the fixpoint gained that arm. Regression test `tests/differential/LambdaMutation.java` covers the direct and the two-hop transitive shape |
| `0034` object array allocation empty | done | Moved to `claude_audits/done`. `new T[n]` now allocates `n` slots via `vec![Default::default(); n]`, gated on a **derive obligation**: `src/array_default_obligations.rs` records the types some array creation needs a zero value for, and only those gain `#[derive(Default)]`. Lowering and item assembly consult one capability check so they cannot disagree. Regression test `tests/differential/ObjectArray.java` runs both toolchains. Residual, stated in the audit: reading a never-assigned element yields a zero value where Java throws NPE — option 2 of the audit; `Vec<Option<T>>` remains the faithful end state |
| `0035` idiom_gaps never terminates | done | Moved to `claude_audits/done`. Cause was a `while … {}` fixpoint deriving "changed" from the *intent* to add a facade method; `add_method` has six early returns where it does nothing. The suite returns in ~8 minutes and its failure-name set is checked in as `tests/baselines/idiom-gaps-failures.txt` (684 tests: 538 passed, 146 failed, 0 hung) |
| `0036` string-concat type inference exponential | done | Moved to `claude_audits/done`. `infer_binary_type` rebuilt the `Expr::Binary` node it already held — deep-cloning both subtrees — to ask whether a `+` was a string concatenation, making inference exponential in chain length: 11 segments took over 2 minutes, 40 segments now take the same 3 s of fixed start-up. Confirmed pre-existing at HEAD by stash-and-compare. Asks the operands directly instead; equivalence argued in the audit |
| `0037` nested class reads enclosing static as a capture | done | Moved to `claude_audits/done`. Found while writing the `0030` fixture, which had to route around it. Two defects stacked: the enclosing type's fields were collected as an outer-*instance* capture without filtering out `static` ones, so a `static` nested class reading a `static` field lowered to `self.__outer.borrow().n` inside a method with no `self`; and once the capture was suppressed the read resolved through the shared shape environment — which carries every type's statics under their bare names — while the accessor projection hardcoded `Self::`, emitting `OuterIn::__static_n()` for a field `Outer` owns (`E0599`). The write side already carried a `rust_owner`, which is why only the read failed; `ShapeEnv` now records the declaring type beside each static-storage name and the bare-name read qualifies with it. Regression tests: `tests/differential/NestedStatic.java` (both directions plus a nested static of the same simple name) and `nested_class_reads_enclosing_static_through_its_owner` |
| `0038` two-dimensional object array carrier disagrees | done | Found while fixing `0026`'s ragged residual, and it blocks that residual's differential fixture. `String[][] g = new String[2][2]` emits `let mut g: Vec<Vec<String>> = ndarray::Array2::from_elem(...)`, writes it with nested indexing and reads it with `g[[i, j]]` — four sites, three carriers, and `rustc` rejects the result. The two Java allocation forms of the same declared type also disagree: `new String[2][]` builds a nested `Vec` while `new String[2][2]` builds an ndarray. The policy is already correct and already exists — `types::ndarray_array2_element_type` requires a **primitive** element — but construction never asks it, and the read consults `two_dimensional_array_expr_uses_nested_slice`, which only recognises `static final` const arrays and so answers `false` for a local. `int[][]` is unaffected. Fix is one predicate consulted by all four sites plus a test that they agree **Fixed**: construction and read now consult `types::ndarray_array2_element_type` — the predicate that already said the ndarray carrier is for primitive elements and that the write path already asked — so all four sites agree and both Java allocation forms produce the same carrier. Moved to `claude_audits/done`. Regression test `tests/differential/ObjectArray2D.java` (`x,y,z,v2,2`) covers both allocation forms, a source-type element, and `0026`'s ragged ordering case; `int[][]` and an external-facade element are excluded because each would fail for a second unrelated reason, and both exclusions are stated in the fixture. This unblocks `0026`'s ragged residual, which now has a differential fixture |
| `0039` static array fields lose value and writes | done | Found while writing `0038`'s differential fixture. Two independent defects on one construct. (1) The declared initializer never reaches static storage: `static int[] counter = new int[3]` lowers to a getter whose fallback is `Vec::new()`, so the array has length 0 and the first index panics — the `0034` shape, unfixed on the static-storage path. (2) Worse and silent: `__static_counter()` returns `Vec<i32>` **by value** (it clones out of the mutex), so `Self::__static_counter()[0] = v` assigns into a temporary and is discarded. Rust accepts it, nothing warns, and the array simply never changes — so fixing (1) alone would turn a panic into a quietly wrong answer, which is why (2) must land first. Scalar statics are unaffected: a whole-value assignment routes through the setter; it is element and field writes *through* the getter that vanish. Not a `0030` regression — the old `thread_local!`+`RefCell` storage had the same by-value getter — but `0030`'s explicit `.lock().clone()` is what made it visible **Fixed**, write half first so the loud defect could not become a quiet one. `lower_static_storage_element_assignment_stmt` binds index and value to temporaries and *then* takes the guard — required, not tidiness: `counter[0] = counter[0] + 1` re-reads the same static and a `Mutex` is not reentrant, so the other order deadlocks; it also yields Java's evaluation order for free, and the guard is never held across a call. `get_or_insert_with` installs the declared value if a write precedes any read. `static_storage_initial_value_expr` gained `ArrayCreation`/`ArrayInitializer` arms that fold a constant length or element list through `source_constant_values`. Moved to `claude_audits/done`. Regression test `tests/differential/StaticArray.java` (`2,3,8,6,2,150`) uses two writes, and a write whose index and value both read the static being written. Residual, stated in the audit: a **non-constant** static array initializer keeps the empty-`Vec` fallback Second pass, both gaps found by review while the gate stayed green: (a) only the **bare** spelling was routed, so `Outer.a[i] = v` was still silently discarded — `static_storage_element_target_owner_field` now resolves the declaring type from the receiver too, which the nested-class case is the only way to exercise; (b) the fold was given an **empty** `ConstantScope`, so "constant length or element list" meant *literals only* and `static final int N = 3; static int[] a = new int[N];` still fell back to the empty `Vec` — `owner_constant_scope` now seeds the owning type's own constants in declaration order. Regression test `tests/differential/StaticArrayQualified.java` (`11,5,3,7,2`) covers qualified same-class, qualified from a nested class, bare, and `N`/`TWO = N - 1` as lengths Two further fold gaps closed on review, each reaching the same empty `Vec` by a different syntax — and the fallback is length 0, so a missed fold panics rather than merely deoptimising: a **qualified** sibling (`new int[Owner.TWO]`), now recorded under qualified spellings; and a constant owned by a **different type** (`new int[Sizes.M]`, ordinary Java), which no per-owner scope can see — the fold now starts from the whole program's declarations via `resolve::fold_constant_declarations`, the same fixpoint the summary-source path uses. Fixture extended to `11,5,3,7,2,2,4,4` |
| `0040` external type-name matching recomputes on the hot path | done | Found by asking why three-line test inputs took 70-100 s. `external_type_name_matches` calls `external_type_name_aliases` on **both** names, and that allocates a `Vec<String>` and calls `to_snake_case` / `canonical_external_receiver_type_key` up to six times — once per external-pack member, during a linear scan. `perf`: 38% of samples under that call, with `to_snake_case` (14.6%), `canonical_external_receiver_type_key` (13.8%) and ~25% in malloc/free/realloc as self-time leaders. Memoized; the function is a pure function of its argument, so a thread-local cache is sound. Measured: 7 methods with a summary source **78.7 s -> 12.4 s**, one test **85.6 s -> 12.8 s**, the whole `slot_shape_design` suite **58-83 s -> 24.5 s**; `scripts/gate.sh --all` zero deltas, as a pure memo must be. A `ContextSignatureCache` for the per-call-site `analyze_usage` + signature solve was built first and **reverted** — 11.15 s with vs 10.52 s without, i.e. noise — recorded in the audit so nobody optimises that path again. Deliberately not fixed: the linear scan itself, which wants an index and is a larger change the measurement did not justify |
| `0041` `slot_shape_design` had no generated-crate checks | done | `cargo_check_generated_modules` lived only in `tests/idiom_gaps.rs` (296 call sites); `slot_shape_design` had **zero**, so 596 tests asserted on rendered Rust with nothing checking it compiled — 0018's blind spot inside the suite. It hid a real defect: an `Option<Rc<Cursor>>` field against an `Option<Cursor>` local read as an assertion failure. Helper extracted to `tests/common/mod.rs`. Of 151 translation tests asserting on `translation.rust`: **99** now check and compile, **21** genuinely do not compile (9x E0599, 4x E0308, 4x E0282, …), **31** fail only because the check crate does not link the fixture's `ndarray`/`hdf5_facade` deps. Only the 99 were turned on, so the gate stays at 516/80 and the two kinds of failure cannot be conflated; the other 52 are enumerated in the audit as a backlog. 0002b's remaining step needs exactly this signal — every earlier attempt converged against assertion text rather than the compiler |

## Parallel Work Split

- Worker A: parse gate, coverage counters, unsupported diagnostic escaping, and
  nested downcast type-name normalization. Owns scripts, `src/unsupported.rs`,
  `src/coercion.rs`, `src/slot_render.rs`, and parse-gate tests.
- Worker B: raw Java expression fallback replacement. Owns `src/java_ir.rs`,
  `src/parser.rs`, `src/unsupported.rs`, and focused expression tests.
- Worker C: `CallableAnalysis` shell and initial behavior-preserving migration.
  Owns `src/callable_analysis.rs`, `src/lower.rs`, `src/lib.rs`, and source
  guard tests.
- Worker D: Java callable signature identity design slice. Owns
  `src/usage.rs`, `src/signature.rs`, `src/inherited_members.rs`, and overload
  regression tests.
- Main checkout: integrate in that order, run focused tests after each merge,
  and commit checkpoints.

## Verification Gates

Run at each checkpoint:

- `cargo fmt --check`
- `cargo check --quiet`
- focused audit repro tests for touched issues
- `cargo test --test idiom_gaps --quiet` filters for changed idioms
- `git diff --check`
- For any audit item marked fixed, verify its `claude_audits/*.md` issue file
  has been moved to
  `/home/mahogny/github/claude/new_java2rust/claude_audits/done` in the same
  checkpoint.
- If the issue file has not been moved to
  `/home/mahogny/github/claude/new_java2rust/claude_audits/done`, the audit is
  still open regardless of code status.

Run at batch boundaries:

- generated-Rust parse gate over focused corpuses
- one reduced htsjdk/VCF rerun
- one FastQC subset rerun
- a cause-ranked update of remaining corpus errors

## Non-Goals

- Do not model AWT/Swing deeply to make FastQC GUI code work.
- Do not model true Spark/DISQ distributed execution or GenomicsDB native
  behavior in this Java translator. For native/distributed systems, the Java
  translator should lower to explicit FFI or handover states when another
  transpiler or runtime owns the implementation.
- Do not replace Java reference semantics with clone insertion and call it
  complete; clone-obligation tracking is only a compile-quality stopgap.
- Do not let a lower corpus error count override silent correctness checks.
