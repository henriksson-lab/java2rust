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
- `0015-static-initializer-blocks-are-silently-dropped.md`: static initializer
  blocks are at least surfaced as unsupported members instead of being silently
  dropped.

Those fixes reduce the silent-wrong-output risk, but the audit set still points
to a larger refactor: make omissions measurable, then route call, borrow,
fallibility, shape, and facade choices through shared facts instead of local
string/name heuristics.

Keep the standing constraint: do not make algorithm changes that block future
global semantic analysis. Prefer explicit keys, indices, compatibility shims,
and divergence logs.

Latest user instruction: once a Claude audit is done, move its issue file to
`/home/mahogny/github/claude/new_java2rust/claude_audits/done` as part of the
same completed checkpoint.

Current checkpoint rule: after implementing and verifying a Claude audit item,
move its issue file from active `claude_audits/` into
`/home/mahogny/github/claude/new_java2rust/claude_audits/done` before calling
that audit done.

Active workflow invariant: when an audit-backed item is actually done, move the
corresponding `claude_audits/*.md` file to
`/home/mahogny/github/claude/new_java2rust/claude_audits/done` in that same
checkpoint. A code fix without that move is not a completed audit.

Operating invariant: a Claude audit is not done until its issue file has been
moved to `/home/mahogny/github/claude/new_java2rust/claude_audits/done`.
Whenever an audit-backed fix is completed, move the corresponding
`claude_audits/*.md` file into that `done/` directory in the same checkpoint as
the code, tests, and plan update. Do not leave completed audit files in the
active audit directory.

## Audit File Definition Of Done

Audit file housekeeping is part of the work, not a later cleanup step. Once a
Claude audit has been fully implemented and focused verification has passed,
move its issue file from `claude_audits/` to
`/home/mahogny/github/claude/new_java2rust/claude_audits/done` in the same
checkpoint as the implementation, plan-status update, and verification notes.

2026-07-31 action: every audit-backed implementation checkpoint must include
the corresponding move into
`/home/mahogny/github/claude/new_java2rust/claude_audits/done` once that audit
is actually done.

2026-07-31 reminder: every completed Claude audit must be moved into
`/home/mahogny/github/claude/new_java2rust/claude_audits/done` once done.

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
- External field fact lookup for enum/static-field lowering now goes through
  `member_access_plan::external_field_type_for_owner`; `lower.rs` no longer
  probes external-pack field facts directly.
- External constructor support checks for concrete-slot lowering now go through
  `call_plan::external_constructor_is_modeled`; `lower.rs` no longer probes
  external constructor facts directly.

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
  must be moved to the `done` directory below as part of the same completed
  checkpoint.
- Required file move: once a Claude audit is implemented and focused
  verification has passed, move its issue file from active `claude_audits/`
  into `/home/mahogny/github/claude/new_java2rust/claude_audits/done`.
- Move the corresponding issue file from `claude_audits/` to
  `/home/mahogny/github/claude/new_java2rust/claude_audits/done`.
- Keep the move in the same verified checkpoint as the implementation.
- Note the move in the plan row and, when committing, in the commit summary.
- Do not move partial or exploratory audit files.
- Treat the audit as still open if its file remains outside `claude_audits/done`.
- Do not mark an audit row as fully done unless the issue file has also been
  moved to `/home/mahogny/github/claude/new_java2rust/claude_audits/done`.

| Audit | Status | Plan owner |
|---|---:|---|
| `0001` loop back edges invisible | partial | Basic loop-carried read compatibility covers the LoopMove-style repeated consume; full CFG/liveness with labelled/switch/exception edges still open |
| `0002` object aliasing/clones | open | Clone discipline, then object representation |
| `0003` integer arithmetic semantics | partial | Binary int/long overflow, shift-distance masking, and local integral update wrapping fixed; divide-by-zero exception integration remains |
| `0004` string concat absorbs arithmetic | fixed locally | Moved to `claude_audits/done`; Rust IR shape tags prevent recurrence |
| `0005` unsigned right shift | fixed locally | Moved to `claude_audits/done`; arithmetic slice should add shift-distance masking |
| `0006` move analysis fragmented | partial | CallableAnalysis present; BindingId compatibility ids are attached to ShapeEnv for params, locals, catch params, loop variables, fields, and static fields, plus MoveUseContext use-count projection; CFG/liveness still open |
| `0007` opaque Rust strings | open | Slot/facade generalization, shape tags |
| `0008` inference does not reach lowering | open | CallableAnalysis |
| `0009` overload identity collapse | open | Java callable signature identity; focused inherited-overload/override probe is covered by `overloaded_override_preserves_child_body_and_inherited_overload`, but broader downstream matching/rendering audit remains open |
| `0010` fallibility disagrees | open | Callable fallibility plan |
| `0011` dyn Any fallback cascades | partial | Opaque external shims; generic/static typed facade fallbacks are now loud; external constructor argument facts now participate in signature inference, so modeled constructors such as `StringTokenizer(String,...)` and `ZipEntry(String)` no longer force declared source `String` params through `Box<dyn Any>`/`downcast::<String>()`; declared locals initialized by fact-backed typed external producers now anchor through a generic return-type identity rule instead of an AWT-specific `Graphics/createGraphics` escape hatch; field-accessing parsed source supertypes with runtime case-split pressure now use companion hierarchy carriers instead of erasure; reduced VCFStandardHeaderLines now cargo-checks cleanly after exact downcast locals/source peer params stay concrete and weak erased-object facade returns upgrade from typed facade impl signatures, but broader erased-object fallback remains open |
| `0012` invalid unsupported diagnostics | fixed locally | Moved to `claude_audits/done` |
| `0013` nested downcast names | fixed locally | Moved to `claude_audits/done` |
| `0014` labelled statements deleted | fixed locally | Moved to `claude_audits/done`; CFG liveness still needs labelled edges |
| `0015` static initializer blocks dropped | partial | Loud diagnostic now; full class-init later |
| `0016` no-op JDK facades | partial | `StringBuilder`/`StringBuffer`, literal `Pattern`/`Matcher`, `StringTokenizer`, `ZipEntry`, and `BitSet.get` paths are stateful; `Arrays.copyOf/copyOfRange/toString/fill/sort/hashCode/deepHashCode` and common `Math`/`StrictMath` calls lower natively without dead fallback support; Apache `FastMath` uses a javalibs owner alias to the Math planner; broader JDK facade fallibility and remaining no-op utility surfaces remain open |
| `0017` generic bounds dropped | partial | Method/constructor type parameters now emit as structured Rust function generics, ordinary source method/constructor bounds that resolve to emitted source interfaces/classes emit Rust bounds, analysis-only context bounds are suppressed to avoid missing Rust traits, and raw trait signatures preserve method generics/bounds with object-safe `where Self: Sized`; multi-bounds, external/JDK bound projection, and full typed type parsing remain open |
| `0018` compile metric blind spots | partial | Parse gate and coverage counters |
| `0019` raw Java expression fallback | fixed locally | Moved to `claude_audits/done`; simple switch expressions now lower to Rust `match` |
| `0020` text block processing | fixed locally | Moved to `claude_audits/done` |
| `0021` instanceof pattern binding | partial | Positive-block `if (o instanceof T v)`, negated terminating guards, and left-pattern `&&` chains now bind the pattern variable for erased-object, dyn/boxed-dyn source-interface, optional erased-object, and closed-enum checks through shared `InstanceOfCheck` projections; record patterns, underscore patterns, guarded patterns, and broader open source-hierarchy carrier analysis remain open |
| `0022` source-text type reparsing | partial | Common source type nodes now parse structurally before legacy text fallback; type-parameter declarations still use the old parallel primary-bound IR, and external/text-sourced types intentionally still use `parse_type_text` |
| `0023` inner-class outer capture | open | Object reference representation |
| `0024` numeric limit constants stubbed to zero | fixed locally | Moved to `claude_audits/done`; JDK wrapper/Math/GZIP/Font values fixed and unknown primitive constants now fail loudly |
| `0025` interface default methods discarded | partial | Ordinary interface methods with parsed bodies now emit Rust trait default bodies, and implementors without overrides inherit them instead of getting missing-implementation panics; `Iface.super`, conflicting defaults, and private/static interface members remain open |
| `0026` assignment evaluation order | partial | Simple and compound statement/expression one-dimensional array assignments with side-effecting indices now hoist the index before RHS evaluation/readback, and compound paths save the old element before RHS evaluation; side-effecting receivers and matrix/index-place generalization remain open |
| `0027` compound assignment narrowing | fixed locally | Moved to `claude_audits/done`; narrow primitive compound assignment casts back through the shared assignment-value hook |
| `0028` reference equality | partial | Binary `String ==`/`!=` now emits `AUDIT(java-string-identity)` and lowering diagnostics report `java_string_identity`/`java_reference_identity_equality`; full object reference identity still waits on object representation |
| `0029` static field mutation invalid Rust | fixed locally | Moved to `claude_audits/done`; static `++`/`--` now route through storage setters for current and qualified source statics |
| `0030` Java statics are thread-local | open | Static storage model |
| `0031` stubbed constant census | partial | JDK and selected HTSJDK domain constants are explicit; unknown primitive/named external constants now fail loudly and status/summary reports include `stubbed_constants`; corpus reruns still need to prove the remaining count |
| `0032` java.lang.Math mistranslated | fixed locally | Moved to `claude_audits/done`; native `Math`/`StrictMath`/javalibs `FastMath` lowering now covers overload-safe `round`, `abs`, float `max/min`, `floorDiv`, `floorMod`, common f64 helpers, receiverless static imports, and argument-sensitive fallback return selection; generated-runtime coverage pins the audit's edge cases including negative/positive half rounding, `Float.NaN`, `Integer.MIN_VALUE` abs, signed `floorMod`, and NaN-propagating floating max |

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
