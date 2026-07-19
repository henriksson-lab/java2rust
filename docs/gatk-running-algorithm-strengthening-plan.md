# GATK Running Algorithm Strengthening Plan

## Purpose

This plan turns the current strategy assessment into trackable engineering
work. The goal is to get from focused GATK resolver/semantic cleanliness to
focused GATK packages that emit Rust which can be checked by `cargo check`,
without replacing the current architecture or adding GATK-specific lowering
rules.

The strategy remains:

```text
parse -> resolve -> semantic facts -> representation plan
      -> signature/slot-shape fixpoint -> conversion plan
      -> lowering -> Rust compile/idiom gates
```

Do not replace this with a general SMT/SAT solver yet. The current problems
are finite, typed dataflow problems with small lattices and explicit fallback
diagnostics. Strengthen the existing algorithms before introducing a heavier
solver.

## Success Definitions

### Milestone A: Resolver/Semantic Envelope Clean

A focused GATK package is envelope-clean when:

- `parser_diagnostics == 0`
- `resolver_diagnostics == 0`
- `semantic_diagnostics == 0`, except explicit unsupported facts that are
  intentionally counted and documented
- `external_api_missing_method_calls == 0`
- `external_api_missing_constructor_calls == 0`
- `external_api_missing_field_accesses == 0`
- `unexplained_method_calls == 0`
- `unexplained_constructors == 0`
- `unexplained_field_accesses == 0`
- source-helper pressure is attributed to `source_dependency_resolved_*`
  counters through co-translation or source summaries, not to external guesses
- object-representation diagnostics are either zero or explained by an
  explicit source hierarchy strategy decision

### Milestone B: Focused Package Emits Checkable Rust

A focused GATK package is Rust-checkable when:

- the translator emits one Rust crate or crate-like output for that package and
  its selected co-translated/source-summary/external boundaries
- the generated crate passes `cargo check`
- unsupported Java features appear as structured diagnostics or explicit stub
  boundaries, not as invalid Rust syntax
- the check gate is automated and reproducible from a script or test command
- the emitted Rust does not contain the top invalid patterns from the current
  idiom audit for the selected package

### Milestone C: Focused Package Emits Idiomatic Rust

A focused package is idiom-clean enough for this phase when:

- Java collection calls are lowered through Rust collection/iterator APIs
- typed getters avoid erased downcast/`expect` paths when a concrete type is
  known
- nullability is represented as `Option<T>` only when required or unproved
- lambdas and method references become closures or function items
- `try`/`catch`/`finally` and try-with-resources are represented through
  `Result`, `Drop`/scope guards, or explicit unsupported boundaries
- `super(...)`, `this(...)`, and constructor delegation are represented through
  planned initialization, not placeholder syntax
- class literals and reflection boundaries are modeled by facts and rendered as
  typed tokens or deliberate opaque boundaries

## Algorithm Choice

### Keep

- finite fact domains
- monotone worklists
- SCC-aware call graph and constructor graph solving
- explicit capability facts
- representation plans
- signature/slot-shape constraints
- conversion plans
- external packs and source summaries
- structured diagnostics and conservative fallbacks

### Avoid For Now

- a global SMT/SAT solver for the whole translator
- lowering-time Java name heuristics
- one-off GATK class or method branches
- path-sensitive facts keyed by expression text
- recursive specialization of functions or generic instantiations
- treating a full GATK benchmark as the only test oracle

### Revisit Later

Reconsider a general solver only if one of these becomes true:

- lattice constraints become non-monotone and cannot be decomposed
- conflicting requirements need optimization rather than precedence
- localized SCC/worklist solving produces unstable or order-dependent results
- fallback diagnostics cannot explain why a shape was chosen

## Transformation Rule Authoring Contract

The canonical repository-wide rule template lives in
`docs/transformation-rule-contract.md`. This plan repeats the rule here because
it is the active implementation roadmap for the GATK-focused work.

The broader-plan readiness gate is the one in
`docs/transformation-rule-contract.md#broader-plan-rule`: each transformation
rule must be documented as an ideal Rust target first, then an implementation
rule. This plan may group work by benchmark slice or subsystem, but grouping
does not relax the per-rule ordering requirement.

For this roadmap, that means every transformation rule we promote from a broad
benchmark item into implementation work must first answer: "what Rust would we
want to maintain by hand?" Put that answer in an explicit **Ideal Rust Target**
section, then put the fact-driven algorithm in the following **Implementation
Rule** section. If the ideal target cannot be stated yet, the item remains an
analysis task, even if the failing Java pattern and likely module change are
already known.

For the broader transpiler plan, the contract is explicit: every
transformation rule must document the **Ideal Rust Target** before documenting
the **Implementation Rule**. The target-first section is not optional planning
commentary; it is the design input that decides which semantic facts,
fallbacks, ownership choices, and layer responsibilities the implementation
must preserve.

Every tracked transformation rule must start from the Rust we want to emit, not
from the Java syntax we happen to have parsed. New rules must be written this
way from the start; existing rules that predate the contract must be normalized
before they are used for implementation work. This keeps the design honest
about ownership, nullability, dispatch, lifetimes, and external API boundaries
before implementation details lock in accidental shapes.

Use this order for each transformation rule. The order is mandatory: the
explicit **Ideal Rust Target** section must appear immediately before the
explicit **Implementation Rule** section, and both must appear before any
algorithm sketch, status note, current generated output, benchmark diagnostic,
or module-specific lowering description. A rule that starts with implementation
mechanics is not ready to implement; first add the ideal Rust target and use it
to expose the ownership, lifetime, dispatch, and fallback decisions the
implementation must preserve.

Concrete rule entries in this plan should therefore start with the target
subsection itself, not a prose lead-in that already decides the implementation.
The first two fields are named contract fields; do not replace them with
synonyms such as "approach", "lowering", "desired output", or "algorithm":

```md
- Ideal Rust Target: ...
- Implementation Rule: ...
- Required Facts: ...
- Owning Layer: ...
- Fallback Shape: ...
- Evidence: ...
```

When a section contains several transformations, repeat that block for each
rule unless the section explicitly documents a shared target and shared fact
requirements. Status notes and current benchmark failures belong after the
target and implementation rule, not before them, and not between them.

Before a broad work item from this plan becomes an implementation slice,
normalize it into individual transformation rules. Each rule must have its own
**Ideal Rust Target** immediately followed by its own **Implementation Rule**.
Broad labels such as ownership recovery, stream lowering, external API
modeling, object representation, or exception handling are planning buckets,
not implementable rules, until the specific mappings inside them have been
written in this target-first form.

This is a per-rule requirement, not a per-milestone heading. If a work item
adds or changes several mappings, each mapping must either have its own
**Ideal Rust Target** immediately followed by its own **Implementation Rule**,
or it must explicitly state why those mappings share one target Rust shape and
one set of required facts. Do not clear a checklist item by documenting only
the implementation mechanics for a cluster of Java symptoms.

1. **Ideal Rust Target**: Show the Rust form we are trying to reach for the
   representative Java pattern. Include the carrier and ownership shape when
   that matters, for example `String` vs `&str`, `Option<T>`, `Vec<T>`,
   `HashMap<K, V>`, enum variants, `Rc<dyn Trait>`, or an explicit opaque
   external boundary.
2. **Implementation Rule**: State the source-independent transformation in
   terms of semantic facts and plan nodes, not in terms of a one-off fixture or
   corpus class. This section should explain the general mapping after the
   target Rust shape is clear.
3. **Required Facts**: List the facts that must be known or conservatively
   assumed before that Rust can be emitted. Facts should be phrased in the
   vocabulary of existing layers: resolved owner, source/dependency boundary,
   slot shape, nullability, mutability/effects, representation, callable
   signature, or external pack capability.
4. **Owning Layer**: Assign each fact and decision to one primary module or
   plan. Rendering should not rediscover semantic facts, and semantic analysis
   should not decide final Rust spelling.
5. **Fallback Shape**: State what we emit or diagnose when the ideal facts are
   unavailable. Fallbacks must remain structured and finite: `Option<T>`,
   `Box<dyn Trait>`, `Box<dyn Any>`, an opaque external facade, or an explicit
   unsupported diagnostic.
6. **Evidence**: Add at least one design-level test for the owning layer and at
   least one neutral fixture/output assertion when the rule affects emitted
   Rust. Corpus-derived regressions should challenge the same general pattern
   without overfitting to one GATK class or method.

Review checklist for every rule edit:

- Does the **Ideal Rust Target** show the Rust API or expression we would want
  by hand, before mentioning parser nodes, Java AST shapes, or current lowerer
  behavior?
- If the edit covers multiple mappings, does each mapping have its own target
  Rust shape before its implementation rule, or an explicit shared-target
  justification?
- Does the **Implementation Rule** describe how semantic facts select that
  target, instead of encoding the fixture name, package name, or one Java
  spelling?
- Are fallbacks expressed as structured Rust shapes or explicit diagnostics,
  rather than ad hoc syntax substitutions?

Existing transformation items that predate this contract must be normalized
before they are used as the basis for a new implementation slice. A rule is not
ready to implement if its first concrete content is "lower Java pattern X as
Rust expression Y" without first showing the complete Rust target shape and why
that shape is the desired idiom.

Design gate for implementation slices:

- If the **Ideal Rust Target** is missing, the item is still in design, even if
  the corpus symptom is obvious.
- If a work item contains several transformation rules, each rule must have its
  own **Ideal Rust Target** immediately before its **Implementation Rule**. A
  milestone-level target is not enough to start coding the individual mappings.
- If the ideal target exposes missing ownership, lifetime, nullability,
  dispatch, coercion, or external-boundary facts, the next slice must add those
  facts or a structured fallback before changing rendering.
- If two possible ideal targets are plausible, document the preferred target,
  the fallback target, and the fact that distinguishes them before coding.
- The **Implementation Rule** may only describe how semantic facts select the
  documented target; it must not be the first place where the Rust shape is
  chosen.

Example rule shape for external static constants:

- **Ideal Rust Target**:
  `GATKVCFConstants::RAW_MAPPING_QUALITY_WITH_DEPTH_KEY` flowing into a
  `String` slot should remain typed static constant access, with no
  `downcast::<String>()` or runtime cast.
- **Implementation Rule**: type-qualified member access is planned as a static
  constant access when field facts prove the receiver is a type/namespace and
  the member has a known static field type. Conversion planning then treats the
  static access as already having the proved value shape instead of inserting a
  cast adapter.
- **Required Facts**: the receiver is a type/namespace, the member is a static
  constant, the external field type is `String`, and the argument/return slot
  also expects `String`.
- **Owning Layer**: `stdlib`/external facts own the field type;
  `types::infer_member_access_type` owns the Java type; `member_access_plan`
  owns static constant access; shape projection and conversion decide that no
  cast is needed; lowering only renders the plan.
- **Fallback Shape**: if the field type is unknown, keep the value in an
  unknown/opaque shape and allow conversion planning to emit a structured
  unsupported or erased-object boundary rather than guessing a typed cast.

## Trackable Work

Each item below has a stable ID. An item is clearable only when its evidence
exists in code, tests, commands, or benchmark docs. If an item spans multiple
modules, the implementation slice should still choose one owning module first
and add tests at that layer before Rust-output assertions. For transformation
items, clearability also requires an **Ideal Rust Target** section before the
**Implementation Rule** section, before status notes, and before
module-specific lowering details.

This is a per-transformation-rule gate. A broad checklist item may contain
several Java-to-Rust mappings, but each mapping must still state the ideal Rust
target before it states the implementation rule. If the same implementation
slice covers several mappings with one target, the plan entry must say why the
target Rust shape, required facts, owning layer, and fallback shape are shared.

For GATK-driven slices, the benchmark failure can motivate the rule but cannot
be the first design statement. Before implementing a transformation rule, write
the ideal Rust API, type, expression, or control-flow shape that we would want
if all facts were known. Then write the implementation rule that explains which
resolver, semantic, representation, slot-shape, conversion, or external-pack
facts select that target. If this ordering is missing, the item remains open
even if the generated Rust count improves.

### 1. Fresh Benchmark And Compile Baselines

- [x] `DOC-1`: Normalize existing transformation entries to the rule template.
  - Owner: `docs/gatk-running-algorithm-strengthening-plan.md`
  - Ideal Rust Target: every transformation item in this plan has an explicit
    target Rust shape before it describes implementation, status, or lowering
    mechanics. Readers should be able to inspect the target first and decide
    whether the algorithm is moving toward idiomatic Rust rather than merely
    removing a Java syntax symptom.
  - Implementation Rule: before starting or materially revising any
    transformation item, add the ordered sections from the Transformation Rule
    Authoring Contract: **Ideal Rust Target**, **Implementation Rule**,
    **Required Facts**, **Owning Layer**, **Fallback Shape**, and **Evidence**.
    Existing status text can remain, but it must come after the target and rule
    sections.
  - Evidence: a plan audit lists every `OBJ-*`, `COERCE-*`, `CALL-*`,
    `CTRL-*`, `EXC-*`, `INIT-*`, and later transformation item that has been
    normalized, and no new implementation slice starts from an unnormalized
    rule.
  - Status: Complete. The authoring contract is explicit, and the mechanical
    plan audit confirms every transformation section has an **Ideal Rust
    Target** before implementation/status text. Normalized sections are
    `OBJ-1` through `OBJ-6`, `DYN-1` through `DYN-3`, `CAST-1` through
    `CAST-3`, `CAP-1` through `CAP-3`, `SIG-1` through `SIG-6`, `TYPE-1`
    through `TYPE-5`, `NULL-1`, `COLL-1`, `STREAM-2`, `COERCE-1`, `NULL-2`,
    `ARRAY-1`, `CALL-1`, `CALL-2`, `CTRL-1`, `CTRL-2`, `EXC-1`, `EXC-2`,
    `EXC-3`, `INIT-1`, and `CTRL-3`. The audit command checks every
    `OBJ|DYN|CAST|CAP|SIG|TYPE|NULL|COLL|STREAM|COERCE|ARRAY|CALL|CTRL|EXC|INIT`
    trackable transformation item and reports no missing ideal-target
    sections.

- [x] `BASE-1`: Add or update a script that runs the resolver benchmark on the full GATK
  tree and on focused package envelopes.
  - Owner: `scripts/`, `src/main.rs`, `src/resolver_benchmark.rs`
  - Evidence: command documented in `docs/gatk-resolver-benchmark-report.md`
    with elapsed time and counters.
  - Implemented as `scripts/run-gatk-resolver-benchmarks.sh`. It builds the
    CLI when needed, runs full GATK plus focused `tools/genomicsdb`,
    `tools/spark/sv/evidence`, and `tools/walkers/variantutils` envelopes,
    supports parallel focused jobs, writes raw reports and timing files, and
    extracts stage/ownership counters into `summary.tsv`. Documented under
    "GATK Benchmark Runner" in `docs/gatk-resolver-benchmark-report.md`.
    Smoke checked with `--gatk-root examples/java --only-full`.

- [x] `GATE-0`: Add a package-level generated Rust compile harness.
  - Owner: `scripts/`, `tests/`, top-level CLI integration
  - Evidence: one command translates a focused package into a temp crate and
    runs `cargo check`.
  - Start with: `tools/genomicsdb`, `tools/spark/sv/evidence`,
    `tools/walkers/variantutils`.
  - Gate must fail on error diagnostics, `compile_error!`, rustc failure,
    Java collection spellings on native Rust containers, concrete `= None`,
    downcast placeholders, and lambda/method-reference placeholders.
  - Existing baseline: `cargo test --test fixtures first_slice_generated_rust_compiles`
    already proves a small compile-positive slice for constructor delegation,
    switch fallthrough, throw-only `Result`, and initial streams. Build on
    that rather than starting a separate compile-check mechanism.
  - Implemented as `scripts/translate-focused-package.sh`. Current limitation:
    source roots are accepted and used for resolver-bench preflight only;
    generated Rust emission remains one Java file per generated module until
    package-aware translation is added. The focused harness now performs a
    package assembly pass over those generated modules: it collects unique
    public generated type names, re-exports them from the generated crate root,
    and imports sibling generated types into each module while filtering
    support/prelude-colliding names such as `JavaException` and `Box`. This
    does not replace real package-aware lowering, but it makes focused gates
    measure unresolved external/dependency boundaries instead of failing merely
    because co-translated sibling types are invisible. The same assembly pass
    now hoists shared generated support carriers (`JavaException`, `Path`,
    `URI`, and `File`) once into the generated crate root and strips the
    leading per-module support prelude from independently translated modules.
    Generated modules import those support names from `crate`, so package gates
    test one crate-level support runtime instead of silently creating distinct
    `generated_...::URI` and `generated_...::Path` types per Java file.

- [x] `BASE-2`: Make broad GATK benchmark runtime visible.
  - Owner: `src/resolver_benchmark.rs`
  - Evidence: report includes elapsed time, files/sec, and slowest phase
    buckets, or the runner emits progress rows.
  - Completion test: a full GATK run can be interrupted without losing partial
    phase counters.
  - Implemented with coarse benchmark phase counters in
    `src/resolver_benchmark.rs`: `files_per_second_x1000`, `parse_phase_ms`,
    `source_summary_phase_ms`, `representation_phase_ms`, and
    `resolution_phase_ms` are now rendered next to `elapsed_ms`. The counters
    are updated at phase boundaries so broad runs expose where time is spent
    before users inspect downstream diagnostics. Coverage:
    `resolver_benchmark_reports_runtime_phase_counters`.

- [x] `BASE-3`: Separate benchmark gates by stage.
  - Owner: docs and scripts
  - Evidence: commands exist for parser/resolver/semantic benchmark,
    representation benchmark, signature benchmark, emitted compile check, and
    idiom sweep.
  - Implemented as `scripts/run-focused-stage-gates.sh`, with explicit
    `--stage parser-resolver-semantic`, `representation`, `signature`,
    `compile`, `idiom`, and `all` modes. The wrapper reuses `resolver-bench`
    for fact/diagnostic stages, delegates compile and idiom evidence to
    `scripts/translate-focused-package.sh`, accepts normal source-root/fact/
    pack inputs, and writes `stage-summary.tsv`. Documented under "Focused
    Stage Gates" in `docs/gatk-resolver-benchmark-report.md`; smoke checked
    with the signature stage on `examples/java`.

### 2. SCC And Worklist Backbone

- [x] `SCC-1`: Declare finite fact-domain heights explicitly.
  - Owner: `src/fact_saturation.rs`, `src/facts.rs`
  - Evidence: `FiniteFactDomain` accounts for declared slots and value
    lattice height, not only currently present facts.
  - Tests: `cargo test fact_saturation`
  - Implemented with explicit `declared_slots` and `value_lattice_height`
    fields. `FactSet::hierarchy_reachability_domain` exposes the hierarchy
    reachability domain, and hierarchy tests assert the finite budget for both
    acyclic and recursive hierarchies.

- [x] `SCC-2`: Add a reusable worklist saturation engine.
  - Owner: `src/fact_saturation.rs`
  - Evidence: synthetic chain, diamond, and cycle graph tests report stable
    output plus bounded `processed_items`, `inserted_facts`, and
    `max_queue_len`.
  - Tests: `cargo test fact_saturation`
  - Implemented as `saturate_worklist` with `WorklistStats`,
    `WorklistSaturationPolicy`, and `WorklistInserter`.

- [x] `SCC-3`: Build an explicit call graph for methods and constructors.
  - Owner: `src/usage.rs`, `src/semantic_analysis.rs`, `src/signature.rs`
  - Evidence: tests expose direct calls, mutual recursion, constructor
    delegation, anonymous-class method calls, lambdas, and unresolved external
    call edges as separate edge kinds.
  - Implemented as `usage::build_call_graph`, with `CallableKey`,
    `CallGraphEdge`, and `CallGraphEdgeKind`. Semantic recursion facts now
    consume this graph instead of maintaining a separate same-type call scan.

- [x] `SCC-4`: Add Tarjan or Kosaraju SCC condensation for the internal call graph.
  - Owner: new helper in `src/graph.rs` or `src/fact_saturation.rs`
  - Evidence: tests show direct recursion and mutual recursion are solved once
    per SCC and do not depend on declaration order.
  - Implemented in `src/graph.rs` as Tarjan SCCs plus
    `component_successors`. `src/usage.rs` exposes the internal call graph,
    semantic recursion facts consume SCCs, and the signature solver pass plan
    runs over signature dependency SCCs. Recursion tests cover direct, mutual,
    three-method, acyclic, and declaration-order-independent cases.

- [x] `SCC-5`: Model signature propagation as an explicit dependency graph.
  - Owner: `src/signature.rs`
  - Evidence: graph nodes exist for method params/returns/locals, constructor
    params/locals, fields, receiver slots, captured values, call args, call
    results, and unknown boundary results.
  - Tests: `cargo test --test slot_shape_design signature_fixpoint`
  - Implemented as `build_signature_dependency_graph` and
    `build_signature_dependency_graph_with_facts`, with explicit slot,
    call-boundary, unknown-boundary, and captured-value nodes. Focused evidence:
    `cargo test --test slot_shape_design signature_dependency_graph`.

- [x] `SCC-6`: Route signature solving over SCCs.
  - Owner: `src/signature.rs`
  - Evidence: recursive method constraints propagate within an SCC to a fixed
    point, then flow to callers in topological order.
  - Tests:
    `cargo test --test slot_shape_design signature_fixpoint_saturates_recursive_call_cycles_within_budget signature_fixpoint_saturates_mutual_recursive_call_cycles_within_budget`
  - Implemented: `SignatureFixpointResult` now carries
    `SignatureDependencySccAnalysis` computed from the explicit signature
    dependency graph. Recursive method and recursive constructor-delegation
    fixtures assert nonzero cyclic SCCs while still solving within the finite
    budget. The fixpoint loop now executes through a
    `SignatureSolverPassPlan` derived from SCC condensation order, commits only
    active-component slot changes per planned pass, and locally repeats cyclic
    components to stability before moving on.

- [x] `SCC-7`: Route constructor delegation solving over SCCs.
  - Owner: `src/signature.rs`, constructor usage facts
  - Evidence: `this(...)` cycles and chained delegations converge with finite
    diagnostics instead of repeated local propagation.
  - Implemented through the SCC-scoped signature fixpoint path:
    constructor-delegation argument edges are represented in
    `SignatureDependencyGraph`, pass execution commits only active-component
    slot changes, cyclic constructor-delegation components locally saturate,
    and chained delegation constraints have focused coverage.

- [x] `SCC-8`: Add phase counters for SCC/worklist solving.
  - Owner: `src/signature.rs`, `src/resolver_benchmark.rs`
  - Evidence: report includes graph nodes, edges, SCC count, max SCC size,
    iterations, processed work items, inserted facts, max queue size, domain
    size, and budget diagnostics.
  - Implemented: signature fixpoint results expose dependency graph node count,
    edge count, component count, cyclic component count, max component size,
    planned component count, cyclic planned component count, max planned
    component size, domain slots, unique constraints, iteration budget,
    iterations, and budget diagnostics in the benchmark report. Covered by
    `resolver_benchmark_reports_signature_solver_phase_counters`. Worklist
    processed-item, inserted-fact, max-queue, domain-size, and budget
    diagnostic counters are aggregated for hierarchy reachability in the
    benchmark report and covered by
    `resolver_benchmark_reports_hierarchy_worklist_phase_counters`.

- [x] `SCC-9`: Make recursive semantic facts finite by construction.
  - Owner: `src/semantic_analysis.rs`, `src/fact_saturation.rs`
  - Evidence: recursion facts are keyed by method/constructor IDs and edge
    kinds, not by expanded call paths.
  - Implemented with method `RecursionFact` and constructor
    `ConstructorRecursionFact` vectors keyed by `MethodKey`/`ConstructorKey`.
    Both are populated from finite call-graph edges plus SCC membership, and
    constructor delegation cycles are covered by
    `records_constructor_delegation_recursion_as_finite_edges`.

- [x] `SCC-10`: Strengthen recursion facts beyond two-node mutual recursion.
  - Owner: `src/semantic_analysis.rs`
  - Evidence: a three-method cycle marks all SCC members as mutual recursion,
    while acyclic call chains produce no recursion facts.
  - Tests: `cargo test --test semantic_facts recursion`
  - Implemented by classifying same-type method call edges through
    `strongly_connected_components`.

- [x] `SCC-11`: Move hierarchy reachability onto the shared worklist/SCC path.
  - Owner: `src/facts.rs`, `src/fact_saturation.rs`
  - Evidence: cyclic hierarchy tests terminate, reachability has no duplicate
    edges, recomputation is avoided or explicitly cached, and budget
    diagnostics can surface through semantic analysis.
  - Tests: `cargo test --test slot_shape_design facts_saturate`
  - Implemented with `reachable_hierarchy_edges_for_type_with_stats`, which
    uses `saturate_worklist` and returns `WorklistStats` plus hierarchy budget
    diagnostics. The existing edge-only helper delegates to it. Semantic
    exception classification consumes the stats-bearing path so hierarchy
    budget diagnostics can surface as semantic diagnostics.

### 3. Finite Lattice Definitions

- [x] `LAT-1`: Define named lattice types for nullability, ownership, view, dispatch,
  mutability, escape, and initialization state.
  - Owner: `src/facts.rs`, `src/slot_shape.rs`, `src/signature.rs`
  - Evidence: each lattice has documented bottom/top, merge, precedence, and
    diagnostic behavior.
  - Implemented in `src/lattice.rs` with descriptors for nullability,
    ownership, view, dispatch, receiver mutability, escape, and
    initialization. Covered by
    `named_lattice_descriptors_define_core_domains`.

- [x] `LAT-2`: Replace ad hoc boolean merges in signature constraints with named merge
  functions.
  - Owner: `src/signature.rs`
  - Evidence: tests assert conflict resolution for public API, unknown API,
    escaping storage, object-preserving boundaries, and user overrides.
  - Implemented by routing ownership, view, dispatch, and storage-nullability
    joins through `lattice::join_ranked_lattice` plus named rank helpers.
    Boundary precedence is represented by
    `signature_boundary_kind_priority_policy`, and boolean-style requirements
    use `merge_signature_flag_requirement_policy`. Covered by
    `signature_requirement_merge_policy_owns_axis_priorities`,
    `signature_boundary_kind_merge_policy_owns_boundary_precedence`, and
    `signature_boundary_policy_owns_public_unknown_and_escape_constraints`.

- [x] `LAT-3`: Add a lattice audit test.
  - Owner: `tests/slot_shape_design.rs` or `tests/type_coercion.rs`
  - Evidence: monotonicity, idempotence, commutativity, and associativity are
    tested for core lattice merges.
  - Implemented by `core_signature_lattice_merges_satisfy_join_laws`, covering
    ownership, view, dispatch, and storage-nullability merge policies.
    Equal-rank variants with different payload names are intentionally left for
    `LAT-2`/`LAT-4`, where rejected evidence and provenance need explicit
    representation.

- [x] `LAT-4`: Add provenance to important lattice joins.
  - Owner: `src/signature.rs`, `src/facts.rs`
  - Evidence: diagnostics can say which constraint forced `owned`, `Option`,
    `Box<dyn Trait>`, erased object, or conservative clone.
  - Implemented by `SignatureConstraintProvenance`,
    `SignatureConstraintConflictDiagnostic`, and
    `MergedSignatureConstraints::conflict_diagnostics`. Conflict diagnostics now
    classify the kept and rejected sides of ownership, view, dispatch, and
    storage-nullability joins. Conservative clone remains routed through
    `signature_owned_recovery_policy` capability diagnostics.

- [x] `LAT-5`: Treat unsupported fallback as a lattice result, not a rendering surprise.
  - Owner: `src/conversion_plan.rs`, `src/coercion.rs`,
    `src/lowering_diagnostics.rs`
  - Evidence: unsupported ownership/dispatch/instanceof/conversion cases emit
    structured diagnostics before lowering renders a placeholder.
  - Implemented by plan-level `ConversionDiagnostic` and
    `unsupported_diagnostics` APIs on shape, slot, cast, `instanceof`, and
    borrowed-element conversion plans, plus
    `lowering_diagnostic_from_conversion_diagnostic` for standard lowering
    severity. Existing renderer-facing fallback diagnostics remain covered.

- [x] `LAT-6`: Make field nullability a proper finite lattice.
  - Owner: `src/semantic_analysis.rs`, `src/facts.rs`, `src/signature.rs`
  - Evidence: `Unknown`, `DefinitelyNull`, `DefinitelyNonNull`, and
    `MaybeNull` have explicit merge rules. Unknown external call assignment,
    nullable parameter assignment, helper assignment propagation, constructor
    delegation cycles, and null/non-null conflicts do not collapse toward
    non-null by accident.
  - Implemented with `lattice::field_nullability_join` and constructor/helper
    assignment propagation over the four `NullabilityState` values instead of
    a two-value assignment enum. Covered by
    `field_nullability_lattice_does_not_prove_unknown_assignments_non_null`,
    `field_nullability_lattice_preserves_nullable_constructor_conflicts`,
    `field_nullability_lattice_keeps_constructor_delegation_cycles_finite`,
    and explicit join assertions in
    `named_lattice_descriptors_define_core_domains`.
  - Tests: `cargo test --test semantic_facts field_nullability`

### 4. Source-Summary Envelope Selection

- [x] `SRC-1`: Add a source-summary recommendation report.
  - Owner: `src/resolver_benchmark.rs`
  - Evidence: when a focused package has project/source constructor or field
    guesses, the report lists likely sibling package roots that would explain
    them.
  - Implemented as `SourceSummaryRecommendation` rows in
    `src/resolver_benchmark.rs`. The report derives package-prefix
    recommendations from the same source/project guess ownership classifiers
    used by constructor, field, and method pressure counters, then renders
    `top_source_summary_recommendations` with package prefix, target type,
    constructor/field/method guess counts, total count, and sample files.
    Coverage: `resolver_benchmark_recommends_source_summary_prefix_for_project_guesses`
    plus existing source-summary and overlap benchmark tests.

- [x] `SRC-2`: Make source-summary roots selectable by package prefix.
  - Owner: `src/main.rs`, `src/dependency_mode.rs`
  - Evidence: CLI accepts package-prefix selectors or generated root lists
    without hard-coding GATK paths in code.
  - Ideal Rust Target: a focused package translation should be able to consume
    selected sibling/source-summary package facts without pulling the entire
    Java source tree into the emitted Rust crate. In Rust terms, generated
    modules should see inherited fields, inherited methods, constants, nested
    types, and callable signatures from the selected summary package as stable
    semantic facts, while only the primary/co-translated package emits Rust
    bodies. The target is a bounded dependency envelope, not a per-file
    reanalysis of all dependency bodies.
  - Implementation Rule: package-filtered roots are selected by
    `--source-root-package mode:path:package.prefix` for resolver/stage gates
    and by `--context-dir-package path:package.prefix` for translation
    context. Context loading first tries the normal Java package directory
    `<path>/<package/prefix>` and then validates each candidate's declared
    package before admitting it; only if that directory does not exist does it
    fall back to scanning the provided root. This keeps normal Java roots
    bounded while preserving correctness for nonstandard layouts. The current
    translation context still merges admitted source files into semantic
    analysis, so large `source-summary` envelopes remain a follow-up algorithm
    task: build reusable source-summary facts once and feed those facts to
    translation instead of solving every context body for every primary file.
  - Implemented with `--source-root-package mode:path:package.prefix`
    plus `DependencySourceRoot::package_prefixes`. Source indexing parses the
    file package before admitting a source-summary file, so broad dependency
    trees can be narrowed to recommended package prefixes without introducing
    GATK-specific paths. Coverage:
    `parse_args_accepts_resolver_bench_dependency_source_root_package_prefixes`,
    `parse_args_accepts_package_filtered_context_dir_for_translation`,
    `package_filtered_context_sources_only_load_matching_packages`, and
    `source_summary_root_package_prefix_filters_summary_files`.
    The GenomicsDB engine-context gate was stopped after it remained in
    translation with partial fragments only; that result confirms the bounded
    loader is necessary but not sufficient for large source-summary envelopes.
  - Ideal Rust Target: source-summary context used by translation should act
    like a reusable dependency fact envelope. Primary generated Rust should
    resolve calls, constructors, fields, static constants, and hierarchy
    declarations from the selected summary package, but representation,
    signature, control-flow, and body lowering should run only for emitted
    primary/co-translated source. Static string constants from summary context
    should still have one Rust definition when the primary file uses them:
    `Constants::NAME` with `pub const NAME: &'static str = "value";`.
  - Implementation Rule: translation now separates full context sources from
    summary context sources. Full context sources still enter semantic solving
    for tests and future co-translation use. Package-filtered
    `--context-dir-package` sources enter the resolver source program and
    support-local-type program, but not the semantic body program. Their
    declaration facts are added to the external fact set with
    `resolve::declaration_facts_from_program`, including type capabilities and
    field declaration facts. `call_plan` uses resolved call coercion evidence
    and resolved constructor params as finite boundary fallback shapes when a
    source-summary method/constructor has no solved body signature. Lowering
    seeds support-context type fields into each method/constructor environment
    and uses snake-case type aliases so member-access planning and rendering
    agree for static qualifiers. Static string support generation suppresses
    only types emitted by the primary program and uses support-context literal
    initializers when emitting dependency constants.
  - Required Facts: parsed source-summary declarations, resolver symbols,
    declaration field facts, type capability facts, resolved call coercion
    evidence, resolved constructor params, support-context static-final string
    initializers, and emitted-primary type identity.
  - Owning Layer: `lib` owns separating full context from summary context;
    `resolve` owns reusable declaration facts; `call_plan` owns summary-boundary
    argument fallback shapes; `expr_shape`/`ShapeEnv` own type-field aliases;
    `lower` owns seeding support-context declarations into lowering
    environments; `stdlib` owns static string support item emission.
  - Fallback Shape: if summary declarations are missing or incomplete,
    translation falls back to the existing unresolved/external boundary shapes
    and diagnostics. Summary bodies are not solved to recover missing facts in
    this mode; promotion to full context/co-translation remains an explicit
    dependency-mode decision.
  - Evidence:
    `summary_context_translation_resolves_sibling_symbols_without_lowering_bodies`
    proves summary symbols shape overload calls and static string constants
    without emitting sibling method bodies.
    `source_summary_root_contributes_declarations_without_benchmarking_bodies`,
    `context_translation_uses_sibling_static_string_constant_shape`,
    `context_translation_uses_sibling_overload_signature_for_call_target`, and
    `package_filtered_context_sources_only_load_matching_packages` pass.
    `cargo check` passes. The GenomicsDB engine source-summary gate at
    `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_engine_context_summary_v1`
    now completes instead of stalling in translation: files `6`,
    translation failures `0`, invalid-pattern files `1`, and
    `cargo_check_errors=82`. The remaining top buckets are now real inherited
    member/super/facade lowering gaps rather than a context-scaling failure.

- [x] `SRC-3`: Prevent overlapping source-summary roots from double-counting.
  - Owner: `src/resolver_benchmark.rs`, source index
  - Evidence: existing most-specific-root behavior is covered for methods,
    constructors, fields, object diagnostics, and fact counts.
  - Covered by
    `overlapping_source_summary_root_does_not_duplicate_primary_files`, which
    configures a broad source-summary parent and a more-specific primary root,
    then proves each file is assigned once while method, constructor, and field
    accesses resolve through the summary dependency.

- [x] `SRC-4`: Add neutral source-summary envelope tests.
  - Owner: `tests/dependency_mode.rs`
  - Evidence: synthetic primary package plus sibling helpers proves fields,
    constructors, nested classes, exceptions, and interface constants resolve
    through summaries.
  - Implemented as
    `source_summary_neutral_envelope_resolves_core_declaration_shapes`, a
    corpus-neutral fixture with a primary package consuming source-summary
    fields, constructors, a nested class, a nested exception class, and a
    static interface constant.

- [x] `SRC-5`: Decide co-translate vs source-summary criteria.
  - Owner: `docs/gatk-dependency-co-translation-plan.md`
  - Evidence: decision table names when bodies must be emitted, when summaries
    are sufficient, and when external packs are correct.
  - Implemented in `Mode Selection Criteria` in
    `docs/gatk-dependency-co-translation-plan.md`, including decision,
    promotion, demotion, and evidence rules for `co_translate_source`,
    `source_summary`, external fact packs, crate mappings, FFI boundaries, and
    conservative fallbacks.

### 5. Object Representation Fallback Matrix

- [x] `OBJ-1`: Decide the boxed-dyn invariant.
  - Owner: `src/object_model.rs`, `src/representation.rs`,
    `src/shape_projection.rs`
  - Ideal Rust Target: Java interface or superclass values that need borrowed
    dynamic dispatch should appear as `&dyn Trait`, while owned escaping
    polymorphic storage should appear as `Box<dyn Trait>` at the slot boundary.
    The object representation decision should stay at `DynTrait`; boxed dyn is
    a Rust storage projection, not a separate Java object model.
  - Implementation Rule: object representation planning decides whether a Java
    hierarchy can be modeled as concrete struct, closed enum, dyn trait,
    erased object, or unsupported. Slot projection then maps dyn-trait
    representation plus ownership/storage pressure into borrowed dyn or boxed
    dyn slots. Representation planning must not encode ownership-specific Rust
    storage forms as Java object representations.
  - Required Facts: hierarchy openness, interface/object-safety facts,
    dispatch requirements from usage, owned-storage pressure, borrowed-view
    pressure, and fallback object strategy facts from external packs or user
    overrides.
  - Owning Layer: `object_model` owns dispatch requirements;
    `representation` owns Java object representation; `shape_projection` owns
    borrowed versus owned dyn slot projection.
  - Fallback Shape: if dyn trait is not object-safe or ownership recovery
    cannot be proven, fall back to erased object or structured unsupported
    diagnostics rather than inventing a second object representation.
  - Evidence: tests assert one chosen invariant:
    - either `BoxedDynTrait` is not an `ObjectRepresentation` and owned
      polymorphic storage enters only through slot projection, or
    - `ObjectRepresentation::BoxedDynTrait` exists and maps through
      `RepresentationDispatchPolicy`.
  - Decision: `BoxedDynTrait` is not an `ObjectRepresentation`. It is a slot
    projection result for owned escaping/storage uses of a dyn-trait
    representation. Implemented by
    `boxed_dyn_object_representation_policy` and covered by
    `object_representation_policy_is_the_dyn_fallback_entrypoint`, which also
    asserts `NeedsOwnedPolymorphicStorage` projects dyn trait dispatch to
    `DispatchShape::BoxedDynTrait`.

- [x] `OBJ-2`: Complete the matrix for concrete struct, closed enum, dyn trait, boxed
  dyn trait, erased object, and unsupported hierarchy.
  - Owner: `src/representation.rs`, `src/object_model.rs`
  - Ideal Rust Target: every Java object role should have one documented Rust
    representation outcome: concrete source structs for ordinary translated
    classes, closed enums for proven closed value hierarchies, `dyn Trait`
    views for object-safe polymorphism, boxed dyn slots for owned polymorphic
    storage, `Box<dyn Any>` for erased runtime boundaries, or an explicit
    unsupported hierarchy diagnostic.
  - Implementation Rule: representation planning consumes the same matrix for
    public API, unknown API, storage, fields, locals, captures, external pack
    facts, non-object-safe interfaces, raw/wildcard objects, and closed/open
    hierarchies. New object decisions must add a matrix row or cell instead of
    branching in lowering.
  - Required Facts: source/dependency boundary, public/unknown API role,
    storage/capture role, hierarchy closure, object safety, generic/raw/
    wildcard status, external object strategy, and usage-derived dispatch
    pressure.
  - Owning Layer: `representation` owns matrix selection; `object_model` owns
    hierarchy/object-safety input; docs own the complete outcome table; tests
    assert every role has an outcome.
  - Fallback Shape: ambiguous or unsupported roles use erased object or
    explicit unsupported hierarchy diagnostics, with provenance, rather than
    allowing lowering to pick a shape opportunistically.
  - Evidence: one table in docs and one test group where every matrix cell has
    a positive or deliberate unsupported case.
  - Matrix roles: public API param/return, unknown API param/return, storage,
    field, local, captured value, external pack fact, non-object-safe
    interface, closed hierarchy, raw object, and wildcard object.
  - Implemented by the `Object Representation Fallback Matrix` table in
    `docs/rust-slot-shape-and-object-representation.md`. Covered by
    `object_representation_fallback_matrix_is_documented`, which checks every
    required role row has all outcome cells populated, and
    `object_representation_fallback_matrix_covers_core_outcomes`, which
    exercises concrete struct, closed enum, dyn trait, boxed-dyn slot
    projection, erased object, raw/wildcard erased fallback, and unsupported
    hierarchy diagnostics.

- [x] `OBJ-3`: Make `ObjectModelInput.usage` contribute dispatch requirements.
  - Owner: `src/object_model.rs`, `src/usage.rs`, semantic fact producers
  - Ideal Rust Target: Rust dispatch form should reflect how Java values are
    actually used. A value only borrowed for virtual calls can stay as a
    borrowed dyn view; a value stored or escaping behind polymorphism should
    use owned boxed dyn storage; raw/unknown runtime values should use erased
    carriers; and object-safety violations should produce diagnostics before
    Rust item emission.
  - Implementation Rule: usage analysis contributes dispatch requirements into
    object model planning before representation and slot projection. The
    representation layer merges fact-derived and usage-derived requirements
    once, so lowering and coercion do not rediscover dispatch pressure from
    individual call sites.
  - Required Facts: receiver call usage, storage/escape/capture usage,
    unknown-runtime-object usage, object-safety rejection facts, source and
    external hierarchy facts, and ownership pressure from signature slots.
  - Owning Layer: `usage` records body uses; semantic fact producers convert
    uses into dispatch requirements; `object_model` merges requirements;
    `representation` and `shape_projection` consume the merged plan.
  - Fallback Shape: conflicting or insufficient dispatch requirements fall
    back to erased object or explicit reject-trait-object diagnostics, not
    late downcasts or package-specific lowering.
  - Evidence: usage facts force `NeedsOwnedPolymorphicStorage`,
    `NeedsBorrowedTraitView`, `NeedsUnknownRuntimeObject`, and
    `RejectTraitObject` into representation/projection decisions.
  - Implemented by `dispatch_requirements_from_usage` and merged
    fact+usage requirements in `plan_object_model_from_input`. Covered by
    `object_model_input_usage_contributes_dispatch_requirements`, which checks
    usage-derived owned storage and borrowed trait requirements project through
    `dispatch_requirement_projection_policy`, while unknown-object and
    reject-trait-object requirements force erased-object fallback.

- [x] `OBJ-4`: Document and test erased-object runtime semantics.
  - Owner: `src/conversion_plan.rs`, `src/coercion.rs`, slot rendering,
    lowering
  - Ideal Rust Target: if an external/object representation fact says a Java
    type is an erased boundary, every Rust slot that stores, returns, passes, or
    nests that type in a native generic carrier should use the same erased
    carrier. For example, `Map<String, FeatureReader<VariantContext>>` with a
    Tribble erased-object pack should render as
    `HashMap<String, Box<dyn std::any::Any>>`, and inserting a value already
    shaped as `Box<dyn Any>` should not emit `downcast::<FeatureReader>()`.
  - Evidence: tests prove erased-to-concrete downcast, erased-to-dyn
    unsupported cases, anonymous erased boundaries, external-pack erased
    handles, store/pass/return/match behavior, and diagnostics all use the same
    runtime contract.
  - Implemented by `ErasedObjectRuntimeContract` and the `Erased Object
    Runtime Contract` section in
    `docs/rust-slot-shape-and-object-representation.md`. Covered by
    `erased_object_runtime_contract_covers_carrier_downcast_instanceof_and_recovery`,
    `erased_object_runtime_contract_is_documented_and_shared`,
    `erased_object_external_handle_lowers_store_pass_and_return_through_same_carrier`,
    anonymous erased-boundary tests, and dependency-mode erased external pack
    tests. Also fixed erased-to-erased dispatch with different declared Java
    names to remain an identity conversion. Slot rendering now consumes
    effective hierarchy override facts when recursively rendering native
    generic type arguments, so erased object values nested in `HashMap`, `Vec`,
    `HashSet`, or `Option` keep the `Box<dyn Any>` carrier. Native collection
    argument slot shaping has the same fact-aware path, so
    `Map.put`/`List.add`/`Set.add` request erased-object slots when the
    collection element type is an erased external boundary. Coverage:
    `slot_render_projects_erased_object_facts_inside_native_generic_containers`
    and the erased-object branch inside
    `call_plan_owns_call_and_new_target_selection_before_argument_lowering`.

- [x] `DYN-1`: Make boxed dyn clone-box requirements explicit.
  - Owner: `src/representation.rs`, `src/coercion.rs`,
    `src/representation_lowering.rs`
  - Ideal Rust Target: converting a borrowed dyn trait value into owned
    `Box<dyn Trait>` should only compile when a clone-box or equivalent owned
    recovery adapter is proven. If the adapter is unavailable, generated Rust
    should contain a deliberate unsupported diagnostic rather than a fake
    clone, invalid move, or erased downcast.
  - Implementation Rule: conversion planning classifies borrowed-dyn to
    boxed-dyn recovery through named adapter availability states. Coercion
    rendering consumes that classification; representation lowering is
    responsible only for emitting adapters that have been explicitly planned.
  - Required Facts: dyn trait type, source value ownership, clone/copy/
    clone-box capability, object safety, adapter availability, and target slot
    ownership requirement.
  - Owning Layer: `conversion_plan` owns adapter requirement classification;
    capability/effective facts own proof of recoverability; `coercion` renders
    the chosen adapter or diagnostic; `representation_lowering` owns emitted
    helper items.
  - Fallback Shape: missing or deferred adapters produce a structured
    clone-box diagnostic; they must not silently erase to `Box<dyn Any>` or
    assume `Clone` on `dyn Trait`.
  - Evidence: borrowed dyn to owned boxed dyn reports either proven adapter,
    required generated adapter, or unsupported non-cloneable fallback.
  - Implemented by `DynCloneBoxAdapterRequirement`,
    `DynCloneBoxAdapterAvailability`, and
    `dyn_clone_box_adapter_requirement_with_availability` in
    `src/conversion_plan.rs`, with renderer diagnostics in `src/coercion.rs`
    consuming the same classification. Covered by
    `dispatch_coercion_uses_clone_facts_for_borrowed_dyn_to_boxed_dyn_fallback`
    and design assertions in `coercion_plan_owns_clone_recovery_policy`.
    Adapter generation remains explicitly scoped to `DYN-2`.

- [x] `DYN-2`: Generate or explicitly defer dyn clone-box adapters.
  - Owner: `src/representation_lowering.rs`, `src/item_assembly.rs`
  - Ideal Rust Target: when clone-box support is in scope, generated Rust
    should include the helper trait/impl glue needed to clone object-safe trait
    objects into `Box<dyn Trait>`. When it is out of scope, the output should
    fail at a stable, documented diagnostic site instead of emitting invalid
    trait-object clone code.
  - Implementation Rule: adapter generation policy is a named conversion-plan
    outcome. Item assembly may emit helper items only for implemented policy
    states; deferred states remain explicit diagnostics consumed by coercion.
  - Required Facts: adapter requirement from `DYN-1`, object safety, trait
    cloneability, helper item availability, and whether the current run mode
    permits support-item emission.
  - Owning Layer: `conversion_plan` owns generation/defer policy;
    `representation_lowering` and `item_assembly` own helper item emission;
    `coercion` owns diagnostic rendering for deferred cases.
  - Fallback Shape: deferred generation remains a stable compile-error
    diagnostic with a specific code and test coverage.
  - Evidence if implemented: a focused trait-object fixture passes
    `cargo check` without a compile-error placeholder for clone-box recovery.
  - Evidence if deferred: docs mark adapter generation out of scope and tests
    assert the structured diagnostic is intentional.
  - Implemented as explicit deferral by
    `DynCloneBoxAdapterGenerationPolicy`,
    `DynCloneBoxAdapterDeferralReason`, and
    `dyn_clone_box_adapter_generation_policy_with_availability` in
    `src/conversion_plan.rs`. `src/coercion.rs` renders the same policy as a
    consuming `compile_error!` placeholder with the
    `unsupported_borrowed_dyn_to_owned_boxed_dyn_clone_box_adapter_deferred`
    diagnostic. Covered by
    `dispatch_coercion_handles_dispatch_shapes` and
    `dispatch_coercion_uses_clone_facts_for_borrowed_dyn_to_boxed_dyn_fallback`,
    plus design assertions in `coercion_plan_owns_clone_recovery_policy`.

- [x] `DYN-3`: Add trait-object method/body compatibility coverage.
  - Owner: `src/representation_lowering.rs`, object safety policy,
    `tests/object_model.rs`
  - Ideal Rust Target: object-safe Java interfaces should emit Rust traits
    whose receiver mutability, parameter slots, return slots, and implementor
    forwarding methods are mutually compatible. Returned polymorphic values
    should use `Box<dyn Trait>` only at owned boundaries; borrowed parameters
    should use `&dyn Trait` or `&mut dyn Trait` according to method effects.
  - Implementation Rule: trait item emission is program-aware: it considers
    implementor receiver requirements and method compatibility before
    rendering trait and impl methods. Object-safety and overload compatibility
    are checked before emission; lowering does not repair incompatible trait
    methods after the items are written.
  - Required Facts: interface methods, implementor methods, receiver
    mutability, overload compatibility, generic/self references, return and
    argument slot shapes, object-safety diagnostics, and boxed/borrrowed dyn
    projection requirements.
  - Owning Layer: object-safety policy owns compatibility facts;
    `representation_lowering` owns trait and impl item rendering; `signature`
    owns method slot shapes; generated-crate tests own end-to-end Rust checks.
  - Fallback Shape: incompatible methods or non-object-safe signatures produce
    stable diagnostics and fall back to erased/unsupported representation
    policy rather than emitting uncompilable trait items.
  - Evidence: generated Rust compiles for an object-safe interface with
    multiple implementors, returned boxed dyn values, borrowed dyn params,
    overload-compatible impl methods, receiver mutability, generic/self
    references, and return adapters. Incompatible impls produce stable
    diagnostics.
  - Implemented with program-aware dyn-trait item emission in
    `src/representation_lowering.rs`: trait method receivers now account for
    compatible implementor receiver requirements before rendering the trait and
    impl forwarding methods. The real lowering path uses
    `raw_type_item_for_program_with_facts_and_exceptions`, preserving the older
    isolated policy helpers for narrow tests. Coverage includes
    `DynTraitMethodCompatibility.java` in the generated Rust compile gate,
    end-to-end assertions for multiple implementors, borrowed dyn params,
    boxed dyn return/storage boundaries, mutable receiver forwarding, existing
    overload-compatible and incompatible implementation diagnostics, and
    object-safety tests for direct/nested self and generic references.

- [x] `OBJ-5`: Support source-summary superclass boundaries systematically.
  - Owner: `src/representation.rs`
  - Ideal Rust Target: when primary source extends a summary-only dependency
    class whose body is not emitted, the Rust type should not pretend to
    inherit fields or methods from a missing Rust superclass. It should use an
    explicit erased or external-boundary representation while still consuming
    declaration facts for resolution, constructors, methods, and hierarchy
    pressure.
  - Implementation Rule: dependency mode and source-summary facts classify
    superclass edges before representation. Source-summary superclass edges
    become representation facts or erased-boundary policies; co-translated
    superclass edges remain real hierarchy pressure. Lowering must not branch
    on package names or missing superclass text.
  - Required Facts: dependency mode, superclass source-summary/co-translation
    status, generic superclass aliases, constructor/method declarations,
    object strategy overrides, and primary-source ownership.
  - Owning Layer: dependency mode/source-summary analysis owns source-root
    classification; `representation` owns superclass boundary policy;
    resolver/effective facts expose declarations and aliases.
  - Fallback Shape: unknown or non-emitted superclass bodies fall back to
    erased/external boundary diagnostics; ordinary inherited state remains
    unsupported until real inheritance lowering is designed.
  - Evidence: primary source extending a summary-only framework class becomes
    an erased boundary, while co-translated superclass edges still require real
    hierarchy handling.
  - Implemented by `SourceSummarySuperclassBoundaryPolicy`,
    `source_summary_superclass_boundary_policy`, and
    `extend_representation_facts_for_source_summary_superclass_boundaries` in
    `src/representation.rs`. The resolver benchmark now calls this
    representation-owned policy instead of computing erased summary
    superclass overrides locally. Coverage includes direct representation
    tests for source-summary vs co-translated superclass modes, generic
    superclass aliases such as `FrameworkBase<Payload>`, responsibility guard
    assertions, and the dependency-mode regression
    `primary_extends_source_summary_base_is_erased_boundary_not_inheritance_pressure`.

- [x] `OBJ-5b`: Flatten inherited source-support instance fields into emitted
  primary Rust structs.
  - Owner: `src/inherited_members.rs`, `src/signature.rs`,
    `src/item_assembly.rs`, `src/constructor_plan.rs`, `src/lower.rs`
  - Ideal Rust Target: when an emitted Java class inherits ordinary instance
    fields from a superclass that is available as source-support or
    source-summary context, the generated Rust struct should contain those
    fields directly under the subclass's Rust field names. Constructor
    emission should initialize those fields through the same field-value plan
    used for local fields, and method bodies should read/write them as
    `self.field_name`. Static constants remain associated constants or support
    constants; they are not flattened into instance state.
  - Implementation Rule: `inherited_members` walks the superclass chain in the
    support program and returns only non-static fields not shadowed by a local
    field. `signature` plans each inherited field under the emitted subclass
    `FieldKey`, `item_assembly` emits the corresponding Rust field,
    `constructor_plan` adds it to the constructor field-value plan, and
    `lower` seeds the method/constructor shape environment so receiverless
    field accesses use the same planned field shape. Rendering does not add
    superclass-name or field-name special cases.
  - Required Facts: parsed superclass edge, support/source-summary type
    declarations, field staticness, shadowing by direct subclass fields,
    storage slot shape, and initializer/default field-value plan.
  - Owning Layer: `inherited_members` owns hierarchy field discovery;
    `signature` owns slot shape; `item_assembly` owns struct field emission;
    `constructor_plan` owns initialization plans; `lower` only consumes the
    shape environment and planned field values.
  - Fallback Shape: if the superclass declaration is unavailable, no inherited
    field is flattened and the existing external/unsupported member diagnostics
    remain responsible for the unresolved access.
  - Evidence:
    `inherited_source_summary_instance_fields_are_flattened_into_primary_struct`
    proves a summary-only superclass field is emitted in the primary struct,
    initialized from the inherited declaration, and read as `self.field`.

- [x] `OBJ-6`: Strengthen closed-world enum proof.
  - Owner: `src/representation.rs`, `src/facts.rs`
  - Ideal Rust Target: Java sealed or otherwise proven closed hierarchies may
    become Rust enums with one variant per reachable concrete subtype. Open
    hierarchies, missing permitted subtypes, non-final variants, and external
    worlds without closure proof should not become enums; they should remain
    dyn/erased/unsupported according to the representation matrix.
  - Implementation Rule: closed-enum representation consumes finite proof
    facts from parsed sealed permits, external closed facts, user overrides,
    and subtype reachability. The proof policy names every rejection state so
    representation never guesses closed-world safety from currently parsed
    files alone.
  - Required Facts: sealed permits, known direct subtypes, final/sealed status,
    external closure facts, omitted subtype diagnostics, user overrides, and
    hierarchy reachability over the finite fact graph.
  - Owning Layer: parser/facts record permit and subtype facts; effective facts
    own reachability; `representation` owns closed-enum proof policy and enum
    representation selection.
  - Fallback Shape: incomplete or open-world proof uses dyn trait, erased
    object, or unsupported hierarchy diagnostics rather than an enum with
    missing runtime cases.
  - Evidence: tests cover sealed `permits`, external closed facts, user
    overrides, omitted subtype diagnostics, and open-world fallback.
  - Implemented by adding parsed `SealedPermitsFact` evidence, an
    `EffectiveFacts::sealed_permitted_subtypes_for` query, and explicit
    `ClosedEnumProofPolicy` states for proven hierarchies, open-world fallback,
    missing direct subtypes, missing permitted subtypes, and non-final variants.
    Closed enum inference now consumes the proof policy while preserving the
    existing fact/inference facades. Coverage includes sealed permits fact
    recording, missing-permit fallback, external closed-fact proof, non-final
    rejection, open-world fallback, and responsibility guards that keep proof
    ownership in the fact/effective-fact/object-model layers.

- [x] `CAST-1`: Split Java cast semantics into explicit categories.
  - Owner: `src/conversion_plan.rs`, `src/coercion.rs`, `src/types.rs`
  - Ideal Rust Target: Java casts should become Rust numeric casts,
    reference-preserving upcasts, checked runtime predicates, unchecked generic
    boundary diagnostics, optional/result adapter policies, or explicit
    invalid-cast panics according to named Java semantics. The emitted Rust
    should never contain a nominal downcast merely because a Java cast token
    appeared in the source.
  - Implementation Rule: `JavaCastSemanticsPolicy` classifies the Java cast
    category before representation-shaped coercion is planned. Coercion then
    renders the category-specific Rust operation or diagnostic. Primitive,
    reference, generic/raw/wildcard, optional/result, impossible, and fallback
    casts must be named policy outcomes, not local `if type_name == ...`
    branches.
  - Required Facts: source and target Java types, primitive/reference status,
    hierarchy relationship, generic/raw/wildcard status, optional/result
    carrier status, representation outcome, and invalid-cast runtime policy.
  - Owning Layer: `types` owns Java type predicates; `conversion_plan` owns
    cast semantic classification and shape pairing; `coercion` owns renderer
    spelling and diagnostics.
  - Fallback Shape: unsupported or unchecked casts produce structured cast
    diagnostics or explicit runtime panic policy, not guessed Rust downcasts.
  - Evidence: primitive numeric casts, reference upcasts, checked downcasts,
    unchecked generic casts, raw/wildcard casts, impossible casts, invalid-cast
    panic policy, optional/result policy, and diagnostic-only fallback each
    have named policy outcomes and tests.
  - Implemented by `JavaCastSemanticsPolicy` and
    `java_cast_semantics_policy` in `src/conversion_plan.rs`, with Java type
    predicates such as `is_numeric_primitive_type`,
    `contains_wildcard_type`, `is_raw_generic_counterpart`,
    `is_unchecked_generic_counterpart`, and `is_optional_or_result_type` in
    `src/types.rs`. `CastConversionPlan` now carries the semantic category
    next to the representation-shaped coercion plan. Covered by
    `cast_semantics_policy_names_primitive_and_impossible_casts`,
    `cast_semantics_policy_names_reference_upcasts_checked_downcasts_and_fallbacks`,
    `cast_semantics_policy_names_generic_raw_wildcard_and_optional_result_casts`,
    `conversion_plan_owns_cast_and_instanceof_shape_pairing_before_lowering`,
    and `coercion_plan_owns_invalid_cast_runtime_policy`.

- [x] `CAST-2`: Make `instanceof` hierarchy/capability aware.
  - Owner: `src/conversion_plan.rs`, `src/coercion.rs`, `src/facts.rs`,
    `src/object_model.rs`
  - Ideal Rust Target: Java `instanceof` should become a Rust predicate only
    when the runtime representation can answer it: concrete type tags,
    closed-enum variant checks, dyn-trait capability checks, erased-object
    helper predicates, or hierarchy-aware interface tests. Primitive,
    raw/wildcard, or missing-runtime-target checks should become structured
    diagnostics rather than invalid Rust type tests.
  - Implementation Rule: `InstanceOfTargetPolicy` plans the target shape using
    representation outcomes, hierarchy facts, and capability facts. Coercion
    rendering consumes the planned predicate/fallback variant. Lowering passes
    fact context into the planner and must not perform hierarchy/package
    checks itself.
  - Required Facts: expression representation, target Java type, hierarchy
    reachability, closed enum proof, dyn/boxed/erased runtime support,
    primitive/raw/wildcard rejection facts, and capability facts from external
    packs.
  - Owning Layer: `facts`/effective facts own hierarchy reachability and
    capability queries; `object_model`/`representation` own representation
    outcomes; `conversion_plan` owns `instanceof` policy; `coercion` renders
    the planned check.
  - Fallback Shape: unprovable runtime target support emits a structured
    `instanceof` diagnostic, not a partial predicate or downcast.
  - Evidence: tests cover concrete target, closed enum target, dyn target,
    boxed dyn target, erased object target, interface target with closed
    implementors, primitive target rejection, raw/wildcard target rejection,
    and missing runtime target diagnostics.
  - Implemented by `InstanceOfTargetPolicy`,
    `plan_instanceof_check_with_facts`, and `hierarchy_instanceof_policy` in
    `src/conversion_plan.rs`, with recursive subtype reachability centralized
    in `HierarchySubtypePolicy` / `FactSet::hierarchy_subtype_policy`.
    Renderer-facing messages in `src/coercion.rs` consume the same
    `InstanceOfFallbackPolicy` variants. Covered by
    `instanceof_target_policy_names_supported_runtime_target_shapes`,
    `instanceof_rejects_primitive_raw_wildcard_and_missing_runtime_targets`,
    `instanceof_uses_hierarchy_facts_for_interface_targets_with_closed_implementors`,
    existing dyn/boxed/erased predicate tests, and design assertions in
    `coercion_plan_owns_instanceof_fallback_policy`.

- [x] `CAST-3`: Make `instanceof` and casts consume representation outcomes only.
  - Owner: `src/conversion_plan.rs`, `src/coercion.rs`, `src/lower.rs`
  - Ideal Rust Target: cast and `instanceof` rendering should be a mechanical
    consequence of a precomputed conversion/representation plan. The final
    Rust should show the selected cast, predicate, adapter, or diagnostic
    without lower-level code rechecking Java package names, hierarchy, or
    object strategy.
  - Implementation Rule: lowering obtains cast and `instanceof` plans from
    `conversion_plan` using representation/effective facts, then delegates all
    rendering to coercion helpers. Any new semantic category must be added to
    the planner/policy types first and guarded by tests that lowering only
    consumes the plan.
  - Required Facts: planned cast semantics, planned runtime target policy,
    representation outcome, hierarchy/capability facts, source and target
    shapes, and fallback diagnostic policy.
  - Owning Layer: `conversion_plan` owns planning boundary;
    `representation`/effective facts own semantic inputs; `coercion` owns
    rendering; `lower` owns only syntax traversal and plan invocation.
  - Fallback Shape: if no plan can be produced, lower a structured unsupported
    conversion/`instanceof` diagnostic rather than branching in `lower.rs`.
  - Evidence: guard tests prove `lower.rs` does not rediscover hierarchy or
    package names for casts/instanceof.
  - Implemented by `CastAndInstanceOfPlanningBoundary` in
    `src/conversion_plan.rs` and enforced by
    `lower_consumes_cast_and_instanceof_plans_without_rechecking_representation_facts`.
    `lower.rs` passes representation/fact context into `plan_cast_conversion`
    and `plan_instanceof`, then renders via `apply_cast_conversion` and
    `apply_instanceof`; it does not call representation, hierarchy,
    capability, cast-semantics, or `instanceof` target classifiers itself.

- [x] `CAP-1`: Centralize remaining direct type checks into capability
  policies.
  - Owner: `src/conversion_plan.rs`, `src/facts.rs`, `src/effective_facts.rs`
  - Ideal Rust Target: Rust output should be driven by type capabilities such
    as castability, runtime-test support, clone/copy recovery, object safety,
    sequence element behavior, and optional/result carrier semantics. Except
    for Java primitives that map directly to Rust numeric operations, emitted
    shape should not depend on direct package/type spelling checks in lowering
    or coercion.
  - Implementation Rule: nonprimitive type classification is exposed through
    named capability policies on `FactSet`/`EffectiveFacts`. Conversion,
    signature, and representation planners query those capabilities and record
    policy outcomes before rendering. Direct type predicates are allowed only
    in the type/fact policy layer or for documented primitive handling.
  - Required Facts: generic/raw/wildcard status, optional/result carrier
    status, runtime target support, clone/copy/owned-recovery capability,
    external pack capability rows, object-safety facts, and primitive type
    classification.
  - Owning Layer: `effective_facts` owns capability policy queries; `facts`
    owns capability provenance; planners consume capability outcomes; lowering
    and renderer layers are guarded against direct nonprimitive checks.
  - Fallback Shape: missing capability evidence yields conservative policy
    outcomes and diagnostics, not package-name branches.
  - Evidence: guard scans show new coercion decisions call capability/policy
    helpers instead of branching directly on package/type spellings, except
    for documented Java primitives.
  - Implemented for cast and `instanceof` planning by
    `CastTypeCapabilityPolicy` and `InstanceOfTargetCapabilityPolicy` in
    `src/effective_facts.rs`, exposed through matching `FactSet` facade
    methods. `src/conversion_plan.rs` now calls
    `facts.cast_type_capability_policy` and
    `facts.instanceof_target_capability_policy` instead of importing raw
    wildcard/generic/optional-result type predicates; direct primitive checks
    remain documented because primitive values drive Rust numeric casts and
    primitive-target diagnostics. Covered by
    `cast_and_instanceof_type_capability_policy_centralizes_nonprimitive_type_checks`
    and source guard assertions in
    `slot_policy_and_signature_solver_use_type_capability_facts`.

- [x] `CAP-2`: Extend owned-recovery capability beyond copy/clone/non-cloneable.
  - Owner: `src/facts.rs`, `src/conversion_plan.rs`, external packs
  - Ideal Rust Target: borrowed-to-owned recovery should use the Rust operation
    that the type boundary actually supports: copy for copy-like values, clone
    or clone-box for cloneable values, factory-copy adapters for external
    handles, immutable shared handles where ownership is logically shared, and
    explicit unsupported transfer diagnostics for facade/FFI-owned handles
    that cannot be duplicated. Reading an owned field through an immutable
    `&self` receiver into an owned Java-shaped return or argument slot should
    recover ownership with the same capability model, for example
    `self.name.clone()` or `self.paths.clone()`, rather than moving the field
    out of `self`.
  - Implementation Rule: owned recovery is a finite capability fact with
    distinct variants, queried through effective facts and consumed by
    conversion/signature planning. Coercion maps each capability to one Rust
    adapter or diagnostic. No lowering rule may infer recoverability from a
    method name or dependency package.
  - Required Facts: target type, source view/ownership, copy/clone/factory/
    shared-handle/FFI capability, external pack provenance, element recovery
    context, receiver mutability, field-read provenance, and target slot
    ownership requirement.
  - Owning Layer: `facts` records `OwnedRecoveryCapabilityFact`;
    `effective_facts` is the query point; `conversion_plan` and `signature`
    classify recovery; `coercion` renders adapters/diagnostics.
  - Fallback Shape: unknown or unsupported recovery emits a structured
    ownership-transfer diagnostic, preserving borrowed shapes where possible.
  - Evidence: clone-box, factory-copy, immutable shared handle,
    facade-owned external handle, and FFI-owned handle facts drive different
    ownership recovery outcomes without lowering package-name checks.
  - Implemented as `OwnedRecoveryCapabilityFact` plus richer
    `OwnedRecoveryCapability` variants. `EffectiveFacts` is the single query
    point; coercion maps clone-box/factory-copy to proven clone recovery,
    immutable shared handles to copy recovery, and facade/FFI handles to
    explicit unsupported transfer diagnostics. Signature owned-recovery policy
    preserves the richer classifications. Covered by
    `owned_recovery_capability_facts_drive_distinct_coercion_outcomes`,
    `borrowed_dyn_recovery_uses_clone_box_capability_fact_once`,
    `borrowed_element_recovery_uses_richer_owned_recovery_facts`,
    `signature_owned_recovery_policy_classifies_capability_proofs`, and source
    guard assertions in `coercion_plan_owns_clone_recovery_policy`.
    Lowering now also marks reads of owned fields through shared `&self`
    receivers as requiring owned recovery when the target slot remains owned.
    This emits `.clone()` for owned Java-shaped return and argument slots
    instead of moving out of `self`, while still allowing borrowed private
    helper arguments such as `&str` to use borrowed views. Coverage includes
    `SharedSelfFieldOwnedRecovery.java` and
    `shared_self_field_reads_recover_owned_values_without_moves`. The focused
    GenomicsDB gate for this slice reports `cargo_check_errors=106`, down from
    `109`, with zero tracked invalid patterns; the previous `cannot move out
    of self.workspace`, `self.reference`, and
    `self.sample_name_to_vcf_path` residuals are gone.

- [x] `CAP-3`: Tie dependency packs into object/capability facts.
  - Owner: `src/external_packs.rs`, `src/external_facts.rs`,
    `src/representation.rs`, `src/conversion_plan.rs`
  - Ideal Rust Target: external dependency decisions should influence emitted
    Rust through the same object, capability, nullability, effect, constructor,
    field, crate/facade, and unsupported-member facts used for source code.
    A modeled dependency pack should make calls and slots look like the chosen
    Rust boundary, not like Java library syntax patched by package-specific
    lowering.
  - Implementation Rule: built-in and parsed external packs emit fact rows
    with provenance and boundary audits. Representation, signature,
    conversion, resolver, and lowering consume those facts through normal
    planning APIs. A pack-owned missing member remains a coverage diagnostic
    attached to that boundary instead of falling through to generic guessing.
  - Required Facts: pack boundary ownership, API effects, argument effects,
    constructors, fields, object strategy, object safety, type capabilities,
    owned recovery, nullability, exception effects, crate/facade mapping, and
    unsupported-member coverage.
  - Owning Layer: `external_packs`/`external_facts` own manifest and built-in
    fact emission; dependency-mode audits own boundary coherence;
    representation/conversion/signature consume facts through normal planners.
  - Fallback Shape: unowned or incoherent pack rows produce manifest/boundary
    diagnostics; unsupported pack-owned members stay explicit rather than
    becoming ad hoc lowering rules.
  - Evidence: neutral GATK-shaped fixtures show pack-owned calls, fields, and
    constructors affect hierarchy, object safety, final/clone/copy,
    erased-object, sequence element, nullability, and unsupported-member
    facts, with benchmark attribution by provider mode.
  - Implemented for parsed and built-in external pack facts through
    manifest-driven boundary, object strategy, object-safety, type capability,
    owned-recovery capability, field nullability, API effect, argument effect,
    exception, constructor, field, crate mapping, and unsupported-member
    fact emission. `external-pack-owned-recovery` rows now emit
    `OwnedRecoveryCapabilityFact`s, participate in boundary audits, feed
    representation-fact lookup, and count as provider facts. Existing
    dependency-mode manifest tests cover calls, fields, constructors, erased
    superclass boundaries, object-safety fallback, provider-mode attribution,
    unsupported members, and pack-owned boundary diagnostics. New neutral
    GATK-shaped coercion tests cover factory-copy, immutable shared handle,
    facade-owned handle, and FFI-owned handle recovery kinds without lowering
    package-name checks. Java platform facade support now emits shared opaque
    Rust boundary carriers for `Path`, `URI`, and `File` when platform imports
    or platform-typed signatures require them. The support item gives those
    carriers ordinary Rust value semantics plus the minimal path/URI conversion
    methods already emitted by the translator (`to_uri`, `to_absolute_path`,
    `to_file`, `to_path`, `resolve_sibling`, and `get_file_name`) instead of
    leaving raw unresolved Java type names in generated Rust.
  - Focused Gate: the GenomicsDB gate with generated package prelude plus
    platform facades reports `cargo_check_errors=308`, zero tracked
    invalid-pattern buckets, and no remaining top-error rows for missing
    `Path` or `URI` types. This is down from `323` after package prelude alone.
    The focused harness now also hoists shared support (`JavaException`,
    `Path`, `URI`, and `File`) once into the generated crate root and imports
    those names into modules after stripping their per-module support preludes.
    The aggregate GenomicsDB error count remains `308`, but the remaining
    errors now refer to crate-root support carriers such as `Option<URI>`
    instead of distinct module-local facade types. The exposed residual is
    deeper external facade modeling and display/conversion planning for
    optional external values (`logger`, `i_o_utils`, `GATKVCFConstants`,
    builder/protobuf/header types, and `Option<URI>` display).
    Optional platform-facade display in format/string-concat slots is now
    planned by conversion policy rather than URI-specific lowering:
    `Option<URI>`/`Option<Path>` display segments render through a generic
    `as_ref().map(|value| value.to_string()).unwrap_or_else(...)` shape while
    `Option<String>` keeps the Java null-string `as_deref().unwrap_or("null")`
    shape. The focused GenomicsDB gate for this slot-only slice reports
    `cargo_check_errors=309`; the previous direct `Option<URI>: Display`
    formatting bucket disappears, but Rust now reports the real next
    ownership issue where optional URI arguments are moved by `unwrap()` before
    later display use. The remaining `Option<&URI>.to_string()` rows come from
    Java method-call planning on map-get results, not from format-slot
    conversion, and should be handled with optional/map-get value policy rather
    than another display special case.
    Java instance calls on lowered optional-borrow receivers now use Java null
    receiver semantics instead of display fallback. In particular,
    `map.get(key).toString()` over a native Rust `HashMap<K, URI>` lowers as
    `map.get(&key).expect("Java null receiver").to_string()`, which removes the
    focused GenomicsDB `Option<&URI>.to_string()` error bucket. The same slice
    accepts normalized Java collection spellings (`contains_key`, `key_set`,
    `entry_set`) in call planning, and `keySet()` lowers to an owned key vector
    via `.keys().cloned().collect::<Vec<_>>()`. The focused GenomicsDB gate for
    map-get receiver plus keySet lowering reports `cargo_check_errors=308` with
    zero tracked invalid patterns. The keySet rows are no longer raw Java
    method names; the exposed residual is target-shape work for Rust collection
    results (`len()` as `usize` in Java `int` comparisons, `Vec::get(i)`
    returning `Option<&T>`, and chained `entry_set().iterator().next().get_key()`
    over method-returned maps).
    FFI-owned handle, unowned owned-recovery rows, and invalid owned-recovery
    kinds without lowering package-name checks.

### 6. Signature And Slot-Shape Co-Solving

- [x] `SIG-1`: Expand constraints for all boundary roles.
  - Owner: `src/signature.rs`
  - Ideal Rust Target: every Java value boundary that can affect Rust
    ownership or mutability should have an explicit Rust slot shape before
    lowering: params, returns, locals, fields, constructor params, receivers,
    captures, call arguments/results, and constructor invocations. The emitted
    Rust should not rely on a default `T`/`&T` guess for any role that can
    escape, mutate, borrow, or store values.
  - Implementation Rule: `signature` maintains a complete inventory of
    boundary roles and creates constraints through role-specific helpers.
    New lowering surfaces must add a role or prove they reuse an existing
    role; ad hoc slot selection inside call, expression, or statement lowering
    is not allowed.
  - Required Facts: binding role, source/dependency/API boundary, mutability
    pressure, ownership/escape pressure, nullability, receiver role, capture
    role, call target role, and constructor result role.
  - Owning Layer: `signature` owns role inventory, constraint generation, and
    slot solving; usage/semantic facts provide pressure; lowering consumes
    solved slots.
  - Fallback Shape: missing role coverage is a signature diagnostic or
    conservative slot policy, not a late renderer guess.
  - Evidence: params, returns, locals, fields, constructor params, receiver
    slots, captured values, call args, and call results all have tests.
  - Implemented as the public `SignatureBoundaryRole` inventory and
    `signature_boundary_role_policies()` in `src/signature.rs`. The inventory
    covers method params/returns, constructor params, method and constructor
    locals, fields, method receivers, captured values, call arguments, call
    results, and constructor invocations, with an owning helper for each role.
    Coverage includes
    `signature_boundary_role_inventory_covers_all_sig1_roles`,
    `signature_boundary_constraints_cover_params_locals_fields_returns_and_receivers`,
    existing capture boundary tests, and
    `signature_dependency_graph_records_captured_value_nodes_from_facts`.

- [x] `SIG-2`: Make public/unknown API fallback type-aware across all Java types.
  - Owner: `src/signature.rs`, `src/slot_shape.rs`
  - Ideal Rust Target: public and unknown API boundaries should expose the
    most precise Rust shape that is safe for the Java type category: primitives
    as Rust primitives, strings as `String`/borrowed string views according to
    role, arrays and collections as native containers when modeled, nullable
    values as `Option<T>`, erased/raw/wildcard objects as opaque carriers, and
    external facades as their declared Rust boundary type.
  - Implementation Rule: fallback slot policy is type-aware and capability-
    aware. It consumes Java type classification, object representation, and
    external/facade facts before choosing public/unknown boundary shapes.
    Public API fallbacks are centralized in signature/slot policy, not copied
    into individual lowerers.
  - Required Facts: Java type category, primitive/boxed/string/array/
    collection status, optionality, object representation, raw/wildcard
    status, external facade mapping, erased-object facts, and public/unknown
    boundary role.
  - Owning Layer: `signature` owns public/unknown fallback policy;
    `slot_shape` owns category-to-slot projection; effective facts provide
    object and capability evidence.
  - Fallback Shape: unknown reference types use conservative owned or opaque
    shapes with provenance; they must not become untyped `Box<dyn Any>` unless
    the representation policy selects an erased boundary.
  - Evidence: tests cover primitive, boxed primitive, string, arrays,
    collections, optional, raw object, wildcard, interface, enum, external
    mapped facade, and erased object.
  - Implemented through the existing `signature_boundary_policy` and
    `signature_native_boundary_policy` path, backed by type capabilities and
    object-representation facts from `EffectiveFacts`/`slot_shape`. Coverage:
    `signature_public_unknown_fallback_is_type_aware_across_java_type_categories`
    explicitly checks primitive, boxed primitive, string, array, collection,
    optional, raw object, wildcard, interface, enum, facade handle, and erased
    object cases for both public and unknown API boundaries.

- [x] `SIG-3`: Co-solve field nullability and signature optionality.
  - Owner: `src/semantic_analysis.rs`, `src/signature.rs`
  - Ideal Rust Target: Java fields proven initialized to non-null values on
    all constructor paths should render as plain `T`; fields that can be null,
    are cleared to null, lack constructor coverage, or cross unknown external
    boundaries should render as `Option<T>` or a documented opaque fallback.
    Constructor-proven maps/lists/sets should avoid unnecessary
    `Option<HashMap<...>>` or `Option<Vec<...>>` carriers.
  - Implementation Rule: semantic field-nullability facts and constructor
    coverage constraints feed the same signature solver that decides field
    slot optionality. The final field shape is solved once and consumed by
    field rendering, constructor lowering, and assignment conversion.
  - Required Facts: field initializers, constructor assignments, constructor
    delegation cycles, null-clearing assignments, external nullability
    overrides, field visibility/API role, and reference-vs-primitive category.
  - Owning Layer: `semantic_analysis` owns field-nullability facts and
    constructor coverage; `signature` owns optional field slot solving;
    conversion/lowering consume the solved field slot.
  - Fallback Shape: missing or cyclic constructor proof is conservative
    `Option<T>` for nullable references; proven non-null fields reject null
    writes through structured diagnostics.
  - Evidence: constructor-proven non-null fields render as non-`Option`, while
    null assignments, missing constructor coverage, and external unknowns
    render as `Option` or conservative diagnostics.
  - Implemented by semantic field-nullability facts feeding
    `field_nullability_shape_policy` and
    `RequiredStorageNullability` constraints in `src/signature.rs`; solved slot
    shapes are then rendered by the normal slot-render path. Coverage includes
    `field_nullability_and_signature_optionality_are_co_solved_for_constructor_coverage`,
    `field_nullability_external_override_beats_constructor_proof`,
    `nullable_field_storage_uses_option_shape_and_assignment_wrapping`,
    `field_nullability_shape_policy_optionalizes_only_nullable_reference_storage`,
    `variantcontext_nullable_attribute_getter_flows_into_option_slot_planning`,
    and `constructor_initialized_variantcontext_field_stays_non_optional`.

- [x] `SIG-4`: Co-solve getter/setter elimination with field ownership.
  - Owner: `src/semantic_analysis.rs`, `src/signature.rs`,
    `src/call_lowering.rs`
  - Ideal Rust Target: trivial Java accessors should disappear into direct
    Rust field reads/writes only when doing so preserves API requirements,
    visibility, mutability, and ownership. Public or override-sensitive
    accessors should remain ordinary Rust methods/wrappers even if their bodies
    are simple.
  - Implementation Rule: semantic analysis records trivial accessor facts with
    an inline/preserve/analyze-only policy. Usage and signature solving may
    use accepted accessor facts as backing-field evidence, while call planning
    decides whether a specific call lowers to field access or remains a
    method. Lowering does not inline by recognizing `get`/`set` names.
  - Required Facts: getter/setter body shape, backing field key, visibility,
    override/API preservation policy, receiver mutability, argument ownership,
    field slot shape, and call-site resolution.
  - Owning Layer: `semantic_analysis` owns accessor facts; `usage` translates
    accepted accessors into field pressure; `signature` solves field slots;
    `call_plan` owns call-site inline selection; `call_lowering` renders the
    selected form.
  - Fallback Shape: preserve accessor calls when API, visibility, overload, or
    ownership proof is insufficient.
  - Evidence: trivial accessors can be inlined only when API preservation,
    mutability, field visibility, and ownership constraints permit it.
  - Status: Semantic analysis records `TrivialAccessorFact` with
    `AccessorInlinePolicy`; usage analysis treats accepted accessor calls as
    backing-field uses for ownership/mutability evidence; `src/call_plan.rs`
    now gates direct field lowering on `InlineCandidate`, so `PreserveApi` and
    `AnalyzeOnly` facts remain available to analysis without forcing call-site
    inlining. Coverage includes
    `records_trivial_getter_and_setter_facts`,
    `preserves_public_trivial_accessor_calls_when_api_wrappers_must_survive`,
    `lowers_package_private_trivial_accessor_calls_as_field_access`,
    `usage_treats_trivial_accessor_calls_as_backing_field_uses`, and
    `call_plan_owns_trivial_accessor_call_selection_before_lowering`.

- [x] `SIG-5`: Keep external API mutability and argument effects in the solver.
  - Owner: `src/semantic_analysis.rs`, `src/signature.rs`
  - Ideal Rust Target: calls into modeled external APIs should shape Rust
    receivers and arguments according to declared effects: read-only receivers
    as shared borrows, mutating receivers as mutable bindings/borrows,
    borrowed arguments as borrowed slots, escaping arguments as owned or
    longer-lived slots, and mutating arguments as mutable references.
  - Implementation Rule: external API effect facts and argument effect facts
    become signature constraints before lowering. The solver merges them with
    usage and source facts, so external setters, builders, indexed getters,
    and collection-like APIs affect slot shapes without API-name branches in
    renderers.
  - Required Facts: receiver type/method key, receiver mutability, role
    (getter/setter/fluent/indexed/side-effect), argument effects, return
    nullability/type, external override precedence, and unknown-boundary
    fallback coverage.
  - Owning Layer: `semantic_analysis` collects external effect facts;
    `signature` turns them into constraints; external packs/user facts own
    provenance; lowering consumes solved receiver/argument slots.
  - Fallback Shape: if an external argument boundary is not covered by facts,
    keep the generic conservative unknown-boundary policy and report the
    missing coverage rather than guessing mutability from method names.
  - Evidence: external setters, fluent builders, indexed getters, borrowed
    arguments, escaping arguments, and mutating arguments affect slot shapes
    without lowering-specific API branches.
  - Status: Existing external API effect facts already drive receiver
    mutability, fluent/indexed roles, return nullability, and user overrides.
    `src/signature.rs` now also turns `ExternalApiArgumentEffectFact` into
    signature constraints for borrowed, escaping, and mutating arguments. When
    every unknown argument boundary for a binding is covered by such facts, the
    solver uses those fact constraints instead of the generic unknown-boundary
    fallback; otherwise it stays conservative. Coverage includes
    `external_argument_effects_shape_method_params_in_signature_solver`,
    `external_setter_effect_requires_mutable_receiver_binding_in_signature`,
    `external_api_user_override_beats_later_name_guess`,
    `external_api_receiver_specific_override_beats_global_override_in_signature`,
    `external_collection_api_effects_are_receiver_specific_facts`,
    and `external_map_api_effects_distinguish_read_and_write_methods`.
    Local storage uses a narrower receiver-mutability projection: a local
    mutated through an external receiver call remains an owned storage value
    with `binding_mutable=true`, rather than becoming an `&mut T` local slot.
    Coverage includes
    `external_setter_effect_keeps_local_storage_owned_and_mutable`; the
    GenomicsDB focused gate now emits
    `let mut import_config: Box<dyn std::any::Any> = ...` instead of
    `let mut import_config: &mut Box<dyn std::any::Any> = &mut ...`.
  - Ideal Rust Target: public helper parameters used only as modeled
    read-only external receivers should borrow when the helper returns an
    obvious read product such as `String`, primitives, arrays, maps, sets,
    lists, collections, iterators, `Optional`, or `Result`. Helpers that
    return opaque external objects or same-identity replacement values should
    keep owned parameter boundaries, even when the receiver calls themselves
    are read-only.
  - Implementation Rule: `signature` suppresses the public API native
    ownership fallback only for fully modeled read-only receiver bindings
    whose method return type is an obvious read product and not the same
    canonical Java boundary identity as the parameter. Read-only receiver
    borrow constraints are now separately gated from mutating receiver borrow
    constraints, so public opaque-product helpers do not inherit a shared-borrow
    parameter shape from receiver effects alone.
  - Required Facts: binding uses including `ReceiverForCall` and compatible
    `ContentRead`, external API receiver mutability facts, method return type,
    parameter type, canonical Java type identity, and read-product return
    classification.
  - Owning Layer: `usage` records receiver/content-read use facts; `facts`
    owns external API effect lookup; `signature` owns public-boundary
    suppression, read-product classification, canonical identity comparison,
    and receiver borrow constraints. Lowering only consumes solved parameter
    shapes.
  - Fallback Shape: if the binding has non-read uses, the external receiver is
    not modeled read-only, the return is opaque, or the return has the same
    Java boundary identity as the parameter, preserve the owned/conservative
    public boundary and let ordinary owned-recovery/coercion rules adapt call
    sites.
  - Evidence:
    `readonly_external_receiver_usage_can_borrow_public_helper_param`,
    `readonly_same_type_public_helper_param_stays_owned_for_replacement_return`,
    and
    `readonly_public_helper_returning_opaque_external_product_keeps_param_owned`.
    The GenomicsDB focused gate keeps `cargo_check_errors=101`; the prior
    `vid_map_p_b` moved-value residual is gone because the pure map-building
    helper now borrows the optional carrier with `.as_ref().expect(...)`, while
    protobuf replacement helpers keep owned parameters.
    Current limitation: `BindingUse::PassedToCall` does not yet retain receiver
    type, so argument-effect constraints are receiver-agnostic until usage facts
    are extended.

- [x] `SIG-6`: Add a signature-solver saturation report.
  - Owner: `src/signature.rs`, `src/resolver_benchmark.rs`
  - Ideal Rust Target: signature solving should be inspectable as a finite
    convergence process. Benchmarks should show how many constraints, joins,
    SCC passes, fallback reasons, and conflicts produced the Rust slot shapes,
    so suboptimal emitted Rust can be traced to a fact/constraint gap instead
    of reverse-engineered from generated code.
  - Implementation Rule: signature solving reports finite-domain statistics,
    pass/SCC information, join conflicts, fallback provenance, and conflict
    axes through a reusable report consumed by resolver benchmarks. The report
    is diagnostic evidence; it does not change slot decisions.
  - Required Facts: constraint states, provenance, dependency graph/SCCs,
    fixpoint iteration counts, join conflict states, fallback reasons, and
    conflict axes.
  - Owning Layer: `signature` owns saturation reporting; graph/SCC helpers own
    dependency structure; `resolver_benchmark` owns aggregate presentation.
  - Fallback Shape: missing report data is a benchmark/reporting gap, not a
    reason for lowering to infer slot causes locally.
  - Evidence: focused benchmarks report constraints, joins, iterations,
    conflicts, and top fallback reasons.
  - Implemented by `SignatureSaturationReport`,
    `signature_saturation_report`, and
    `signature_saturation_report_for_constraints` in `src/signature.rs`.
    Reports include finite-domain stats, fixpoint iterations, diagnostics,
    dependency/pass SCC stats, join-conflict counts, top constraint provenance
    fallback reasons, and top conflict axes. `src/resolver_benchmark.rs`
    aggregates these into `signature_join_conflicts`,
    `top_signature_fallback_reasons`, and `top_signature_conflict_axes` rows.
    Coverage includes a synthetic conflicting constraint-state report test,
    source responsibility guards, and the resolver benchmark signature counter
    test.

### 7. Java Type Inference And Coercion Evidence

- [x] `TYPE-1`: Make overload applicability produce structured rejection reasons.
  - Owner: `src/resolve.rs`, `src/types.rs`
  - Ideal Rust Target: Java overload resolution should select the same
    callable Java would select before Rust lowering starts, and failed or
    ambiguous candidates should produce structured diagnostics explaining
    primitive widening, boxing/unboxing, varargs, generic, null, lambda, or
    method-reference incompatibility. Rust output should never depend on
    lowering trying overload candidates again.
  - Implementation Rule: resolver produces an applicability report for each
    candidate and selected overload, preserving argument-level conversion and
    rejection reasons. Later stages consume the selected call plus evidence;
    they do not rescore overloads.
  - Required Facts: candidate signatures, argument Java types, primitive
    conversion rules, boxing/unboxing, varargs, generic substitutions, null
    conversion, lambda/method-reference target types, and ambiguity scores.
  - Owning Layer: `resolve` owns overload candidate reporting and selection;
    `types` owns Java conversion predicates; diagnostics consume structured
    rejection reasons.
  - Fallback Shape: ambiguous or unsupported overloads remain resolver
    diagnostics and opaque call boundaries, not lowering-time guesses.
  - Evidence: tests cover primitive widening, boxing/unboxing, varargs,
    generics, null arguments, lambdas, method references, and ambiguous calls.
  - Status: `src/resolve.rs` now exposes `OverloadApplicabilityReport`,
    `OverloadArgumentApplicability`, `OverloadArgumentConversion`,
    `OverloadRejectionReason`, and `overload_applicability_report` so overload
    candidate checks produce structured conversion/rejection data instead of
    only string diagnostics. Existing method/constructor selection behavior is
    preserved for this slice; the report mirrors the current resolver scoring
    vocabulary and is ready for later selection diagnostics. Coverage includes
    `overload_applicability_reports_primitive_boxing_unboxing_and_rejections`,
    `overload_applicability_reports_varargs_generics_and_ambiguous_scores`,
    and
    `overload_applicability_reports_lambda_and_method_reference_targets`.

- [x] `TYPE-2`: Preserve generic substitutions through inheritance.
  - Owner: `src/resolve.rs`, `src/types.rs`
  - Ideal Rust Target: inherited Java generic methods and fields should lower
    with substituted Rust types at the use site. A `Derived extends
    Base<String>` call to `get()` should feed a `String` slot, interface-chain
    bounds should preserve their declared/extends type, and wildcard bounds
    should not collapse to untyped `Object` unless the Java type is genuinely
    erased.
  - Implementation Rule: resolver carries substitution maps along superclass
    and interface edges when resolving inherited members. The selected member
    stores substituted return/parameter types for downstream type inference,
    slot planning, and call lowering.
  - Required Facts: superclass/interface type arguments, inherited member
    declarations, wildcard declared/extends bounds, raw-type erasure status,
    branch-specific substitution maps, and selected member provenance.
  - Owning Layer: `resolve` owns inherited member traversal and substitutions;
    `types` owns substitution normalization; later planners consume resolved
    substituted types.
  - Fallback Shape: missing or raw substitutions fall back to erased/unknown
    type shapes with provenance, not guessed concrete generics.
  - Evidence: `Base<T>.get()` called through `Derived extends Base<String>`
    resolves as `String`, including interface chains and wildcards.
  - Status: `src/resolve.rs` now resolves inherited method symbols through
    both superclass and implemented-interface edges while carrying a
    per-branch substitution map. Supertype binding normalizes wildcard actuals
    to readable declared/extends bounds, so inherited generic method returns
    can feed later field lookup and overload resolution. Coverage includes
    existing superclass-chain tests such as
    `resolver_substitutes_inherited_generic_return_type_through_superclass_chain`
    and new interface coverage in
    `resolver_substitutes_inherited_generic_return_type_through_interface_chain`
    plus
    `resolver_uses_extends_wildcard_bound_for_inherited_interface_return`.

- [x] `TYPE-3`: Make diamond inference a resolver result.
  - Owner: `src/types.rs`, `src/resolve.rs`
  - Ideal Rust Target: Java diamond constructors should produce Rust values
    with the target-inferred generic type, such as `Vec<String>`-like or
    source generic shapes, including nested constructor arguments. Untargeted
    diamond construction should remain raw/unknown instead of inventing type
    parameters from later lowering context.
  - Implementation Rule: resolver records the constructed type and substituted
    constructor parameters on `ResolvedConstructor` using assignment/call
    target context and nested constructor targets. Call/new planning consumes
    that resolver result directly.
  - Required Facts: constructor declaration, target slot Java type, generic
    parameter list, nested argument target types, subtype/interface relation
    between constructed and target types, raw fallback state, and selected
    constructor provenance.
  - Owning Layer: `resolve` owns diamond inference and selected constructor
    evidence; `types` owns substitution; `call_plan` consumes the constructed
    type for new-target planning.
  - Fallback Shape: no target or unsolved parameters preserve raw/unknown
    construction with diagnostics rather than lowering-time generic guesses.
  - Evidence: one-unknown, multiple-unknown, nested diamond, target-typed
    assignment, and fallback raw-type tests.
  - Status: `ResolvedConstructor` now carries resolver-owned
    `constructed_type` and substituted `params`. `src/resolve.rs` fills this
    from target-typed local declarations, including source/known subtype
    targets such as `MyArrayList<T> implements MyList<T>`, and recursively
    applies selected constructor parameter targets to nested constructor
    arguments. Untargeted/raw construction remains raw. `src/call_plan.rs` now
    reads the resolved constructed type when planning `new` targets, so later
    representation/lowering stages consume the inference result instead of
    re-deriving diamond facts. Coverage includes
    `resolver_records_target_typed_diamond_constructed_types`.

- [x] `TYPE-4`: Emit coercion evidence from resolver/type inference.
  - Owner: `src/resolve.rs`, `src/types.rs`, `src/conversion_plan.rs`
  - Ideal Rust Target: argument and constructor conversions should be rendered
    from Java coercion evidence selected during resolution: primitive
    widening, boxing, unboxing, null/reference conversion, generic inference,
    and lambda/method-reference target adaptation. The emitted Rust should
    show the required adapter once, not rediscover conversion rules in every
    call renderer.
  - Implementation Rule: resolved calls and constructors carry
    `JavaCoercionEvidence`; `conversion_plan` maps that evidence into
    conversion plan nodes before coercion/lowering. Cast-backed conversions
    reuse the cast semantics policy rather than a parallel call-specific
    conversion path.
  - Required Facts: selected overload/constructor, per-argument conversion
    kind, primitive/boxed/reference relationship, generic substitutions,
    target functional-interface type, cast category, and target slot shape.
  - Owning Layer: `resolve` owns Java coercion evidence; `types` owns Java
    conversion predicates; `conversion_plan` owns Rust adapter planning;
    lowerers consume conversion plans.
  - Fallback Shape: missing coercion evidence falls back to structured
    conversion diagnostics or conservative target-slot adaptation, not
    renderer-local Java coercion rules.
  - Evidence: conversion planner can distinguish Java primitive widening,
    boxing, unboxing, cast, unchecked cast, and target-typed lambda adaptation.
  - Status: `src/java_ir.rs` now carries Java-level `JavaCoercionEvidence`
    and `JavaCoercionKind` on resolved method and constructor calls.
    `src/resolve.rs` emits that evidence from the same overload applicability
    report used for candidate scoring, so primitive widening, boxing,
    unboxing, null/reference conversion, generic inference, and lambda/method
    reference adaptation are produced once at resolution time.
    `src/conversion_plan.rs` exposes `JavaTypeConversionPlan` and
    `JavaTypeConversionKind`, including evidence-backed call/constructor
    conversions and cast-backed conversion plans that reuse
    `JavaCastSemanticsPolicy` for checked, unchecked generic, raw/wildcard,
    optional-result, primitive, and fallback casts. Coverage includes
    `resolver_records_java_coercion_evidence_for_selected_call_arguments`,
    `resolver_records_java_coercion_evidence_for_selected_constructor_arguments`,
    `conversion_plan_classifies_resolver_coercion_evidence_and_casts`, and the
    conversion responsibility guard in
    `conversion_plan_owns_cast_and_instanceof_shape_pairing_before_lowering`.

- [x] `TYPE-5`: Make class literals typed tokens.
  - Owner: `src/types.rs`, `src/stdlib.rs`, `src/call_lowering.rs`
  - Ideal Rust Target: Java `Type.class` and `Class<T>` values should become
    typed Rust class tokens such as `JavaClass<T>`, preserving bounds when
    known and erasing raw/unbounded tokens deliberately. Reflection helpers
    such as `getSimpleName` or `Class.forName` should operate on that token
    boundary rather than on stringly Java syntax.
  - Implementation Rule: class literals are planned as stdlib/class-token
    expressions with a Java type parameter before lowering. Slot rendering and
    call lowering consume the same token type for literal, variable, and
    reflection-call cases.
  - Required Facts: class literal target type, `Class<T>` generic argument,
    wildcard bounds, raw/unbounded erasure, reflection call target, and
    support-item availability for token helpers.
  - Owning Layer: `types` owns class-token type modeling; `stdlib` owns class
    literal and reflection call planning; `call_lowering` renders planned
    token operations.
  - Fallback Shape: unsupported reflection remains an explicit class-token or
    reflection diagnostic, not package-name lowering or raw strings.
  - Evidence: `Type.class`, `Class<T>`, `getSimpleName`, `forName`, and
    reflection fallback tests do not require package-name lowering checks.
  - Status: class literals now lower to a `JavaClass<T>` token through
    `src/stdlib.rs` and `src/call_lowering.rs` instead of going through
    unsupported-expression lowering. `Class<T>` slot rendering uses the same
    token type, bounded wildcard class tokens preserve their bound, unbounded
    wildcard/raw class tokens erase to `JavaClass<()>`, and `Class.forName`
    has a dedicated call target plus support-item emission. Coverage includes
    `class_literal_tokens_are_planned_before_lowering`,
    `class_for_name_uses_java_class_token_support`, and the class-literal
    idiom-gap fixtures for registration and token comparison.

### 8. Conversion Planner And Rust Adapter Coverage

- [x] `NULL-1`: Make nullable locals, params, fields, and returns slot-shaped.
  - Owner: `src/signature.rs`, `src/types.rs`, `src/literal_lowering.rs`,
    `src/conversion_plan.rs`, statement rendering
  - Ideal Rust Target: Java bindings that can hold `null` should have Rust
    slot types such as `Option<T>` at the declaration, parameter, field, or
    return boundary where nullability is part of the value contract. A Java
    `null` literal should never render as an untyped `None`; it should lower
    through the target slot as `None`, `Some(...)`, or a structured diagnostic.
    Non-null slots should stay as plain `T` when facts prove all assignments,
    constructors, and returns are non-null.
  - Implementation Rule: source nullability constraints are gathered before
    rendering and merged through the signature/slot-shape solver. Literal and
    assignment lowering consume the solved slot shape through
    `conversion_plan`; they do not decide optionality from a local null token
    alone. Return and parameter optionality propagate through finite method
    facts, so recursive or mutually recursive nullability still converges.
  - Required Facts: null initializers, null assignments, null comparisons,
    returned nullable bindings, constructor field initialization, parameter
    null pressure, source/external return nullability, and public/unknown
    boundary fallback policy.
  - Owning Layer: `semantic_analysis` owns nullability constraints;
    `signature` owns slot projection; `conversion_plan` owns literal and
    assignment adapters; literal/statement lowering renders only the planned
    optional shape.
  - Fallback Shape: when nullability is plausible but unproven at a
    public/unknown boundary, prefer `Option<T>` or an opaque external boundary
    with provenance. When a slot is proven non-null, reject null assignments
    with a structured diagnostic rather than emitting `T = None`.
  - Evidence: `examples/idiom-gaps/NullableParameterAndLocal.java` emits
    `Option<T>` where required, never `T = None`, and null literals are lowered
    with target-slot awareness rather than raw literal rendering.
  - Tests: convert the current negative idiom fixture into positive assertions
    and include it in the generated Rust compile gate.
  - Status: `src/signature.rs` now derives finite source-nullability
    constraints for bindings from `null` initializers/assignments and
    `== null` / `!= null` tests, then applies the same constraints to
    method/constructor params, locals, and method returns when nullable
    bindings are returned. The existing slot-shape solver and conversion
    planner own the final `Option<T>` shape and `None`/`Some(...)` lowering.
    Coverage includes `documents_nullable_concrete_object_gap`,
    `source_nullability_constraints_shape_locals_params_and_returns`, and
    `examples/java/05-types/NullableLocalReturn.java` in the generated Rust
    compile gate.

- [x] `COLL-1`: Route Java collection APIs through collection/iterator plans.
  - Owner: `src/stdlib.rs`, `src/conversion_plan.rs`, `src/call_lowering.rs`,
    `src/stream_lowering.rs`
  - Ideal Rust Target: modeled Java collections should lower to native Rust
    collection APIs at the boundary where their semantics match: `Vec<T>`,
    `HashSet<T>`, `HashMap<K, V>`, slices/iterators for borrowed views,
    `.len() as i32` for Java `size()`/`length`-as-`int`, `.is_empty()`,
    `.push`, `.extend`, `.insert`, `.get`, `.contains_key`, `.iter`, and
    `retain` for supported predicates. Borrowed elements should be
    cloned/copied only through the generic owned-recovery policy required by
    the target slot.
  - Implementation Rule: collection constructors, receiver calls, argument
    slots, receiver mutability, and iterator views are planned before generic
    call rendering. `call_plan`/stdlib identify modeled collection operations;
    conversion planning adapts element/key/value ownership; lowering renders
    only planned native calls or explicit facade/unsupported boundaries.
  - Required Facts: collection kind, generic key/value/element types,
    receiver optionality, receiver mutability, argument ownership effects,
    element recovery capability, constructor capacity/initial elements,
    iterator source shape, and external/facade collection boundary facts.
  - Owning Layer: `stdlib` and `call_plan` own collection operation planning;
    `signature` owns receiver/argument slots; `conversion_plan` owns element
    recovery; `call_lowering` and `stream_lowering` render native collection
    and iterator forms.
  - Fallback Shape: unsupported Java collection APIs remain facade calls or
    structured collection diagnostics; generated Rust should not contain raw
    Java `.addAll`, `.size`, `.put`, or `Map` spellings for native containers.
  - Evidence: `size`, `isEmpty`, `add`, `addAll`, `put`, `get`, `entrySet`,
    `containsKey`, `removeIf`, and iteration fixtures emit Rust collection
    APIs or explicit facade calls.
  - Gate expectations: `Vec::new`, `HashMap::<K, V>::new`,
    `HashSet::<T>::new`, `.push`, `.extend`, `.insert`, `.get`,
    `.len() as i32` for Java `size()` results, and no `.add_all`, `.size`,
    `.put`, or raw `Map` for native Rust containers.
  - Fixtures: `CollectionAddAllAndSize.java`, `MapPutGetJavaApi.java`,
    `DiamondSingleUnknown.java`, `DiamondNestedUnknowns.java`.
  - Status: Java collection receiver calls now plan to native Rust collection
    operations in `src/call_plan.rs` and render through `src/call_lowering.rs`:
    `size`/`isEmpty` to `len`/`is_empty`, list `add` to `push`, set `add` to
    `insert`, `addAll` to `extend`, map `put`/`get`/`containsKey`/`entrySet`
    to `insert`/`get`/`contains_key`/`iter`, `Arrays.asList` to `vec![...]`,
    and simple predicate `removeIf` to inverted `retain`. Collection
    constructors now lower to native `Vec`, `HashSet`, or `HashMap`
    constructors, `Map<K,V>` slot rendering emits `HashMap<K,V>`, and mutating
    collection receivers request mutable locals or mutable-borrow params.
    Coverage includes the four listed fixtures plus
    `CollectionRemoveIfLambda.java`, `CollectionCapacityConstructor.java`, and
    the collection target design checks in `tests/slot_shape_design.rs`.
    One-argument native collection constructors are now classified as capacity
    constructors when the argument is integral or a planned `size()` call, so
    `new ArrayList<>(names.size())` emits `Vec::with_capacity(...)` instead of
    returning the capacity expression as the collection value.
    The GATK-focused collection slice also handles Java `int` comparisons
    against native collection lengths by casting the Rust length at the
    comparison boundary, and handles `Vec`/sequence `get(i)` through a slot
    shape rule that emits `get(i as usize).expect("Java index out of bounds")`
    plus normal borrowed-element recovery. Map-entry iterator chains are now
    lowered as map/iterator shapes rather than raw Java calls:
    `entrySet().iterator().next().getKey()` becomes
    `.iter().next().expect("Java iterator exhausted").0` with the owning slot
    deciding whether the borrowed key is cloned. When a focused package lacks
    cross-file method-return facts, the entry-chain fallback uses the Java
    map-entry chain as map-like evidence and the target slot as key-type
    evidence instead of preserving `.entry_set().iterator().next().get_key()`.
    Coverage includes `MapKeySetIntLoop.java`,
    `MapEntrySetFirstKey.java`, and
    `MapEntrySetFirstKeyFromNullableField.java`. The focused GenomicsDB gate
    for the map-entry fallback reports `cargo_check_errors=301`, zero tracked
    invalid patterns, and the `java_collection_api` idiom signal drops to
    `3` occurrences in `1` file; the remaining collection signal is in
    `GATKGenomicsDBUtils`, not the `SampleNameMap` entry-chain call.
    Java collection and map `size()` receiver calls now render as
    `.len() as i32` in `call_lowering`, preserving Java `int` return width
    instead of leaking Rust `usize` into public Java-shaped slots. Coverage
    includes `MapSizeReturnInt.java`, `CollectionAddAllAndSize.java`, and
    `constructor_initialized_sorted_map_field_is_not_optional_carrier`. The
    focused GenomicsDB gate for this slice reports
    `cargo_check_errors=110`, down from `111`, with zero tracked invalid
    patterns.
    Native map insertion now has its own expression carrier rule:
    - Ideal Rust Target: Java `Map.put(k, v)` used as an expression should
      preserve Java's "previous value or null" result as Rust `Option<V>`.
      For a native `HashMap<K,V>`, the ideal Rust is
      `let old: Option<V> = map.insert(k, v);` followed by ordinary
      `old.is_none()` / `old.unwrap()` null refinement when needed. The
      insertion result should not be wrapped as `Some(map.insert(...))`,
      because Rust `HashMap::insert` already returns `Option<V>`.
      Java `Map.get(k)` flowing into a required `V` slot should become
      `map.get(&k).expect("Java null receiver")` followed by normal
      borrowed-element recovery such as `.clone()` for owned `String`.
      Boxed primitive values such as Java `Integer` flowing into primitive
      `int` slots should use the native Rust map carrier directly:
      `map.get(&k).copied().expect("Java null receiver")`.
    - Implementation Rule: `call_plan::native_collection_lowered_type`
      classifies `MapInsert` as an owned optional value carrier derived from
      the map value generic. Existing argument-slot planning still owns key
      and value ownership. `lower_expr_for_slot` then routes expression-position
      `Map.put` through the normal lowered-slot conversion path, so assigning
      to nullable slots is identity and assigning to non-null slots becomes an
      explicit optional unwrap/fallback decision. Required-value `Map.get`
      slots use a map-get-specific slot path parallel to sequence `get`: the
      Rust `Option<&V>` is unwrapped at the Java null-receiver boundary and
      then adapted by the normal slot conversion policy. Optional borrowed
      carriers can now also target a concrete element slot; for boxed
      primitive Java element types, target matching accepts the corresponding
      primitive slot and selects copy recovery before the Java null-boundary
      unwrap. Field-backed expression-position map inserts use the same
      call-target argument lowering as statement calls; when target slot typing
      is imprecise, clone recovery may use the source expression shape for
      clone capability while still requiring an owned target slot and
      repeated-binding usage facts.
    - Required Facts: receiver collection kind, map key/value generic types,
      target slot nullability, existing collection argument slot shapes, and
      binding-use facts for cloned owned insert arguments.
    - Owning Layer: `call_plan` owns the `MapInsert` actual carrier;
      `lower` owns required-value map-get unwrapping and clone-on-reused
      insert arguments from usage facts; `conversion_plan` owns
      optional-to-target and borrowed-element adaptation; `call_lowering` only
      renders `.insert`/`.get`.
    - Fallback Shape: if the receiver is not proven to be a native map, keep
      the facade/fallback call rather than assuming `HashMap::insert`
      semantics.
    - Evidence: `native_collection_lowered_type` now reports an optional value
      slot for `MapInsert`, and
      `translates_map_put_previous_value_as_optional_without_double_wrapping`
      proves emitted Rust uses `values.insert(key, value)` directly with no
      `Some(values.insert(...))` wrapper. `documents_map_put_get_java_api_gap`
      proves reused map keys are cloned for insertion and required map-get
      return values unwrap and clone from the borrowed Rust map entry.
      `field_map_put_reused_key_clones_at_owned_boundary` proves the same
      reused-key rule applies to field-backed expression-position map inserts
      without cloning one-shot constructor arguments.
      `cargo check --quiet` passes. The focused GenomicsDB gate with
      `htsjdk.tribble.feature` and `com.intel.genomicsdb` packs moves from
      `cargo_check_errors=146` to `145` at
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_map_insert_get_carrier`;
      the `E0308` mismatched-type bucket drops from `54` to `47`. Remaining
      move errors in `SampleNameMap` are now constructor/string ownership
      issues such as `URI::new(vcf_path)` consuming Java strings that are used
      later, not map carrier mismatches.
    Constructor arguments that consume reused Java bindings have their own
    ownership rule:
    - Ideal Rust Target: a Java constructor call should not consume a binding
      that Java can still read later. If the Rust constructor slot requires an
      owned value and usage facts show the source binding is reused, the ideal
      Rust is `Type::new(value.clone())` for clone-capable owned values, or a
      borrowed constructor surface such as `Type::new(value.as_str())` when the
      target API is modeled as borrowing/string-view based. For `java.net.URI`,
      a path string reused after construction should lower like
      `let uri = URI::new(path.clone());` rather than moving `path`.
    - Implementation Rule: constructor lowering consumes the resolved
      constructor argument slot shapes from `plan_constructor_arguments`.
      Before rendering `apply_new_target_with_facts`, it applies a generic
      argument ownership adapter when all of these are true: the argument is a
      local binding, the constructor slot requires owned storage, the binding
      is used more than once in the current callable, and the argument type
      has a clone recovery capability. This is a slot/usage rule, not a
      `URI`-specific branch.
    - Required Facts: resolved constructor argument slot shape, current
      callable usage facts, binding Java type, ownership shape, and clone or
      borrowed-view capability facts.
    - Owning Layer: `call_plan` owns constructor argument slot shapes;
      `usage` owns repeated-binding facts; `slot_shape`/capabilities own
      clone eligibility; `lower` applies the mechanical owned-argument
      recovery; future external API models may replace clone recovery with
      borrowed constructor slots when those APIs are known to accept borrowed
      views.
    - Fallback Shape: if the binding is not known to be reused, emit the
      ordinary owned argument. If clone capability is unavailable, do not guess
      a clone; keep the existing expression shape and let the compile gate or
      structured ownership diagnostics expose the missing capability.
    - Evidence: add a neutral `java.net.URI` fixture proving a reused
      `String` constructor argument is cloned at the constructor boundary and
      remains available for later Java operations. Re-run focused idiom tests,
      `cargo check`, and the GenomicsDB compile gate to confirm the
      `URI::new(vcf_path)` move errors shrink without adding library-specific
      branches.
    External opaque constructor facades have their own Rust overload rule:
    - Ideal Rust Target: Java constructors for an external/opaque type should
      render as legal Rust associated functions. If only one constructor arity
      is known for the type, the facade can use idiomatic `Type::new(...)`. If
      the Java type has multiple modeled constructor arities, Rust call sites
      and support must use stable arity-specific names such as
      `SimpleInterval::new_1(locatable)` and
      `SimpleInterval::new_3(contig, start, end)`, because Rust cannot overload
      `new`.
    - Implementation Rule: `call_plan` owns constructor target naming. For
      unresolved external constructor targets, it checks external constructor
      facts for the constructed type; a single modeled arity keeps `new`, while
      multiple modeled arities select `new_{arity}`. `stdlib` independently
      collects actually used constructor arities from expression trees and
      emits matching opaque facade support methods. `stream_lowering` consumes
      the same call-plan constructor naming policy through its render context,
      so stream lambdas do not hardcode `Type::new(...)`.
    - Required Facts: constructed Java type, constructor arity, external
      constructor facts by type, local type inventory including nested local
      classes, and stream render facts.
    - Owning Layer: `call_plan` owns constructor target names; `stdlib` owns
      opaque constructor facade support and nested-local exclusion;
      `stream_lowering` only asks `call_plan` for the unresolved constructor
      name when rendering stream lambda `new` expressions.
    - Fallback Shape: types with no evidence of multiple arities keep `new`.
      If facts later prove multiple arities, call sites and support switch
      together to `new_{arity}`. Local source constructors keep resolver-owned
      names such as `new_path` or `new_string_int` and are excluded from
      external support facades.
    - Evidence:
      `external_constructor_support_uses_arity_names_for_overloaded_facades`
      proves neutral emitted Rust uses `ExternalBox::new_1`,
      `ExternalBox::new_2`, and matching support methods. `cargo check` passes.
      The focused GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_external_constructor_facades_v2`
      moves from the previous comparable `cargo_check_errors=78` to `73`.
      `SimpleInterval` constructor calls in ordinary expressions and stream
      lambdas now render as `new_1`/`new_3`, and duplicate support definitions
      for nested local classes are gone after local-type inventory became
      recursive.
    Opaque external facade equality/hash/display fallbacks are a separate
    support rule:
    - Ideal Rust Target: an opaque external Java value used at a fallback
      equality, hash collection, or stringification boundary should remain a
      legal Rust value. Direct Java identity/equality fallback can compile
      through `PartialEq`/`Eq` on the facade, Java object membership in
      `HashSet`/`HashMap` can compile through `std::hash::Hash`, and Java
      `toString()`/display fallback can compile through `std::fmt::Display`.
      The ideal support is a conservative facade implementation such as
      `impl PartialEq for ExternalToken { ... }`,
      `impl Eq for ExternalToken {}`,
      `impl std::hash::Hash for ExternalToken { ... }`, and
      `impl std::fmt::Display for ExternalToken { ... }`.
    - Implementation Rule: `stdlib` emits conservative `PartialEq`, `Eq`,
      `Hash`, and `Display` impls for generated opaque external facade types
      after alias filtering. This is support-surface behavior only; it does
      not make semantic equality, hash-code, or formatting claims for
      translated source types, closed enums, or fully modeled external crate
      mappings.
    - Required Facts: generated opaque facade type inventory, alias filtering,
      and local type inventory. No lowering-time Java method branch is needed.
    - Owning Layer: `stdlib` owns the fallback support impls; `call_plan` and
      `operator_lowering` still own when equality or stringification is
      selected; modeled APIs may later replace fallback behavior with typed
      methods or crate mappings.
    - Fallback Shape: fallback `PartialEq` returns a conservative equality
      value, fallback `Hash` contributes no state, and fallback `Display`
      renders an empty string. These are compile facades, not semantic
      translations of object identity, Java `hashCode()`, or human-readable
      formatting.
    - Evidence:
      `external_opaque_facades_support_equality_and_display_fallbacks` proves
      generated support contains `PartialEq`, `Eq`, `Hash`, and `Display` for
      an opaque external value used in equality and `toString()` contexts. The
      focused GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_opaque_display_eq_v1`
      moves from `cargo_check_errors=73` to `71`; the previous
      `SimpleInterval` `PartialEq` and two `SimpleInterval: Display` residuals
      are gone. The remaining `Display` residual is `()`-typed and belongs to
      external return-shape modeling, not opaque facade support.
      `Hash` support extends the same rule for native collection membership:
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_opaque_hash_v1` moves
      from `cargo_check_errors=71` to `69`, and the previous
      `HashSet<VCFHeaderLine>::extend` method-not-found residuals disappear.
    Ordinary method arguments use the same ownership rule:
    - Ideal Rust Target: a Java method call should not consume a local binding
      that Java can still read later. If the solved Rust method argument slot
      is still owned and usage facts prove reuse, emit
      `self.method(value.clone())` rather than `self.method(value)`. If the
      method signature solver can prove a borrowed API, prefer that borrowed
      API instead of cloning.
    - Implementation Rule: shaped non-constructor call arguments now route
      through the same `adapt_reused_owned_argument` helper already used by
      constructor and map-insert argument lowering. The helper remains
      fact-driven: local binding, owned target slot, current callable use count
      greater than one, and clone-recoverable argument type.
    - Required Facts: solved method argument slot shape, inferred actual
      shape, current callable usage facts, local binding identity, and
      clone/borrrowed-view capability.
    - Owning Layer: `call_plan` owns method argument slot shapes; `usage`
      owns repeated binding facts; `lower` applies the existing mechanical
      adapter. This keeps the mapping independent of specific method names.
    - Fallback Shape: if reuse or clone recovery is not known, leave the
      ordinary argument and let later signature/capability diagnostics expose
      the missing fact.
    - Evidence: `MethodArgumentReuseOwnedBoundary.java` proves
      `self.keep(name.clone()/* TODO(translation): validate added clone */)`
      while later Java reads of `name` remain valid. The focused GenomicsDB
      gate for this slice reports `cargo_check_errors=105`, down from `106`,
      with zero tracked invalid patterns; the previous
      `borrow of moved value: sample_name` residual is gone in the serial
      reader path.
    Overloaded callable usage facts use the same ownership rule:
    - Ideal Rust Target: Java overloads should keep separate body-use facts so
      an owned Rust argument is cloned only when the specific overload body can
      still read that binding later. For a fluent builder call inside an
      overloaded helper, the ideal Rust is
      `builder.set_workspace(workspace.clone())` followed by a later
      `workspace.as_str()` read, while unrelated overloads keep their own
      usage facts and signature constraints.
    - Implementation Rule: `usage` records the existing Java-name method facts
      unchanged for legacy/signature consumers and additionally records
      Rust-disambiguated method facts in `methods_by_rust_name`. Lowering's
      reused-owned-argument adapter consults the Rust-name usage map when the
      current Java method may have overload-specific Rust names. Signature
      solving does not consume the Rust-name usage map, so this rule does not
      add duplicate method constraints.
    - Required Facts: Java method name, resolved or parser-assigned Rust
      method name, current callable type/name, binding-use facts for the
      overload body, target argument shape, and owned recovery capability.
    - Owning Layer: `usage` owns overload-specific body-use fact storage;
      `signature` continues to consume the existing Java-name usage facts;
      `lower` owns selecting the overload-specific use count for the mechanical
      clone adapter.
    - Fallback Shape: if no Rust-name usage facts are available, lower through
      the legacy Java-name use count. If reuse cannot be proven, leave the
      ordinary argument and let ownership diagnostics or the compile gate expose
      the missing fact.
    - Evidence: `usage_counts_static_call_arguments_after_fluent_builder_consumption`
      checks later static-call reads are recorded; `reused_local_recovery_uses_overload_specific_usage_facts`
      proves overloaded fluent-builder calls clone the reused binding. The
      focused GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_overload_usage_v3`
      moves from `cargo_check_errors=65` to `64`; the previous
      `borrow of moved value: workspace` residual is gone without adding new
      signature-method buckets.
    Optional unwraps that consume reused Java nullable bindings have their own
    ownership rule:
    - Ideal Rust Target: Java nullable values that have already been checked
      for non-null should flow into required Rust value slots without
      consuming the original `Option<T>` when Java reads it again later. For a
      clone-capable owned target, the ideal Rust is
      `value.as_ref().unwrap().clone()` rather than `value.unwrap()`. This
      preserves the nullable binding for later diagnostics such as
      `value.as_ref().map(...).unwrap_or_else(...)`.
    - Implementation Rule: before generic slot conversion applies the normal
      optional unwrap coercion, lowering checks whether the source expression
      is a local `Option<T>` binding, the target slot is owned non-optional
      `T`, the current callable usage facts show the binding is reused, and
      the element target has clone recovery capability. If so, lowering emits
      a borrowed unwrap plus clone and bypasses the consuming `unwrap()`
      coercion for that expression. This is an optional-shape ownership rule,
      independent of maps or any specific external type.
    - Required Facts: actual optional element type, target slot shape, current
      callable binding-use facts, target ownership, and clone capability.
    - Owning Layer: `conversion_plan` owns the default `Option<T>` to `T`
      unwrap shape; `usage` owns repeated-binding facts; `slot_shape` and
      effective type capabilities own clone eligibility; `lower` applies the
      source-aware borrowed unwrap because this decision depends on the
      expression identity and current callable usage.
    - Fallback Shape: if the binding is not reused, keep normal `unwrap()`.
      If clone capability is unavailable or the target is erased/opaque, keep
      the normal conversion and let structured ownership diagnostics or the
      compile gate expose the missing capability.
    - Evidence: add a neutral nullable object fixture where a checked
      `Option<URI>` is inserted into a map and then rendered in a duplicate
      diagnostic. The emitted Rust should use `as_ref().unwrap().clone()` for
      the insert value and still use the original optional binding later.
    Optional wraps that consume reused Java bindings have their own ownership
    rule:
    - Ideal Rust Target: passing a non-null Java value into a nullable
      `Option<T>` slot should not consume the source binding if Java reads it
      later. For clone-capable owned values, the ideal Rust is
      `Some(value.clone())` rather than `Some(value)`. For string-like
      borrowed optional APIs, a future modeled target may prefer
      `Some(value.as_str())` or `Some(&value)`, but owned optional slots should
      preserve the binding by cloning.
    - Implementation Rule: before generic slot conversion applies the normal
      optional wrap coercion, lowering checks whether the source expression is
      a local binding, the target slot is `Option<T>`, the actual source shape
      is owned non-optional `T`, usage facts show the binding is reused in the
      current callable, and clone recovery is available. If so, lowering emits
      `Some(value.clone()/* TODO... */)` and bypasses the consuming
      `Some(value)` coercion for that expression.
    - Required Facts: actual source shape, target optional element shape,
      current callable binding-use facts, ownership, dispatch, and clone
      capability.
    - Owning Layer: `conversion_plan` owns the default `T` to `Option<T>` wrap;
      `usage` owns repeated-binding facts; `slot_shape`/capabilities own clone
      eligibility; `lower` applies the expression-aware clone-before-wrap
      because it depends on source binding identity.
    - Fallback Shape: if the binding is not reused, keep normal `Some(value)`.
      If clone capability is unavailable or the target element is erased,
      retain the default conversion and expose the remaining ownership problem
      through diagnostics or compile gates.
    - Evidence: add a neutral fixture where a `String` argument is passed to a
      nullable validator and then used in an error message. The emitted Rust
      should call `validator(Some(name.clone()/* TODO... */))` and still render
      `name` later.
    Java platform path/URI/file facades have their own argument-surface rule:
    - Ideal Rust Target: Java APIs that accept path-like values should lower
      to a Rust facade boundary that accepts the same finite family of
      translated Java carriers: `String`, `&String`, `&str`, `Option<String>`,
      `&Option<String>`, `Path`, `URI`, and `File`. For constructors such as
      `new URI(path)` and helpers such as `IOUtils.getPath(path)`, the ideal
      Rust support surface is `URI::new<P: JavaPathArg>(path)` and
      `IOUtils::get_path<P: JavaPathArg>(path)`, not ad hoc lowerer rewrites
      at each call site. When the Java value is proven non-null but still
      represented as `Option<String>`, the support boundary can consume the
      option and recover the string value.
    - Implementation Rule: platform facade support owns Java path-like
      argument normalization through the `JavaPathArg` trait. Constructors and
      helpers that conceptually accept Java file/path/URI strings use that
      trait rather than plain `Into<String>`. Lowering may still apply reused
      binding ownership adapters before the call, but it does not need a
      per-constructor branch for `URI`, `Path`, or `File`.
    - Required Facts: the receiver/constructor is a modeled Java platform
      facade, the Rust support type is emitted, and the argument carrier is in
      the finite `JavaPathArg` family.
    - Owning Layer: `stdlib` owns the support trait and constructor/helper
      signatures; `call_plan` owns choosing the facade target; `lower` owns
      only generic ownership preservation before arguments reach that target.
    - Fallback Shape: if a value is not in the `JavaPathArg` family, keep the
      normal argument expression and let Rust type checking or a structured
      unsupported diagnostic expose the missing platform carrier.
    - Evidence: add a neutral fixture with a nullable `String` path checked
      for non-null and passed to `new URI(path)`. The generated support should
      accept `URI::new(path)` when `path` is represented as `Option<String>`
      without requiring a fixture-specific unwrap rewrite.
    A follow-up mutability slice records native collection field mutators in
    expression position as `self` mutations. This makes methods such as
    `values.put(k, v)` and `let previous = map.put(k, v)` emit `&mut self`
    when they mutate a field-backed Rust collection. Coverage includes
    `CollectionFieldMutator.java` and the usage assertion
    `usage_treats_native_collection_field_mutators_as_self_mutation`. In the
    focused GenomicsDB gate, `SampleNameMap.addSample` now emits `&mut self`
    and the aggregate count drops to `cargo_check_errors=299`. The remaining
    `GenomicsDBImport` caller still emits `.add_sample(...)` through
    independently translated modules; that is now a package-aware
    signature-sharing/call-target problem, not a callee mutability problem.

- [ ] `STREAM-2`: Route stream chains through a stream plan beyond the current
  capture-free expression-lambda slice.
  - Owner: `src/stream_lowering.rs`, `src/stdlib.rs`, `src/conversion_plan.rs`
  - Ideal Rust Target: Java stream pipelines should become ordinary Rust
    iterator pipelines whose item ownership is visible at every step. Borrowed
    collection sources should start from `.iter()`, owned/projection sources
    should use `.into_iter()` only when the projected value is owned, nullable
    element refinement should become `filter_map`, side-effecting `forEach`
    should become a `for` loop, and terminal collectors should become native
    `Vec`, `HashSet`, or `HashMap` collection boundaries. Callback bodies
    should look like normal Rust closures or loop bodies, with `Result`
    carriers only when the callback itself can throw.
  - Implementation Rule: `stream`/`stream_shape` owns pipeline planning from
    source shape, operation sequence, lambda body shape, and terminal
    collector. `stream_lowering` renders only planned iterator operations and
    closures. `call_plan` owns statement-position `forEach` and
    functional-interface callback arguments before generic call adaptation.
    Conversion planning owns borrowed/owned element recovery between iterator
    item shapes and terminal or callback slots. Lowering must not inspect a
    specific stream fixture, collector spelling, or GATK method name to repair
    ownership after rendering.
  - Required Facts: source collection/nullability shape, stream item shape
    after each operation, lambda parameter and capture slots, terminal
    collector kind, callback return/exception carrier, method-reference
    target arity, and external/source callable return shape for projected
    stream sources.
  - Owning Layer: resolver and `types` infer source/call shapes;
    `stream_shape` records item and terminal shapes; `stream` records the
    operation plan; `call_plan` records callback statement/call boundaries;
    `conversion_plan` records element recovery; `stream_lowering` and
    `call_lowering` render the selected Rust spelling.
  - Fallback Shape: if a source, operation, callback, or collector cannot be
    planned, emit a structured unsupported stream diagnostic at the stream
    boundary rather than a partially Java-shaped chain or a late
    `compile_error!` inside a closure.
  - Evidence: `map`, `filter`, `flatMap`, `forEach`, `collect`, `findFirst`,
    `anyMatch`, method references, and side-effecting closures are planned
    before rendering.
  - Fixtures: `MethodReferenceConstructor.java`,
    `MethodReferenceFlatMap.java`, `CollectionRemoveIfLambda.java`,
    `ForEachLambdaSideEffect.java`, `StreamCallbackChain.java`.
  - Status: Partial. Stream planning now recognizes
    `flatMap(Collection::stream)` as a dedicated stream op and
    `collect(Collectors.toSet())` as a collection-kind terminal, instead of
    leaving those spellings for generic call lowering. Java `Set<T>` now has a
    native `HashSet<T>` slot-rendering policy, and borrowed stream elements can
    be recovered through a `Set` borrowed-element carrier in the conversion
    layer. Java `Iterable.forEach` expression lambdas used as statements are
    now planned through `CallStatementPlan::IterableForEach` and lowered as
    Rust `for` loops with the lambda parameter inserted as a borrowed
    iteration-element shape, so side-effecting calls such as
    `names.forEach(name -> emit(name))` render as `for name in names.iter() {
    self.emit(name.clone()); }` instead of `.for_each(compile_error!(...))`.
    The same statement plan now accepts multi-parameter block lambdas such as
    `map.forEach((key, value) -> { ... })`, rendering a destructuring Rust
    `for (key, value) in map.iter()` loop and learning lambda-local
    declarations from their declared Java types when they are outside the
    method-level signature plan. Expression-position callback lambdas used as
    target-typed functional-interface arguments now render as Rust closures
    before argument adaptation, so `executor.submit(() -> { ... })` becomes
    `.submit(|| { ... })` rather than
    `.submit(compile_error!("unsupported Java expression: lambda"))` or a
    runtime `downcast::<Callable>()`.
    Generic method references used as call arguments now route through
    `method_reference_plan` and `method_reference_lowering`: constructor
    references render as function items such as `IllegalArgumentException::new`,
    likely static references as `Score::add`, and accessor-shaped unbound
    references as closures such as `|value| value._2()`. Stream planning no
    longer has a global capture-free veto; simple captureful expression lambdas
    are accepted when `stream_lowering` can render the actual lambda body, so
    `values.stream().mapToInt(v -> v + base).sum()` lowers to
    `values.iter().map(|v| (*v + base)).sum::<i32>()`. Stream `flatMap`
    also recognizes direct collection-stream lambdas such as
    `group -> group.stream()` as `FlatMapPlan::LambdaCollectionStream`, so the
    stream planner owns flattening shape and `stream_lowering` owns only the
    iterator spelling. `flatMap` lambdas now also support object-to-collection
    projection streams such as `group -> group.names().stream()`: the resolver
    target-types `flatMap` lambdas from the upstream `Stream<T>`, infers a
    flattened `Stream<U>` when the lambda body returns `Stream<U>`, and the
    stream plan records whether the nested source is a borrowed stream source
    (`.iter()`) or an owned projected value (`.into_iter()`). Stream lambdas
    now distinguish expression bodies from value-returning block bodies, so
    simple `.map(value -> { ...; return expr; })` closures render as Rust
    closure blocks with local declarations/statements before the final value.
    Stream source planning also recognizes nullable sequence carriers such as
    `Option<Vec<T>>` and lowers Java `source.stream()` as an explicit
    `source.as_ref().unwrap().iter()`, matching Java's null-pointer behavior
    instead of iterating over the `Option` carrier. `Collectors.toMap(k, v)` is
    now a terminal stream plan that computes a `Map<K,V>` actual shape before
    lowering and renders through `HashMap` collection. Stream filters now have
    a non-null refinement plan for `Objects::nonNull` and `x -> x != null`;
    when the input stream item is `Option<T>`, lowering emits `filter_map` and
    downstream terminal typing sees `T`. Nullable value-returning map blocks
    with early `return null` guards lower as `Option<T>` closures, so
    `.map(...).filter(Objects::nonNull).collect(...)` is handled by the stream
    planner instead of by ad hoc post-processing. Stream-lambda expression
    lowering also covers Java array `.length`, static constants, `String.split`
    in local block declarations, and `Integer.parseInt`/`Math.toIntExact`
    inside closures. `flatMap` block lambdas may now return a nested stream
    sub-pipeline such as `return names.stream().filter(...).map(...);`; the
    planner records the nested source, operations, source element, and output
    shape, while lowering renders the nested iterator chain inside the outer
    `flat_map` closure.
  - Ideal Rust Target: stream closure bodies should render bare names with the
    same Rust binding rules used by normal expression lowering. Lambda
    parameters and local declarations remain local variables; unshadowed
    instance fields become `self.field`; static fields on the current type
    become `Self::FIELD`; and type-qualified external static calls stay
    `Owner::method(...)`. For example, a Java stream closure that references
    `workspace`, `ARRAY_COLUMN_BOUNDS_START`, and
    `GenomicsDBUtils.getArrayColumnBounds(...)` should render as
    `self.workspace`, `Self::ARRAY_COLUMN_BOUNDS_START`, and
    `GenomicsDBUtils::get_array_column_bounds(...)`. An outer local such as
    `vidMapPB` must remain `vid_map_p_b`, not become `VidMapPB::...`.
  - Implementation Rule: `stream_lowering` now receives a `StreamRenderContext`
    backed by `ShapeEnv` from `lower_stream_plan`. Stream-lambda bare-name
    rendering delegates to the shared `access_lowering` field/static-field
    rule when a name is not shadowed by lambda locals. Static-owner detection
    checks lambda locals and `ShapeEnv` locals/fields before classifying a
    receiver as a type owner, so local and field bindings beat external
    static-owner inference.
  - Required Facts: method/type `ShapeEnv`, lambda parameter/local names,
    current-type static field names, instance field shapes, outer local shapes,
    and static-owner spelling from `call_plan`.
  - Owning Layer: `expr_shape` builds the field/local/static-field environment;
    `access_lowering` owns the shared bare-name Rust spelling;
    `stream_lowering` owns closure-body use of that environment; `call_plan`
    owns static-owner Rust type spelling. Lowering does not add stream-specific
    GATK name branches.
  - Fallback Shape: if a bare name is not a lambda local, method local, field,
    or static field, keep the conservative snake-case variable spelling or
    static-owner fallback already used by stream lowering; unresolved external
    facades remain separate dependency/fact gaps.
  - Evidence:
    `stream_lambda_emission_uses_shape_env_for_bare_fields_and_static_fields`
    covers instance fields, static fields, external static owner calls, nested
    static owners, and outer locals inside stream closures. The focused
    GenomicsDB gate moves from `cargo_check_errors=101` to `97`; the previous
    closure-local residuals for `workspace`, `coordinates`, and
    `ARRAY_COLUMN_BOUNDS_*` are gone.
  - Coverage: positive `documents_method_reference_flat_map_gap` regression
    and generated Rust behavior test for `MethodReferenceFlatMap.java`;
    positive output and generated Rust compile tests for
    `ForEachLambdaSideEffect.java`; positive method-reference adapter tests for
    `MethodReferenceConstructor.java`, `StreamCallbackChain.java`, and
    `StreamCapturesLocal.java`; positive generated Rust behavior coverage for
    `StreamFlatMapLambda.java`; positive generated Rust behavior coverage for
    `StreamBlockLambda.java`; positive translation coverage for
    `StreamFlatMapProjection.java`; positive static-owner stream block coverage
    in `StreamBlockStaticOwner.java`; positive `CollectorsToMap.java`,
    `StreamFilterNonNull.java`, `StreamMapNullFilter.java`, and
    `StreamFlatMapBlockPipeline.java` fixture coverage; positive block
    callback coverage in `MapForEachBlockLambda.java` and
    `ExecutorSubmitBlockLambda.java`; plus the design-level
    method-reference, call-statement, and stream
    planner/renderer boundaries, including optional sequence source unwrapping,
    owned string-result shape for `toLowerCase`/`toUpperCase`, non-null
    element refinement, nullable map/filter-map fusion, closure-local
    split/length/parse lowering, and nested `flatMap` stream sub-pipelines.
  - Residual: `flatMap` lambdas whose bodies are not stream-source expression
    lambdas, non-list/set collectors, mutation/escape-heavy capture bodies
    beyond local declarations/expressions/assignments plus final return,
    closure statements needing full method-lowering semantics, and full
    target-functional-interface arity/type adaptation for method references.
    In the GenomicsDB gate, block-lambda planning now removes placeholders and
    renders type-qualified builder calls such as `ContigPosition::new_builder`
    and `GenomicsDBColumn::new_builder`; remaining failures are missing
    external/co-translated builder types and broader call/slot-shape gaps. The
    latest focused GenomicsDB stream refresh removes the `Collectors.toMap`
    placeholder for `getContigsList().stream().collect(toMap(...))` and lowers
    `generateIntervalListFromWorkspace` to an iterator chain with
    `filter_map`. The follow-up flatMap refresh lowers
    `generateIntervalListFromVidMap` to a nested iterator chain and removes the
    remaining stream lambda placeholder. A later callback refresh lowers
    `futures.forEach((sampleName, future) -> { ... })` as a Rust loop, reducing
    unsupported lambda placeholders from `2` to `1`; the exposed nested
    try/catch body and the remaining `submit(() -> { ... })` callback still
    need callback-local exception/result planning. A later callback-expression
    refresh renders the `submit` block lambda as a Rust closure and removes the
    last unsupported lambda placeholder from the focused GenomicsDB gate; the
    only remaining explicit placeholder is the nested callback-local try/catch
    body, which needs exception/result planning with the lambda's return
    carrier rather than the enclosing method's return carrier. A follow-up
    callback-local exception refresh treats statement-only `forEach` callback
    bodies as unit-returning exception scopes, so nested callback try/catch
    renders `match (|| -> Result<(), JavaException> { ... })() { Ok(()) => {},
    ... }` instead of returning from the enclosing method. The focused
    GenomicsDB gate now reports zero tracked invalid-pattern buckets; remaining
    failures are real unresolved imports/external facades, field naming, and
    slot-shape/type mismatches.

- [x] `COERCE-1`: Remove typed downcast paths when concrete types are known,
  including external primitive and array getters.
  - Owner: `src/conversion_plan.rs`, `src/coercion.rs`, `src/call_lowering.rs`
  - Ideal Rust Target: when facts prove a value already has the concrete Rust
    shape required by its target slot, conversion should be identity or a
    simple primitive/owned-recovery adapter. External getters returning
    primitives, strings, byte arrays, or other concrete native values should
    not render `downcast::<T>()`; real runtime downcasts are reserved for
    dyn/erased object boundaries where the representation plan requires them.
    Owned native sequence storage flowing into a borrowed sequence API should
    become a slice view, for example `values.as_slice()`. Nullable sequence
    storage flowing into the same borrowed slice slot should borrow the option
    carrier first, for example
    `self.intervals.as_ref().expect("Java null receiver").as_slice()`, rather
    than moving the `Vec<T>` out of the owner.
  - Implementation Rule: conversion planning compares actual and target slot
    shapes after type inference, external facts, and representation projection.
    It selects no-op, primitive, optional, ownership, dyn, erased, or
    unsupported conversion variants before coercion renders anything.
    Call lowering passes expected slot information into this plan rather than
    inserting nominal Rust downcasts itself.
  - Required Facts: actual Java/Rust shape, target slot shape, external return
    type, primitive/array/string category, representation outcome, dispatch
    shape, optionality, sequence element type, requested slice view,
    owned-recovery capability, and erased/dyn boundary policy.
  - Owning Layer: `conversion_plan` owns shape comparison and conversion
    variant selection; `coercion` owns rendering; call/member lowering only
    supplies actual and target context.
  - Fallback Shape: unknown concrete evidence remains an opaque or unsupported
    conversion diagnostic; do not emit typed downcasts for primitive/native
    concrete targets.
  - Evidence: typed getter, nullable linked node, and external API typed return
    fixtures no longer emit `downcast::<T>().expect(...)`.
  - Fixtures: `TypedGetterAvoidsDowncast.java`,
    `PrimitiveGetterDispatchCoercion.java`,
    `BooleanGetterDispatchCoercion.java`, `ByteArrayGetterDispatchCoercion.java`.
  - Status: Complete for the listed fixtures. Dispatch coercion now treats
    same-typed or unknown external getter results flowing into native concrete
    primitive/array targets as having no dispatch conversion, because such
    targets have no nominal runtime downcast target. This preserves real
    dyn/boxed object downcasts for named targets while avoiding generated
    `compile_error!` or `downcast::<T>()` on typed primitive and byte-array
    getter results. Coverage includes direct dispatch policy tests plus
    positive idiom regressions for string, boolean, primitive int, and byte
    array getters.
    Sequence view coercion now treats owned native arrays/lists/sets with a
    matching element type as valid borrowed slice-view sources. It renders
    `.as_slice()` for owned storage and uses
    `.as_ref().expect("Java null receiver").as_slice()` when the source is an
    optional carrier flowing into a borrowed slice slot. Coverage includes
    `ArrayArgumentSliceView.java`,
    `view_coercion_policy_owns_sequence_slice_adapters`, and
    `optional_sequence_to_slice_uses_borrowed_unwrap_then_slice_view`. The
    focused GenomicsDB gate for this slice reports `cargo_check_errors=109`,
    down from `110`, with zero tracked invalid patterns; the previous
    `expected &[SimpleInterval], found Vec<SimpleInterval>` residual is gone.
    Owned values flowing into mutable-borrow target slots use the same
    ownership-coercion layer: `OwnershipCoercionPolicy::BorrowOwnedMutably`
    renders `&mut value`, including fallback translated-source calls whose
    unique method signature was recovered even when resolver facts were absent.
    Coverage includes `owned_to_mutable_borrow_is_an_ownership_coercion` and
    `FallbackStaticMutBorrowArgument.java`. The focused GenomicsDB gate now
    renders
    `GATKGenomicsDBUtils::update_import_protobuf_vid_mapping(&mut importer)?;`
    and removes the previous `expected &mut Box<dyn Any>, found Box<dyn Any>`
    residual.
    Reused owned call arguments now query the existing
    `OwnedRecoveryCapability` facts instead of a `String`-only predicate.
    Cloneable native collections such as `HashMap<String, i32>` therefore
    clone once at each owned call boundary when Java reuses the local later.
    Coverage includes `ReusedMapArgumentClone.java`; the GenomicsDB gate now
    clones `field_name_to_index_in_vid_fields_list` across the repeated update
    calls, removing the exposed 13-error moved-`HashMap` bucket.

- [ ] `NULL-2`: Make null checks consume slot optionality.
  - Owner: `src/semantic_analysis.rs`, `src/conversion_plan.rs`,
    `src/lower.rs`, `src/operator_lowering.rs`
  - Ideal Rust Target: Java nullable storage should be represented as
    `Option<T>` at the slot where null can actually occur, and Java null tests
    should consume that carrier with `is_none`, `is_some`, `as_ref`, `as_mut`,
    `take`, or a planned unwrap only at a branch where non-null has been
    proven. Java null-pointer behavior on nullable collection receivers should
    remain explicit through `as_ref().unwrap()` or `as_mut().unwrap()` rather
    than silently treating `None` as an empty collection.
  - Implementation Rule: nullability facts and signature/slot planning decide
    which locals, fields, params, returns, and expression carriers are
    optional. `nullability_plan` owns branch refinements and ternary/null-test
    refinements. `operator_lowering` owns null comparison spelling.
    `conversion_plan` owns `Option<T>` wrapping, unwrapping, clearing, and
    borrowed-element recovery. Call and collection plans must consume the
    solved optional receiver/value shape instead of adding Java-null special
    cases in generic rendering.
  - Required Facts: slot optionality, local/field assignment nullability,
    constructor initialization facts, branch refinement facts, collection
    element and receiver shape, external getter/factory nullability, and
    target-slot shape for unknown calls flowing into optional storage.
  - Owning Layer: `semantic_analysis` and `fact_saturation` produce finite
    nullability facts; `signature` projects them into slot shapes;
    `nullability_plan` plans scoped refinements; `operator_lowering` renders
    null predicates; `conversion_plan` renders optional adapters; `lower` only
    applies the plan in the current environment.
  - Fallback Shape: when nullability cannot be proved, choose `Option<T>` for
    public/unknown nullable boundaries or keep an opaque external boundary with
    a diagnostic. Do not assign raw `None` to a concrete slot, and do not
    unwrap nullable values outside a proven refinement or explicit Java
    null-pointer boundary.
  - Evidence: `x == null`, `x != null`, while-assignment null checks, ternary
    null branches, and nullable returns all type-check as `Option`.
  - Status: Partial. Plain literal-null equality and inequality now route
    through an operator-owned null-comparison policy and render as
    `value.is_none()` / `value.is_some()` instead of `value == None` or
    `value != None`. This consumes the already-solved `Option<T>` slot shape
    for ordinary expressions and nullable returns without introducing local
    null-special casing in generic binary rendering. `nullability_plan` now
    owns ternary branch refinements for `x == null ? ... : ...` and
    `x != null ? ... : ...`; lowering applies those facts only inside the
    proven branch, so `old == null ? 1 : old + 1` can render
    `old.unwrap() + 1` while `missing ? null : value` still renders through
    the generic target-slot `Option<T>` wrapping path. Assignment-in-condition
    while loops now route through the `CTRL-1` planned-loop path and use
    `is_none()` / `is_some()` checks. Native `Map.get` now advertises an
    `OptionalElementBorrow` lowered type, so assignments to nullable owned
    slots recover through the borrowed-element adapter path instead of
    wrapping `counts.get(key)` in a second `Some(...)`; boxed primitive map
    values use copy recovery and render `.copied()`. Required primitive slots
    fed by nullable borrowed boxed primitive map values now append the Java
    null-boundary unwrap after recovery, for example
    `indexes.get(&name).copied().expect("Java null receiver")`. Native
    collection calls now also own their argument slot shapes:
    `Map.get`/`containsKey` borrow keys, while `Map.put`, `List.add`, and
    `Set.add` request owned element slots derived from collection generics.
    Enhanced-for lowering inserts a
    borrowed iteration-element shape into the body environment, so a Java
    `for (String name : names) counts.put(name, value)` can render the Rust
    insertion boundary as `counts.insert(name.clone(), value)` through the
    generic ownership conversion path. External nullable read/getter calls now
    also have call-plan-owned lowered return carriers: a fact such as
    read-only getter + `maybe-null return=String` is planned as an
    `OptionalElementBorrow`, and `lower_expr_for_slot` feeds that through the
    same borrowed optional recovery path used by native collections. Bulk
    collection extension now separates receiver mutation from source
    borrowing: usage analysis records `addAll` argument 0 as content-read
    without reclassifying the receiver as read-only, and collection lowering
    adapts borrowed sequence sources through the existing borrowed-element
    recovery policy before rendering `extend`. Native collection call planning
    now also looks through optional storage carriers such as
    `Option<Vec<T>>`/`Option<HashSet<T>>` when deciding whether a Java `size`,
    `isEmpty`, or mutating collection call belongs to the native collection
    path; lowering uses `as_ref().unwrap()` for read calls and
    `as_mut().unwrap()` for mutating calls, preserving Java's null-pointer
    behavior while avoiding raw `.size()`/`.addAll()` spellings.
  - Coverage: `NullableParameterAndLocal.java`,
    `WhileAssignmentNotNull.java`, `WhileAssignmentNullVariants.java`,
    `CollectionsUse.java`, and the inline `TernaryNullBranchRefinement`
    regression. Design coverage asserts native collection argument slots are
    planned in `call_plan`, enhanced-for body element shapes are introduced by
    lowering before body conversion, nullable external getter return carriers
    are planned in `external_call_lowered_type`, and optional collection
    receivers are unwrapped at native collection call lowering boundaries.
    Translation coverage includes an inline nullable external getter
    assignment/null-check regression and `BulkAddAllBorrowed.java`, which emits
    `&mut Vec<String>` for the destination, `&[String]` for the source, and
    `values.extend(other.iter().cloned())`. Constructor field-nullability
    analysis now treats unshadowed bare assignments such as `field =
    new TreeMap<>()` as field writes while preserving parameter/local
    shadowing; constructor delegation cycles remain conservative. This removes
    `Option<HashMap<...>>` carriers from the GATK-shaped
    `SampleNameMap` pattern when all constructors initialize the map fields.
    Coverage includes bare constructor assignment, parameter-shadowing, and
    constructor-initialized `SortedMap` regressions in `tests/semantic_facts.rs`.
    Optional null-clearing assignments now have a conversion-plan-owned
    statement policy: assigning Java `null` into a solved `Option<T>` target
    lowers as `target.take();` instead of `target = None;`, preserving Java
    clear/discard semantics while avoiding the focused gate's concrete-None
    invalid-pattern signal. The focused gate now also excludes explicitly
    typed `let ...: Option<...> = None;` declarations from
    `concrete_none_assignment`, because those are valid optional
    initialization rather than concrete null assignment.
  - Residual: External nullability facts still need broader corpus-driven
    coverage for non-getter factories and nullable values that are produced
    without receiver borrowing. Bulk collection extension now removes raw
    `addAll` spelling in the GenomicsDB gate. Unresolved calls flowing into a
    shaped slot now use that target slot as fallback actual-shape evidence only
    when their inferred type is still unknown, so inherited/external calls such
    as `getDefaultToolVCFHeaderLines()` can feed `Set.addAll` without an erased
    `downcast::<Set>()`. Set-style and non-cloneable element recovery should be
    expanded from corpus evidence rather than guessed. The remaining GenomicsDB
    `None`/`Option` signals are now method/local return carriers and optional
    string formatting sites, not the constructor-proven map-field case.

- [x] `ARRAY-1`: Lower Java arrays with shape-aware constructors.
  - Owner: `src/array_lowering.rs`, `src/signature.rs`
  - Ideal Rust Target: Java array creation should lower to Rust `Vec<T>` or
    fixed/native array forms with explicit element shape: `vec![default; len]`
    for one-dimensional defaultable allocations, `vec![...]` for initializer
    literals, and `Vec<Option<T>>` or an unsupported diagnostic when Java
    null-initialized object arrays cannot safely become default Rust values.
  - Implementation Rule: array planning records element type, initializer
    elements, allocation dimensions, and defaultability before lowering.
    `array_lowering` renders only planned sized or initializer arrays, while
    signature/slot planning decides optional object element carriers and
    conversion planning adapts array arguments.
  - Required Facts: array element Java type, primitive/object category,
    initializer children, computed length expression, dimension count,
    default value availability, element nullability, and target slot shape.
  - Owning Layer: parser/Java IR preserve initializer versus sized creation;
    `array_plan` owns array-construction classification; `signature` owns
    array slot/element shape; `array_lowering` renders planned Rust
    allocation.
  - Fallback Shape: unsupported multidimensional or non-defaultable object
    arrays emit structured array diagnostics rather than placeholder empty
    vectors or raw Java array syntax.
  - Evidence: primitive/object arrays with literal and computed lengths emit
    valid `Vec`/array initialization instead of placeholder empty vectors.
  - Gate expectations: `PrimitiveArrayWithComputedLength.java` emits
    `vec![0; len]` or equivalent; `StringArrayLiteralArgument.java` emits a
    real `Vec<String>` allocation or literal; no "Java array creation"
    placeholder remains.
  - Implemented: `Expr::ArrayCreation` now preserves typed initializer
    elements; `array_plan` exposes typed initializer children separately from
    sized allocation dimensions; `array_lowering` emits `vec![default; len as
    usize]` for one-dimensional defaultable arrays and `vec![...]` for typed
    and untyped literals.
  - Residual: computed-length arrays of arbitrary named object types still need
    the nullability/slot-shape layer to decide between `Vec<Option<T>>`,
    default construction, or a deliberate unsupported fallback.

- [x] `CALL-1`: Lower static owner and common Java stdlib calls.
  - Owner: `src/stdlib.rs`, `src/call_plan.rs`, `src/call_lowering.rs`,
    `src/access_lowering.rs`
  - Ideal Rust Target: static constants and Java stdlib calls should look like
    the Rust construct they model, not like Java syntax with a cast adapter.
    `Owner.CONSTANT` should become `Owner::CONSTANT`; a known external string
    constant flowing into a `String` slot should not render
    `downcast::<String>()`; common string/math/equality calls should use Rust
    string methods, `format!`, numeric helpers, or equality operators. Rust
    string-pattern APIs such as `contains`, `starts_with`, and `ends_with`
    should receive borrowed `&str` pattern arguments, including when the Java
    receiver is stored as `Option<String>` and lowered through an explicit
    Java-null unwrap boundary.
  - Implementation Rule: call planning classifies known Java stdlib targets
    before fallback method rendering, including native Java string, math,
    formatting, class/object, and static-owner forms. Argument lowering then
    supplies target slots for the selected native call rather than relying on
    unshaped expression lowering: `String.format` display varargs request
    display-segment conversion, and string-pattern arguments request a borrowed
    string view slot. Lowering only applies these plans and renders the Rust
    method or macro spelling.
  - Required Facts: receiver expression type after optional-carrier lookup,
    resolved/static owner status, argument count, external field/call facts,
    target slot shape for each argument, and string-like content capability.
  - Owning Layer: `call_plan` owns target classification; `conversion_plan`
    owns argument slot conversions and borrowed string-view adapters;
    `call_lowering` owns Rust call spelling; `lower` only connects planned
    targets to planned argument slots.
  - Fallback Shape: unknown Java stdlib calls remain fallback receiver/free
    calls or structured diagnostics. Do not introduce one-off receiver/argument
    rewrites in rendering to make a single corpus call compile.
  - Evidence: `Owner.CONSTANT` becomes associated/module syntax,
    `String.format` becomes `format!`, Java string methods become Rust string
    methods, and `Objects.equals`/`.equals` become Rust equality or an
    option-aware helper.
  - Fixtures: `StaticConstantOwner.java`, `StringFormatStaticCall.java`,
    `JavaStringMethods.java`, `EqualsAndObjects.java`,
    `NestedStaticBuilderCall.java`, `StreamBlockStaticOwner.java`.
  - Implemented: call planning now has native Java instance/static call targets
    for string length/lowercase/equality, `String.format`, `Objects.equals`,
    and `Math.max`, with rendering centralized in `src/call_lowering.rs`.
    Member-access planning now owns static constant receiver syntax, so
    constant-like `Owner.CONSTANT` lowers as `Owner::CONSTANT` instead of an
    instance field on a lowercased receiver. Call planning also has explicit
    static-owner targets for resolved type-qualified static calls and a narrow
    unresolved owner-chain fallback for qualified static-looking receivers;
    stream block-lambda lowering carries lambda/block locals so it can use the
    same owner spelling without treating value receivers as type owners.
    Java string receiver return-type inference now preserves `String` through
    unresolved chains such as `value.toString().toLowerCase()`, so later
    string-pattern calls still use the borrowed `&str` argument target instead
    of falling back to an owned `String` pattern.
    Java `String.format` now has two target shapes: literal format templates
    still render as Rust `format!`, while dynamic Java format templates render
    through the shared `java_string_format(template, Vec<String>)` helper.
    This keeps Rust's literal-only macro requirement explicit and prevents
    invalid fallback output such as `format(format!(...), arg)`.
    `DynamicStringFormatTemplate.java` covers runtime Java format templates and
    has generated-crate behavior coverage for `%d` replacement. The GenomicsDB
    focused gate with Tribble and GenomicsDB packs moves from
    `cargo_check_errors=119` to `118`; the old
    `logger::warn(format(format!(...)))` residual is replaced by
    `logger::warn(java_string_format(...))`.

  - Chained string receiver rule:
    - Ideal Rust Target: Java string chains that produce a string should stay
      typed as Rust string values across the chain. A hand-written Rust target
      for `path.toString().toLowerCase().endsWith(suffix)` is
      `path.to_string().to_lowercase().ends_with(suffix.as_str())`: the
      receiver chain owns the intermediate lowercase `String`, while the
      pattern argument is a borrowed `&str` view and is not moved. Java string
      methods whose Rust spelling returns a borrowed view, such as `trim()`,
      should stay borrowed until the target slot asks for ownership:
      `let value: String = raw.trim().to_string()` and
      `slot = Some(raw.trim().to_string())`.
    - Implementation Rule: Java string receiver methods with known return
      types are exposed as stdlib return-type facts before call planning.
      `call_plan` then classifies the final `contains`/`startsWith`/`endsWith`
      call from the inferred receiver type and argument lowering uses the
      string-pattern borrowed slot. Expression shape inference additionally
      projects known borrowed-view methods such as `trim()` as shared
      `&str`-view shapes, so normal slot conversion owns the later
      `to_string()` recovery. Rendering only emits the planned Rust string
      method spelling.
    - Required Facts: receiver effective type after optional-carrier lookup,
      Java string method name and arity, argument slot shape for Rust pattern
      APIs, and string-like content capability for the pattern expression.
    - Owning Layer: `stdlib` owns known Java string receiver return types;
      `types` consumes those facts during expression type inference;
      `call_plan` owns native string call classification; `lower` owns
      applying the planned borrowed string slot to arguments.
    - Fallback Shape: if the receiver type is unknown, keep fallback method
      rendering or emit a structured diagnostic. Do not rewrite `endsWith`
      arguments by rendered text after call planning has failed.
    - Evidence: `StringPatternChainedReceiver.java` proves a chained
      `toString().toLowerCase().endsWith(...)` receiver still borrows the
      pattern argument as `suffix.as_str()`. `StringTrimOwnedSlot.java` proves
      `trim()` lowers as a borrowed view and is converted to owned `String`
      only at owned or optional owned slots. The focused GenomicsDB gate at
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_string_trim_owned_view`
      moves from `cargo_check_errors=113` to `111`, with zero invalid-pattern
      files; the `SampleNameMap` `trim()` assignments now render
      `.trim().to_string()`.

  - Java string split rule:
    - Ideal Rust Target: Java `String.split(regex)` and
      `String.split(regex, limit)` should not be rendered as raw Rust
      `str::split` when Java regex or limit semantics are in play. The
      hand-written Rust target is a small compatibility helper such as
      `java_string_split(line.as_str(), "\\t", -1) -> Vec<String>`, so the
      receiver is borrowed, the regex delimiter is explicit, and the result is
      owned string parts suitable for Java `String[]`/`List<String>` slots.
    - Implementation Rule: Java string receiver return/type facts classify
      `split` as a native Java instance call with array-of-string result.
      `call_lowering` renders the planned call through `java_string_split`
      instead of Rust `str::split`, defaulting the one-argument Java overload
      to limit `0`. Support-item scanning emits the helper only for programs
      that use the Java split API. The focused package script mirrors that
      helper into shared crate support when it strips per-module support
      prelude items.
    - Required Facts: receiver effective type after optional-carrier lookup,
      method name and arity, regex argument expression, optional limit
      argument, result slot shape, and support-item usage.
    - Owning Layer: `stdlib` owns Java string split return type and support
      helper; `call_plan` owns native split classification; `call_lowering`
      owns helper-call rendering; `lower` owns support-item inclusion; the
      focused package script owns crate-level shared support extraction.
    - Fallback Shape: unsupported arities emit an explicit compile diagnostic.
      Unknown receivers stay on fallback call rendering rather than applying
      a rendered-text `.split(...)` rewrite.
    - Evidence: `StringSplitWithLimit.java` proves
      `line.split("\\t", -1)` renders as `java_string_split(...)` and no
      longer emits a two-argument Rust `str::split` call.

  - Nullable path overload rule:
    - Ideal Rust Target: Java overloads that pass `null` into a path-like
      parameter and then guard that parameter with `param != null` should expose
      the Rust parameter as `Option<Path>`. The overload wrapper should call the
      wider overload with `None`, not `Some(None)`. Java `Path.resolveSibling`
      should return a direct `Path`, so a fallback expression can be wrapped at
      the nullable boundary as `Some(path.resolve_sibling(...))` only when the
      surrounding slot is optional.
    - Implementation Rule: source nullability collection walks variable
      declaration initializers as expressions, so null comparisons inside
      ternary initializers contribute parameter nullability constraints.
      Lowering treats a Java null literal flowing into an optional slot as
      `None` before generic optional wrapping can run. Branch-local non-null
      refinements are suppressed when the branch target is still an optional
      slot, preserving the optional carrier across ternary arms. Platform
      facade support models `Path.resolve_sibling` as returning `Path` and
      accepts `Option<Path>` at path-like external boundaries.
    - Required Facts: source parameter null comparison, target slot optionality,
      branch refinement facts, platform facade return shape, and JavaPathArg
      boundary capability.
    - Owning Layer: `signature` owns source nullability evidence from
      declaration initializers; `lower` owns null literal optional-slot
      rendering and branch slot preservation; `stdlib` owns platform facade
      return and boundary shapes; the focused package script mirrors shared
      facade support.
    - Fallback Shape: if local expression nullability is not fully proved,
      retain `Option<Path>` at the boundary and rely on JavaPathArg support as
      the structured fallback rather than emitting invalid `Path.is_some()` or
      `Some(None)`.
    - Evidence: `NullablePathOverloadArgument.java` proves a wrapper passes
      `None`, the wider overload parameter is `Option<Path>`, and
      `resolve_sibling` is a direct `Path` value.
    External static constant member types now flow through
    `types::infer_member_access_type`, using the same stdlib/external field
    facts consumed by resolver and call planning. That lets static constant
    access project a concrete shape before conversion planning, so known
    `GATKVCFConstants.*_KEY` string constants avoid erased
    `downcast::<String>()` adapters independent of the eventual lowering slot.
    String pattern calls are also planned as native Java instance calls:
    `contains`, `startsWith`, and `endsWith` render through Rust
    `contains`, `starts_with`, and `ends_with`, while their pattern arguments
    flow through the existing borrowed string-view slot conversion (`AsStr`).
    Optional string receivers are classified before receiver adaptation, so
    `Option<String>` fields still use the same target and then unwrap through
    the normal Java-null boundary.
    - Ideal Rust Target: imported Java static utility owners should render as
      Rust type/facade calls using the imported simple owner, not as snake-case
      receiver modules. A Java call through `import external.lib.Utility;`
      should become `Utility::read_value(arg)`. If the utility class is not
      translated or mapped to a real Rust crate yet, the generated fallback
      should provide a conservative facade and an alias from the imported
      simple owner to the fully qualified facade type:
      `pub type Utility = ExternalLibUtility;`. Local variables or fields with
      the same spelling must still win over the imported type owner.
    - Implementation Rule: `TypeEnv` now records imported static type aliases
      seeded from Java imports into `ShapeEnv` before method, constructor, and
      stream lowering. `call_plan` promotes only unshadowed alias names to
      `StaticOwner`, preserving local/field precedence. `stdlib` scans used
      imported static-owner calls and emits opaque facade support plus simple
      owner aliases for non-local owners; value-producing fallback methods use
      an inferred `R: Default` return, while obvious side-effect methods such
      as `use_*`, `set_*`, and `assert_*` return `()`.
    - Required Facts: parsed imports, parser-normalized owner aliases, current
      method/constructor locals and fields, local translated type names, used
      static-owner call arities, and external/support ownership boundaries.
    - Owning Layer: parser owns import facts; `expr_shape`/`TypeEnv` own
      imported type-owner aliases and shadowing; `call_plan` owns static-owner
      target selection; `stdlib` owns conservative facade and alias support;
      lowering only renders the planned `Owner::method(...)` target.
    - Fallback Shape: unknown imported external utilities receive an opaque
      support facade with generic return inference where the surrounding Rust
      slot can supply the target type. Static imports and fully qualified
      owner expressions that do not pass through an imported simple alias
      remain separate CALL-1 residuals.
    - Evidence:
      `call_plan_recovers_normalized_static_utility_owner_receivers` covers
      imported aliases and shadowing; `ImportedStaticUtilityOwner.java` covers
      emitted `ExternalUtility::read_value(...)`, local receiver preservation,
      and the simple-owner type alias. The focused GenomicsDB gate moves from
      `cargo_check_errors=97` to `80`, removing the lower-case unresolved
      module buckets for imported utility owners such as `genomics_d_b_utils`,
      `bucket_utils`, `interval_utils`, `abstract_feature_reader`,
      `v_c_f_utils`, `v_c_f_file_reader`, and `executors`.
    - Ideal Rust Target: explicit static member imports and package-qualified
      utility calls should render through the same static-owner facade shape.
      `import static external.lib.Utility.readValue; readValue(path)` and
      `external.lib.Utility.readValue(path)` should both become
      `Utility::read_value(path)`. Ordinary receiver-less helper calls with no
      static import should remain free/self fallback calls; package-qualified
      nested model types such as protobuf builders should not be reclassified
      as generic utility facades.
    - Implementation Rule: `TypeEnv` records explicit static member imports
      separately from imported type aliases. `call_plan` promotes only
      receiver-less calls named by that map to `StaticOwner`. `stdlib` scans
      explicit static member imports and package-qualified static-owner calls
      whose root is a lowercase package and whose owner has exactly one
      class-like segment, then emits conservative facade support. Existing
      simple opaque support types are used directly instead of creating
      aliases that could cycle with nested type aliases.
    - Required Facts: explicit static imports, parser-normalized member names,
      receiver absence, package-qualified owner expression segments, local
      support type inventory, and used call arity.
    - Owning Layer: parser owns static import facts; `expr_shape`/`TypeEnv`
      owns static-member import lookup; `call_plan` owns receiver-less
      static-owner promotion; `stdlib` owns support facades and alias-cycle
      avoidance.
    - Fallback Shape: wildcard static imports remain fallback free calls until
      resolver/source-summary facts identify the owner. Nested type-qualified
      builder/model calls stay with nested-type planning rather than the
      package-qualified utility fallback.
    - Evidence:
      `StaticImportUtilityOwner.java` covers explicit static member imports,
      package-qualified utility calls, and ordinary local free-call fallback.
      The GenomicsDB static-import probe removes the unresolved
      `GenomicsDBUtils`, `read_entire_file`, and `is_genomics_d_b_array`
      residuals, but the focused gate is not yet a compile-count win
      (`cargo_check_errors=80` to `82`) because the now-planned calls expose
      static-facade argument ownership and return-slot inference gaps.
    - Ideal Rust Target: modeled external static utility calls should use the
      same argument slot facts as instance external calls. If a static utility
      API declares a read-only string/path argument, emitted Rust should pass a
      shared string view such as `workspace.as_str()` rather than consuming the
      local `String`, independent of whether Java wrote the call through an
      imported type owner, an explicit static member import, or a
      package-qualified owner expression.
    - Implementation Rule: `call_plan` derives a Java/canonical external owner
      key for static-owner argument planning from `TypeEnv` imported type
      aliases, explicit static member imports, and package-qualified owner
      expressions. It then reuses `ExternalApiArgumentEffectFact` to produce
      shaped argument slots. The GenomicsDB external pack now owns
      `org.genomicsdb.GenomicsDBUtils` utility argument facts for
      `isGenomicsDBArray`, `readEntireFile`, and `useGcsHdfsConnector`; lowering
      continues to consume only `ArgumentLoweringPlan`.
    - Required Facts: static owner source, Java/canonical receiver key,
      external argument effect facts, argument parameter type/nullability,
      alias policy, and receiver/member shadowing facts.
    - Owning Layer: `external_packs` owns GenomicsDB utility API facts;
      `TypeEnv` owns imported static owner/member facts; `call_plan` owns
      owner-key recovery and argument slot planning; lowering owns only slot
      conversion/rendering.
    - Fallback Shape: static utility calls without argument-effect facts remain
      unshaped conservative facade calls. They may compile via generic
      fallback support, but they must not add renderer-local ownership guesses.
    - Evidence:
      `static_external_owner_argument_shapes_use_external_effect_facts` covers
      imported type-owner aliases, explicit static member imports, and
      package-qualified owners. `static_import_lowering_borrows_arguments_from_external_effect_facts`
      verifies emitted `StaticUtility::read_value(path.as_str())`.
    - Ideal Rust Target: modeled external static utility facades should expose
      fact-owned return types through the Rust support method, not a generic
      `R: Default` fallback. A Java static utility predicate such as
      `GenomicsDBUtils.isGenomicsDBArray(...)` should emit a support method
      returning `bool`; a modeled file-reading utility should return `String`;
      a modeled side-effect utility should return `()`.
    - Implementation Rule: `stdlib` collects imported static-owner calls before
      support generation, skips duplicate instance fallback support for those
      calls, and asks `FactSet::external_api_effect_for_receiver_with_arity`
      for the static owner and arity. `FactSet` receiver matching treats
      fully-qualified external facts and imported simple owner names as aliases,
      so one `org.genomicsdb.GenomicsDBUtils` fact can serve calls written
      through `GenomicsDBUtils`, static member imports, or package-qualified
      owners. Non-conservative side-effect/mutator facts without a return type
      lower as `()`, while unknown utilities keep the generic fallback.
    - Required Facts: external API effect fact, receiver/simple-owner alias,
      used static-owner call arity, return type/nullability, role/provenance,
      local type inventory, and imported static owner/member facts.
    - Owning Layer: `external_packs` owns concrete external API effects;
      `facts` owns receiver/method/arity alias lookup; `stdlib` owns support
      facade method emission and duplicate instance fallback suppression;
      lowering only renders the planned static call.
    - Fallback Shape: if no modeled API effect exists for the owner/method/arity,
      support generation keeps the existing conservative
      `pub fn method<R: Default, ...>(...) -> R` fallback for value-like
      utilities, or `()` for obvious side-effect naming. The fallback is
      explicit support-surface behavior, not a lowering-time type guess.
    - Evidence:
      `external_api_effect_receiver_lookup_matches_simple_owner_for_fqn_facts`
      proves fact-layer FQN/simple-owner matching.
      `static_import_support_reuses_fqn_return_facts_for_simple_owner_facade`
      proves emitted static-import support uses the FQN fact to render
      `is_genomics_d_b_array(...)-> bool` and borrowed string arguments.
      `static_import_support_uses_external_return_facts_without_instance_duplicate`
      proves a typed static utility return suppresses generic and instance
      duplicate support. With `--builtin-external-pack com.intel.genomicsdb`,
      the focused GenomicsDB gate now emits:
      `is_genomics_d_b_array<T0, T1>(...) -> bool`,
      `read_entire_file<T0>(...) -> String`, and
      `use_gcs_hdfs_connector<T0>(...) -> ()`; the gate is
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_static_return_effects_pack_v1`
      with `cargo_check_errors=78`.
      `genomicsdb_pack_declares_constructors_and_nested_builder_qualifiers`
      verifies `GenomicsDBUtils` argument facts are pack-owned. The focused
      GenomicsDB gate at
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_static_arg_effects_v3`
      renders `GenomicsDBUtils::is_genomics_d_b_array(workspace.as_str(), ...)`
      and `GenomicsDBUtils::read_entire_file(vidmap_json.as_str())`, removing
      the previous moved-`workspace` shape at that call site. The current dirty
      worktree gate still reports `cargo_check_errors=89` and
      `invalid_java_cast_downcast=1`; remaining top buckets are static facade
      return/support inference, missing constructor/default surfaces, and
      external collection/header models.
    Coverage includes positive idiom-gap regressions and design tests for
    formatter conversion, equality, static utility calls, nested static builder
    calls, stream block static-owner calls, static constant member access, and
    borrowed string-pattern arguments.
    Focused package translation now accepts `--context-dir`, so sibling source
    signatures and field facts shape per-file lowering without giving up the
    one-module-per-Java-file generated crate layout. Static final literal
    fields now emit Rust associated constants instead of instance storage:
    `public static final String NAME = "x"` becomes
    `impl Owner { pub const NAME: &'static str = "x"; }`. Parser field naming
    preserves Java constant spelling for static-final constant-style fields.
    Source static string constants now stay out of instance-field owned
    recovery:
    - Ideal Rust Target: a Java source `static final String NAME = "..."`
      declared on the translated type should behave like the emitted Rust
      associated constant. Borrowed contexts should render `Self::NAME`.
      Owned `String` slots, such as `Vec<String>::push`, should render
      `Self::NAME.to_string()`. The output should not use
      `Self::NAME.clone()`, because the Rust constant is `&'static str`, not
      `String`.
    - Implementation Rule: `expr_shape`/`ShapeEnv` keep source static string
      fields as static field facts with borrowed string view shape.
      `lower`'s instance-field clone recovery excludes
      `env.is_static_field(name)`, so static constants fall through to the
      normal `conversion_plan` path. The existing `ViewShape::Str` to owned
      native `String` conversion then selects `ToOwnedString`.
    - Required Facts: static field identity, Java `String` field type, emitted
      borrowed Rust string view shape, current target slot shape, and local
      shadowing facts.
    - Owning Layer: `expr_shape`/`ShapeEnv` own source static-field identity
      and string view shape; `conversion_plan` owns borrowed-to-owned string
      recovery; `lower` only prevents instance-field clone recovery from
      claiming static fields.
    - Fallback Shape: non-string static fields keep their normal static-field
      shape. Unknown static fields remain conservative rather than inventing a
      string adapter.
    - Evidence:
      `source_static_string_constant_push_to_vec_string_uses_to_string_adapter`
      covers the source-declared associated-constant path, and
      `static_string_constant_argument_to_owned_string_slot_uses_to_string_adapter`
      keeps the external/member-access path covered. The focused GenomicsDB
      gate at
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_static_source_string_v1`
      renders
      `missing.push(Self::BYPASS_FEATURE_READER.to_string())` and
      `missing.push(Self::VCF_HEADER_OVERRIDE.to_string())`; the former
      `.clone()` mismatches are gone. Total `cargo_check_errors` reports `65`
      versus the prior `64` because the compiler now reaches a separate erased
      object/reference assignment residual (`E0605`), not because the static
      string rule regressed.
    Member-access planning now owns proved instance field spelling:
    - Ideal Rust Target: Java source or modeled dependency instance fields
      should use the Rust field name chosen for the receiver type. A Java
      access like `other.batchCount`, `this.totalBatchCount`, or
      `arg.batchCount` should render as `other.batch_count`,
      `self.total_batch_count`, or `arg.batch_count` only when facts prove the
      member is an instance field. Unresolved field-like syntax should remain a
      fallback field access rather than being globally rewritten by spelling.
    - Implementation Rule: `TypeEnv` records source type-field facts keyed by
      receiver type and field alias. `member_access_plan` classifies proved
      instance fields as `MemberAccessOp::RustField { rust_name }`, using the
      same `to_snake_case` naming policy as struct field emission. Current
      instance fields use current-field facts; receiver values use type-field
      facts or external-pack public field facts. `access_lowering` only renders
      the planned field name.
    - Required Facts: receiver type or current-instance receiver, source
      field declaration or external field fact, field type, and the Rust field
      naming policy.
    - Owning Layer: `expr_shape`/`TypeEnv` own source field facts and aliases;
      `types` owns field type inference; `member_access_plan` owns instance
      field classification and Rust field spelling; `access_lowering` renders
      the planned operation.
    - Fallback Shape: unresolved field accesses preserve fallback field syntax
      instead of guessing a snake-cased field. If a modeled external support
      type has no generated Rust fields yet, the access now fails as a missing
      field on that support type, which is an external facade/data-model gap
      rather than a member spelling gap.
    - Evidence: `member_access_plan_snake_cases_proved_instance_fields`,
      `access_lowering_owns_bare_field_and_member_access_fallbacks`, and
      `source_instance_field_access_uses_rust_field_names` pass.
      `cargo check --quiet` passes. The focused GenomicsDB gate with
      `htsjdk.tribble.feature` and `com.intel.genomicsdb` packs moves from
      `cargo_check_errors=178` to `174` at
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_instance_field_names`.
      The generated `GenomicsDBImport` code no longer contains
      `self.batchCount`, `self.inputPreloadExecutorService`,
      `arg.batchCount`, or `arg.totalBatchCount`; remaining
      `BatchCompletionCallbackFunctionArgument` errors are now missing
      generated fields on an external support struct.
    External support structs now consume those same field facts:
    - Ideal Rust Target: a modeled dependency type with proved public field
      facts should not be emitted as an empty unit struct when source code
      reads those fields. The support surface should be a small data facade,
      for example
      `pub struct BatchCompletionCallbackFunctionArgument { pub batch_count: i32, pub total_batch_count: i32 }`,
      using the same Rust field names that member-access planning emits.
    - Implementation Rule: `stdlib` external facade support records external
      field facts for already declared or otherwise used opaque external
      receiver types. When rendering an opaque support type, it emits a struct
      body with public fields instead of a unit struct. Field names use
      `to_snake_case`, and field types use the same external facade Rust type
      projection as external method returns. Method generation remains tied to
      used calls; field-only facades can still be emitted from declared
      external types plus field facts.
    - Required Facts: declared/used external receiver type, external field
      fact, field type, local type exclusion, Rust identifier validity, and
      external facade type projection.
    - Owning Layer: `stdlib` owns opaque external support item generation;
      external packs own field facts; `member_access_plan` owns field access
      spelling; lowering only inserts support items and renders planned field
      accesses.
    - Fallback Shape: if no field type is known, keep the unit opaque support
      type and let field access report a missing field or structured external
      boundary. Do not infer fields from call-site spelling alone.
    - Evidence: `external_support_struct_fields_are_generated_from_field_facts`
      passes alongside the instance-field access tests. `cargo check --quiet`
      passes. The focused GenomicsDB gate with `htsjdk.tribble.feature` and
      `com.intel.genomicsdb` packs moves from `cargo_check_errors=174` to
      `167` at
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_external_support_fields`.
      `BatchCompletionCallbackFunctionArgument` now emits public
      `batch_count` and `total_batch_count` fields, and that missing-field
      bucket is gone.
    Co-translated sibling source types now suppress same-named external
    facades in consuming files:
    - Ideal Rust Target: when a focused crate contains a translated Java
      source type, other translated files should refer to that source type
      through the crate's normal re-export/import surface. A Java field such as
      `SampleNameMap sampleNameMap` in one file should lower to
      `Option<SampleNameMap>` using the real translated `SampleNameMap` type,
      not to a local opaque `pub struct SampleNameMap;` facade that shadows the
      sibling type and loses constructors or methods.
    - Implementation Rule: support-item generation receives the full
      semantic/source context as its local type set while still scanning only
      the current file for declared types and used external calls. Any type
      present in that context is excluded from external opaque facade and
      external static-constant support emission for the current file. The
      focused-crate assembly script can then import the uniquely emitted
      sibling type from the crate root.
    - Required Facts: merged context/source program, canonical local type keys,
      current-file declarations and call arities, and generated-crate unique
      type re-export/import information.
    - Owning Layer: `lib` passes the semantic context program to lowering;
      `lower` uses that context only for support-item local type filtering;
      `stdlib` owns opaque facade/static-constant support generation and local
      type exclusion; generated-crate assembly owns cross-module imports.
    - Fallback Shape: without context, translation preserves the previous
      conservative behavior and may emit an opaque facade for unknown named
      types. With context, sibling source types must not be shadowed by support
      facades; if the sibling type is not uniquely re-exported, the compile
      gate should expose an import/crate assembly issue rather than a facade
      method-stub issue.
    - Evidence:
      `context_translation_does_not_emit_opaque_facade_for_sibling_source_type`,
      `context_translation_uses_sibling_overload_signature_for_call_target`,
      and `context_translation_uses_sibling_static_string_constant_shape` pass.
      `cargo check --quiet` passes. The focused GenomicsDB gate with
      `htsjdk.tribble.feature` and `com.intel.genomicsdb` packs moves from
      `cargo_check_errors=167` to `146` at
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_sibling_source_facade`.
      Generated `00004_GenomicsDBImport.rs` now imports `SampleNameMap` from
      the crate root, generated `src/lib.rs` re-exports
      `generated_00006_SampleNameMap::SampleNameMap`, and
      `00004_GenomicsDBImport.rs` no longer contains
      `pub struct SampleNameMap;`. The previous missing
      `SampleNameMap::{new,new_path_boolean}` and accessor-method bucket is
      removed; remaining errors are dominated by general mismatched-type,
      unresolved external module, constructor support, and optional/path
      coercion buckets.
    and final reference fields with non-null initializers produce non-null
    semantic field facts even when the class has constructors.
  - Focused Gate: the GenomicsDB gate with the optional display/string-pattern
    slices reports `cargo_check_errors=327`, zero tracked invalid-pattern
    buckets, and no remaining `String: Pattern` top-error bucket. This is down
    from `330` after optional display alone and `341` after exception display.
  - Focused Gate Update: the GenomicsDB package-context/static-constant slice
    reports `invalid_pattern_files=0` and removes the previous
    `GenomicsDBConstants::*` missing-associated-item errors. The generated
    constants module now contains associated `&'static str` constants and no
    instance fields. A follow-up slice shapes static string constants as
    borrowed string views, so the existing conversion planner emits
    `.to_string()` when an owned `String` slot is known. The focused gate drops
    from `cargo_check_errors=307` to `304`; the three
    `GenomicsDBConstants::DEFAULT_*` path arguments now render with
    `.to_string()` and no optional unwrap/downcast adapter.
  - Focused Gate Update: unresolved external setter/fluent-mutator calls now
    use an argument-aware call-plan entry point. Explicit external argument
    facts and resolved source signatures still win; the conservative fallback
    applies only to unresolved external setter-like roles and derives each
    target slot from the argument Java type or source-context static field
    shape. This makes
    `set_array_name(GenomicsDBConstants::DEFAULT_ARRAY_NAME)` render as
    `set_array_name(GenomicsDBConstants::DEFAULT_ARRAY_NAME.to_string())`
    without adding a GenomicsDB-specific lowering rule. The focused gate stays
    at `cargo_check_errors=304`, so the targeted residual moved but total
    errors are now dominated by missing boundary/source-summary symbols such as
    `logger`, `Builder`, `GATKVCFConstants`, `i_o_utils`, and unresolved domain
    types.
  - Focused Gate Update: same-type source `static final` fields now carry
    static-field metadata in `ShapeEnv`, including parser-normalized aliases
    back to the emitted associated-constant name. Bare references from instance
    and static methods lower as `Self::CONST`, not `self.const`, not a raw
    snake-case local, and string constants keep the borrowed static-string
    shape so owned `String` slots still use `.to_string()`. This keeps the
    rule in field/name access planning instead of adding a GATK-specific
    constant rewrite. The GenomicsDB focused gate drops from
    `cargo_check_errors=304` to `260`, with invalid-pattern buckets still at
    zero.
  - Focused Gate Update: normalized static utility receivers such as parsed
    `i_o_utils.getPath(...)` are now recovered as static-owner calls when no
    local or field shadows the receiver and external/stdlib facts know the
    owner. The ideal Rust target is an opaque platform facade call such as
    `IOUtils::get_path(path_like)` over a small `Path`/`URI`/`File` support
    surface, with borrowed string-like arguments accepted at the facade
    boundary. Enhanced Java `for` loops over optional sequence values now
    unwrap the optional collection before `.iter()`, preserving Java null
    receiver behavior with `expect("Java null receiver")` while iterating the
    sequence elements rather than the `Option` wrapper. The implementation
    lives in call planning, platform facade support, and enhanced-for iterable
    lowering; rendering only prints the selected call/loop shape. The
    GenomicsDB focused gate drops from `cargo_check_errors=260` to `244`, with
    invalid-pattern buckets still at zero and no remaining `i_o_utils`,
    duplicate `IOUtils`, missing `JavaPathArg`, or `&Vec<String>: JavaPathArg`
    residual bucket.
  - Focused Gate Update: inherited or external Java logging calls now target a
    generated logging facade module rather than an unresolved external crate.
    The ideal Rust target for currently unsupported logging boundaries is
    `logger::info(message)`, `logger::warn(message)`, `logger::debug(message)`,
    `logger::error(message)`, or `logger::trace(message)` over a tiny no-op
    facade that accepts any `Display` message. This preserves the call
    side-effect boundary without adding a logging dependency or encoding GATK
    class names. The implementation keeps the existing static-owner call plan,
    adds logger-call detection to platform-facade support emission, and hoists
    the support module in the focused package harness. The GenomicsDB focused
    gate drops from `cargo_check_errors=244` to `223`; the former top
    `logger` missing-module bucket is gone, and invalid-pattern buckets remain
    zero.
  - Focused Gate Update: Java `String.join(delimiter, values)` is now a native
    Java static call target rather than fallback associated-function syntax.
    The ideal Rust target is `values.join(delimiter)` for iterable/string
    collection inputs, with string literal delimiters rendered as borrowed
    `&str` literals. This keeps the rule in call planning and call lowering,
    next to `String.format`, instead of adding a rendered `String::join`
    rewrite. The GenomicsDB focused gate drops from `cargo_check_errors=220`
    to `219`, and the previous missing associated item
    `String::join(" and ", missing)` residual now renders as
    `missing.join(" and ")`.
  - Design Cleanup: fallback receiver method names selected from solved
    signatures are now applied through `call_plan` via a dedicated helper
    instead of pattern-matching and rebuilding `CallTargetPlan::FallbackReceiver`
    in `lower`. The ideal Rust target is unchanged; this keeps the
    responsibility boundary honest so lowering consumes planned call targets
    rather than rewriting their semantic name choice. The focused GenomicsDB
    gate remains at `cargo_check_errors=219`, and the previously failing
    `call_plan_owns_call_and_new_target_selection_before_argument_lowering`
    design test passes again.
  - Focused Gate Update: external static string constants now get a generated
    facade when the translated source subset references a typed external field
    owner that is not itself translated.
    - Ideal Rust Target: a Java external `public static final String`
      constant should read as ordinary Rust associated constant access:
      `Owner::NAME` when a borrowed string view is enough, and
      `Owner::NAME.to_string()` when the target slot requires owned `String`.
      The generated support surface should be a small namespace
      `pub struct Owner; impl Owner { pub const NAME: &'static str = "..."; }`.
      If the literal value is not available from co-translation or
      source-summary facts, the facade may use the member name as a compile
      boundary placeholder until literal constant facts are modeled.
    - Implementation Rule: `stdlib` scans actual program expressions for
      type-qualified field accesses whose external field facts or built-in
      external model prove `JavaType::String`, then emits only the referenced
      owner/field facade constants for owners not present in the translated
      program. `member_access_plan` gives static string constants a borrowed
      `&str` Rust shape while preserving Java type `String`, so normal slot
      conversion, not call-site rewriting, inserts `.to_string()` for owned
      `String` parameters.
    - Required Facts: unshadowed static/type receiver, external/static field
      type fact or built-in external field type, local translated type names,
      and the target argument/return/assignment slot shape.
    - Owning Layer: `stdlib` owns external constant facade emission and
      built-in external field typing; `member_access_plan` owns the static
      constant Rust shape; conversion planning owns borrowed-to-owned string
      recovery; lowering only inserts support items and renders planned
      expressions.
    - Fallback Shape: if a referenced external field has no known string type,
      no typed facade is emitted; the value remains unknown/opaque and should
      be handled by the existing unsupported or erased-boundary paths. If the
      field value is unknown but the type is known, the compile-boundary
      placeholder is explicit and should be superseded by source-summary
      literal facts when available.
    - Evidence: `external_static_string_constant_support_is_generated_from_field_facts`
      covers a neutral injected external field fact;
      `type_inference_owns_known_external_static_string_constants` asserts
      static string constants project as borrowed `&str` shapes; and
      `translates_external_string_constant_argument_without_downcast` covers
      the GATK-style fixture. The focused GenomicsDB gate moves from
      `cargo_check_errors=219` to `201`; the unresolved `GATKVCFConstants`
      owner bucket is gone, the constant arguments render with `.to_string()`
      in `String` slots, and invalid-pattern buckets remain zero.
  - Design Slice: nested Java type identity is now preserved through parsing
    and projected to valid Rust owner-specific type names instead of collapsing
    all nested classes to their final segment.
    - Ideal Rust Target: a Java dependency type such as
      `GenomicsDBVidMapProto.VidMappingPB.Builder` should not become a single
      ambiguous Rust `Builder`. It should become a distinct Rust boundary type
      such as `GenomicsDBVidMapProtoVidMappingPBBuilder` until a crate mapping,
      co-translated source type, or more idiomatic facade chooses a better
      public name.
    - Implementation Rule: the parser keeps the nested Java type suffix from
      the first uppercase segment, stripping only lower-case package prefixes.
      Slot rendering maps dotted Java type names to deterministic UpperCamel
      Rust identifiers by concatenating their nested segments. This is a type
      projection rule, not a generated-code rewrite.
    - Required Facts: parsed Java type spelling, package-vs-nested-class
      boundary, target slot shape, and any external pack/source-summary facts
      that later choose a crate/facade override.
    - Owning Layer: `parser` owns preserving the Java nested type identity;
      `slot_render` owns the default Rust type-name projection; external pack
      or source-summary facts may later override the boundary with a more
      idiomatic crate or facade type.
    - Fallback Shape: if no external facade is generated, the output may still
      fail with missing owner-specific Rust types. That is preferable to
      collapsing unrelated builders into one `Builder`, because the residual
      now names the exact boundary type that needs a support surface.
    - Evidence: `slot_render_owns_conservative_java_to_rust_type_projection`
      now asserts that
      `com.intel.genomicsdb.model.GenomicsDBVidMapProto.VidMappingPB.Builder`
      parses as `GenomicsDBVidMapProto.VidMappingPB.Builder` and projects to
      `VidMappingPBBuilder`/owner-specific Rust names. The focused GenomicsDB
      gate with the original tribble-only envelope is effectively flat
      (`cargo_check_errors=201` to `202`), and the pack-enabled probe is
      effectively flat (`183` to `184`), but the residual is now explicit:
      missing `GenomicsDBVidMapProtoVidMappingPBBuilder`,
      `CoordinatesGenomicsDBColumnBuilder`, and similar facade types instead
      of one ambiguous `Builder` bucket.
  - Residual: remaining common-call failures are no longer static string
    constants flowing into owned string slots or same-type static constants
    referenced as locals, normalized IOUtils receivers, optional sequence
    enhanced-for iterables, or missing logger facade modules. The next
    high-leverage owner is dependency/source-summary boundary modeling for
    missing external/source symbols and generated facade surfaces, plus the
    remaining general string-format rendering gap shown by
    `logger::warn(format(format!(...)))`, not additional static-constant,
    setter-argument, IOUtils, or logger module lowering.
  - External Facade Support Slice:
    - Ideal Rust Target: when a Java dependency boundary has explicit external
      API facts but no Rust crate mapping yet, generated Rust should expose a
      finite opaque facade rather than missing owner-specific types. Nested
      Java builder paths such as `Owner.Product.Builder` should compile as a
      named Rust alias to a concrete opaque builder target, and builder-like
      calls should stay fluent:
      `Product::new_builder().set_name(value).build()`. The target is a
      conservative compile boundary, not the final idiomatic crate mapping.
    - Implementation Rule: `stdlib` collects method names/arities actually
      used by the program, matches them against external API effect facts, and
      emits opaque structs, nested-type aliases from external field facts, and
      no-op facade methods only for nonlocal external types. Alias spellings do
      not receive their own inherent impls; methods live on the concrete opaque
      target so Rust type aliases do not create duplicate definitions. Known
      platform/JDK support names such as `Path`, `URI`, `File`, `IOUtils`,
      `List`, `Map`, and `HashMap` are excluded from this generic facade
      generator because they are owned by existing platform/stdlib rules.
    - Required Facts: used call name and arity, external API effect facts,
      external nested field facts, local type names, declared Java slot types,
      and the shared Java-named-type-to-Rust-name projection policy.
    - Owning Layer: `stdlib` owns generated opaque external facade support;
      `slot_render` owns Java named-type projection; `lower` only inserts the
      generated support item before source items. External packs or crate
      mappings may later replace the facade with a more idiomatic boundary.
    - Fallback Shape: if no fact or declaration exposes the type, the output
      still reports a missing type or missing method. If a type is a known
      JDK/platform carrier, it is left to the dedicated platform/stdlib
      support instead of being redefined by the generic facade.
    - Evidence:
      `external_nested_builder_facade_support_is_generated_from_facts_and_used_calls`
      covers a neutral external nested builder boundary. Focused external tests
      pass with `cargo test external_ --test slot_shape_design --quiet`. The
      pack-enabled GenomicsDB compile gate moves from the prior recorded
      `cargo_check_errors=184` to `181` at
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_external_api_facades_v3`,
      with invalid-pattern counts still zero.
      GenomicsDB protobuf `VidMappingPB.getContigsList()` is now modeled as
      `Vec<GenomicsDBVidMapProtoChromosome>` rather than an erased or
      misnamed element list, with typed `get_name`, `get_length`, and
      `get_tiledb_column_offset` facade methods generated from external API
      facts. `genomicsdb_pack_declares_constructors_and_nested_builder_qualifiers`
      covers the pack facts. The focused gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_chromosome_getters_v1`
      moves from `cargo_check_errors=69` to `68`; the previous missing
      `Contig` getter residuals are gone, and the remaining map residual is an
      erased-object propagation problem (`Box<dyn Any>`), not missing external
      getter support.
  - Java Object Boundary Slice:
    - Ideal Rust Target: a Java `Object` value slot should not render as a
      missing Rust type named `Object`. When no more precise source trait,
      enum, or crate/facade type is proved, the correct conservative Rust
      boundary is `Box<dyn std::any::Any>`. Method signatures such as
      `Object value()` should therefore emit
      `fn value(...) -> Box<dyn std::any::Any>`.
    - Implementation Rule: `slot_render` maps the named Java type `Object` to
      the erased-object Rust carrier through `NativeJavaRustTypePolicy::Direct`.
      External facade return rendering uses the same boundary for explicit
      external API facts returning `Object`. This keeps the rule at type
      projection/conversion boundaries rather than generating a fake `Object`
      struct in support code.
    - Required Facts: declared Java type spelling, object representation facts
      when present, and the absence of a more specific representation override.
    - Owning Layer: `slot_render` owns Java type projection; `stdlib` owns the
      matching external facade fallback return spelling; object representation
      and conversion planning may still select a more precise trait, enum, or
      concrete source type when facts prove one.
    - Fallback Shape: if a value is explicitly an erased Java object, use
      `Box<dyn std::any::Any>`. Do not emit a support `Object` type, because
      that hides the erased runtime boundary and conflicts with the existing
      object-representation matrix.
    - Evidence:
      `java_object_return_type_lowers_to_erased_any_boundary` covers emitted
      method signatures; the existing type-projection test now asserts
      `Object -> Box<dyn std::any::Any>`. The pack-enabled GenomicsDB gate
      moves from `cargo_check_errors=181` to `178` at
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_object_erased_boundary`,
      and the previous `cannot find type Object` bucket is gone.
  - Erased Object Facade And Nullable External Return Slice:
    - Ideal Rust Target: erased Java object boundaries should remain finite
      Rust boundaries while preserving the operations proved by external API
      facts. A nullable external return such as
      `GenomicsDBImporter.getProtobufVidMapping()` should lower to an optional
      erased or facade-backed slot, use `is_none()`/`as_ref().unwrap()` only on
      the `Option`, and then expose fact-owned methods such as
      `get_contigs_list`, `get_fields_count`, and `to_builder` on the
      unwrapped value. `new Object()` should render as
      `Box::new(()) as Box<dyn std::any::Any>`, never as invalid
      `Box<dyn std::any::Any>::new(...)`.
    - Implementation Rule: constructor lowering treats an erased `Object`
      constructor target as a boxed unit or boxed tuple/value expression.
      `stdlib` emits an extension-trait facade for `Box<dyn std::any::Any>`
      from used external instance API facts that are stronger than
      conservative fallback facts. Explicit return facts become typed facade
      returns; modeled void/mutator/side-effect facts become `-> ()` facade
      methods, so erased handles expose fact-owned operations without forcing
      concrete Rust types. GenomicsDB external pack facts model
      `getProtobufVidMapping` as `MaybeNull`, because the source checks it
      against `null`. External pack lookup and `FactSet` lookup normalize
      receiver aliases and method-name spellings so parsed snake-case calls
      and Java pack facts share one fact boundary.
    - Required Facts: constructor target slot shape, used call name/arity,
      external API effect return type/nullability, external receiver aliases,
      local source null checks, and fact provenance strong enough to expose an
      erased-object method.
    - Owning Layer: `call_lowering` owns erased constructor expression
      rendering; `stdlib` owns generated erased-object facade support;
      `external_packs` owns GenomicsDB API nullability and receiver/method
      lookup aliases; `facts` owns normalized external API fact lookup; and
      `semantic_analysis` owns collecting modeled external pack APIs even when
      resolver attached a candidate.
    - Fallback Shape: if an external method conflicts with another method of
      the same name/arity but a different return, do not add it to the
      erased-object facade. Optional erased receivers are represented as
      `Option<Box<dyn Any>>`; calls on the inner object unwrap at the option
      boundary before applying the erased facade. If a required erased facade
      method is unavailable, keep the opaque object and report the missing
      method or structured unsupported boundary instead of pretending the
      erased value has a concrete Rust type.
    - Evidence:
      `java_object_return_type_lowers_to_erased_any_boundary`,
      `external_nested_builder_facade_support_is_generated_from_facts_and_used_calls`,
      `signature_plan_marks_null_checked_reference_locals_optional`, and
      `semantic_analysis_uses_genomicsdb_pack_for_importer_and_protobuf_boundaries`
      pass. The focused GenomicsDB gate at
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_nullable_erased_facade`
      has zero invalid-pattern files and removes invalid
      `Box<dyn std::any::Any>::new` constructor syntax, but regresses the
      compile count to `cargo_check_errors=206`. The top residual is now
      optional-erased adaptation: generated code can still produce
      `let vid_map_p_b: Box<dyn std::any::Any> = importer.get_protobuf_vid_mapping();`
      before calling `.is_none()`/`.unwrap()` on it.
      `stdlib` now also emits erased-object facade methods for modeled void
      methods such as mutators and side-effect calls. Coverage includes
      `erased_object_facade_includes_modeled_void_methods`. The focused
      GenomicsDB gate renders `update_protobuf_vid_mapping`, `execute_import`,
      and `close` on erased-object facades; combined with owned-reuse clone
      recovery, it moves the latest focused gate to `cargo_check_errors=101`.
  - Optional Erased Slot Adapter Slice:
    - Ideal Rust Target: when a concrete or unknown Java expression flows into
      a nullable erased `Object` slot, the Rust value should first adapt the
      element to the erased carrier and only then wrap it in `Option`. The
      target shape is
      `Option<Box<dyn std::any::Any>> = Some(Box::new(value) as Box<dyn std::any::Any>)`,
      not
      `Some(value)`, not `Some(Box::new(Box::new(value) as Box<dyn Any>) as
      Box<dyn Any>)`, and not a plain `Box<dyn Any>` followed by
      `is_none()`/`unwrap()`.
    - Implementation Rule: optional shape coercion now plans wrap/unwrap as a
      structural coercion whose nested element plan owns dispatch, view, and
      ownership adaptation. `apply_shape_coercion_plan` applies that tree
      directly instead of flattening it into independent operations. Slot
      fallback for unresolved unknown calls uses the optional element shape
      rather than copying the whole optional target, and erased-object
      boundaries keep concrete unknown actual dispatch so erasure remains an
      explicit adapter. Named Java `Object` projects to the same erased-object
      representation as unknown object, and slot type rendering peels optional
      element shapes before dispatch rendering so optional erased slots render
      as `Option<Box<dyn std::any::Any>>`.
    - Required Facts: target slot shape, optional element type, object
      representation/dispatch projection, actual expression shape when known,
      and external/source nullability facts that decide whether the slot is
      optional.
    - Owning Layer: `conversion_plan` owns structural optional element
      planning and slot fallback; `coercion` owns structural application of
      nested optional plans; `object_model` and `shape_projection` own named
      `Object` erased representation; `slot_render` owns optional element type
      rendering. Lowering only asks for slot conversion.
    - Fallback Shape: unresolved unknown calls may borrow the target element
      carrier as a fallback, but they do not inherit the whole optional target
      as their actual shape. Missing element methods remain facade/source
      boundary errors after the option and erased carrier are represented
      correctly.
    - Evidence:
      `optional_shape_coercion_wraps_after_erased_element_conversion`,
      `optional_shape_coercion_policy_records_structural_element_coercions`,
      `nullable_object_slot_wraps_concrete_return_after_erasure_once`,
      `signature_baseline_marks_declared_object_as_erased_object_boundary`,
      `object_return_from_typed_external_getter_erases_once_at_boundary`,
      `typed_feature_reader_header_cast_uses_external_pack_return_fact`,
      `shape_projection_projects_unknown_java_types_as_erased_without_plan`,
      `java_object_return_type_lowers_to_erased_any_boundary`,
      `external_nested_builder_facade_support_is_generated_from_facts_and_used_calls`,
      `signature_plan_marks_null_checked_reference_locals_optional`, and
      `semantic_analysis_uses_genomicsdb_pack_for_importer_and_protobuf_boundaries`
      pass. `cargo check --quiet` passes. The implementation now also keeps
      declared Java `Object` signature baselines erased when no stronger
      object-representation fact exists, lets typed external pack rows win over
      later semantic-only rows for expression typing, renders nullable external
      facade returns as `Option<T>`, and routes optional external returns into
      erased targets through structural shape coercion instead of borrowed
      element recovery. The focused GenomicsDB gate with
      `htsjdk.tribble.feature` and `com.intel.genomicsdb` packs reports
      `cargo_check_errors=116` at
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_optional_erased_external_return`,
      with zero invalid-pattern files. The generated anonymous/wrapper
      `get_header` method now emits
      `Box::new(self.reader.get_header().unwrap()) as Box<dyn std::any::Any>`
      instead of returning a nullable typed external value directly, and
      `getHeaderFromPath` keeps the typed `VCFHeader` return without an invalid
      downcast.
      A later generic nullable-return slice keeps explicit external nullable
      getters as owned `Option<T>` carriers rather than borrowed optional
      projections and suppresses identity optional element maps. Coverage
      includes
      `external_nullable_getter_return_shape_flows_as_owned_optional_carrier`,
      `external_nullable_getter_assignment_uses_option_carrier_recovery`, and
      `concrete_external_facade_strategy_preserves_typed_nullable_returns`.
      `cargo check` passes. The focused GenomicsDB gate at
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_nullable_return_generic_v1`
      remains at `cargo_check_errors=68`, matching the previous
      chromosome-getter baseline without regressing it.
      Constructor-assigned locals now keep produced object storage owned even
      when later uses are borrowed:
      - Ideal Rust Target: a Java local declared before assignment and filled
        by a constructor call, such as
        `GenomicsDBImporter importer; importer = new GenomicsDBImporter(config);`,
        should store the produced value in an owned Rust local. Later Java
        calls that only borrow that local should adapt at the use site:
        `let mut importer: Box<dyn std::any::Any>;`,
        `importer = Box::new(value) as Box<dyn std::any::Any>;`,
        `use_readonly(&importer)`, or `use_mutating(&mut importer)`. The
        target is not `let mut importer: &Box<dyn Any>` and not
        `&Box::new(value) as Box<dyn Any>`, because that borrows a temporary
        while trying to satisfy an owned erased target.
      - Implementation Rule: `signature` records constructor-assigned local
        storage constraints from usage facts. If a nonprimitive binding has an
        `AssignedFromCall { name: "new", .. }` use, local signature solving
        adds `RequiredOwnership(Owned)` for the local slot. External argument
        and receiver facts may still request borrowed or mutable borrowed
        forms, but those are rendered by call-site conversion from the owned
        local instead of changing the stored local shape.
      - Required Facts: local declaration type, assignment use classified as
        constructor-produced storage, local mutation/binding facts, external
        argument or receiver alias/mutability facts, and object/erased dispatch
        projection.
      - Owning Layer: `usage` owns classifying constructor-assigned locals;
        `signature` owns the owned storage constraint and conflict merge;
        `conversion_plan`/`coercion` own borrow or mutable-borrow adaptation at
        later use sites; lowering only consumes the solved local shape and slot
        conversions.
      - Fallback Shape: locals not assigned from constructors keep their
        existing constraints, so borrowed string views, copy locals, and
        nullable return carriers are not forced into owned storage by this
        rule. More general call-result storage can be added later only with
        explicit source/escape facts.
      - Evidence:
        `constructor_assigned_local_keeps_owned_storage_despite_borrowed_external_uses`
        covers a neutral constructor-assigned local passed to a borrowed
        external argument. `cargo check` passes. The focused GenomicsDB gate at
        `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_constructor_storage_v1`
        reports `cargo_check_errors=62`; the previous `E0605`
        `&Box<Box<dyn Any>> as Box<dyn Any>` residual is gone, and generated
        `GenomicsDBImport` now emits
        `let mut importer: Box<dyn std::any::Any>;` followed by use-site
        `&mut importer` and `&importer` adapters.
      Enabling concrete
      GenomicsDB facades for `GenomicsDBImporter`, `VidMappingPB`, `Chromosome`,
      and `FieldInfo` exposed a separate cross-module facade identity problem:
      each translated Java file emits its own local fallback facade type, so
      concrete returns from sibling generated modules become distinct Rust
      types. The broad concrete-facade attempt regressed to
      `cargo_check_errors=81`, and a narrowed attempt still regressed to `74`;
      those object-strategy changes were not kept.
    - Typed External Facade Identity Follow-up:
      - Ideal Rust Target: fact-modeled concrete external values should have
        one Rust identity across the generated crate. If two translated Java
        files mention `GenomicsDBVidMapProto.VidMappingPB`, both should refer
        to the same facade type, and source/static helper returns should be
        assignable to local variables expecting that type without converting
        through `Box<dyn Any>` or creating a second module-local
        `VidMappingPB`.
      - Implementation Rule: external facade support must distinguish source
        types from external fallback facades during crate assembly. Concrete
        external facade declarations, aliases, and support methods should be
        emitted once at the crate/support boundary or through a dedicated
        generated support module, and per-file lowering should import that
        shared boundary. Only after shared identity is available should
        external pack object-strategy facts promote GenomicsDB data facades
        from erased object boundaries to concrete structs.
      - Required Facts: external-library ownership, local source type keys,
        object-strategy facts, external API return facts, alias/nested-type
        facts, and the generated crate/module boundary.
      - Owning Layer: `stdlib` owns facade support item generation and method
        surfaces; crate assembly owns where shared support items live and how
        generated modules import them; `external_packs` owns whether a type is
        eligible for concrete facade representation; lowering only consumes
        the selected slot shape and emitted support boundary.
      - Fallback Shape: until shared facade identity exists, GenomicsDB pack
        facades stay erased for focused multi-file gates. Concrete external
        strategy may still be tested in single-program/unit contexts, but it
        must not be enabled for cross-module package output where it creates
        distinct Rust nominal types.
      - Evidence: the failed concrete experiments are recorded at
        `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_concrete_facade_nullable_v1`
        (`cargo_check_errors=81`) and
        `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_concrete_facade_nullable_narrow_v1`
        (`cargo_check_errors=74`). The retained safe state is
        `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_nullable_return_generic_v1`
        (`cargo_check_errors=68`).

- [ ] `CALL-2`: Make `equals` and string/object comparisons idiomatic.
  - Owner: `src/operator_lowering.rs`, `src/stdlib.rs`
  - Ideal Rust Target: Java equality should render as Rust equality only when
    Rust equality is semantically available for the operands. Primitive,
    string, enum, and proven `PartialEq` values should use `==`/`!=`; nullable
    values should compare through references such as `left.as_ref() ==
    right.as_ref()` or explicit `Some(&value)` shapes; source-defined
    `equals` methods should remain method calls when they carry custom
    semantics; and `Objects.equals` should become a null-safe helper or match
    only when direct `PartialEq` is not the right model.
  - Implementation Rule: call planning identifies builtin Java equality forms
    and resolved source `.equals` overloads before lowering. Operand shape,
    nullability, representation, and method-effect facts are passed to
    `operator_lowering`, which chooses direct comparison, optional comparison,
    source method dispatch, or a structured unsupported equality boundary.
    Generic method-call lowering must not decide object equality by method name
    after overload/source resolution has already classified the call.
  - Required Facts: operand slot shapes, optionality, primitive/string/enum
    classification, source method resolution, whether a type has a custom
    `equals`, representation strategy, and external equality metadata when
    available.
  - Owning Layer: `resolve` owns source-vs-builtin call classification;
    `stdlib`/`call_plan` recognize Java equality APIs; `semantic_analysis`
    records custom equality facts; `operator_lowering` owns comparison
    spelling; conversion planning owns any borrowed/owned adapter needed before
    a planned comparison.
  - Fallback Shape: when equality semantics are not proven, preserve a source
    method call if resolved, use an explicit null-safe custom-equality helper
    if facts prove one exists, or emit a structured unsupported equality
    diagnostic rather than inventing `PartialEq`.
  - Evidence: `equals`, `Objects.equals`, primitive equality, string equality,
    optional equality, and enum equality use Rust comparison or explicit helper
    plans.
  - Status: Partial. Java equality calls now route nullable-aware comparison
    spelling through `operator_lowering`: matching non-optional operands render
    direct `==`, matching `Option<T>` operands render `left.as_ref() ==
    right.as_ref()`, and mixed nullable/non-null operands render explicit
    `Some(&value)` comparisons. `lower.rs` only identifies Java
    `equals`/`Objects.equals` call forms and passes operand shapes to the
    operator policy, keeping optional comparison spelling out of generic call
    rendering. Plain binary `==` / `!=` now also consume the same operand-shape
    policy before generic binary rendering, so enum comparisons use the parsed
    enum `PartialEq` derive and optional/non-optional comparisons use the same
    reference-aware spelling as `.equals`. Resolved source `.equals(...)`
    overloads now bypass the native Java instance equality fallback and lower
    as ordinary method calls, so custom equality semantics are not replaced
    with Rust `==` unless the call is an unresolved/builtin equality form.
    `Objects.equals` over two nullable operands of the same source type now
    consults method-effect facts; when that type declares `equals`, equality
    lowering emits one null-safe `match` that calls the source method for the
    `Some/Some` case instead of requiring a generated `PartialEq` impl.
    Optional string-view comparisons normalize the optional side through
    `.as_deref()` when the other operand is a borrowed string view such as a
    `trim()` result. The ideal Rust for
    `sampleName.trim().equals(sampleName)` after a null guard is
    `Some(sample_name.as_ref().unwrap().trim()) == sample_name.as_deref()`,
    not `Some(&...) == sample_name.as_ref()`, because the latter compares
    `Option<&&str>` to `Option<&String>`. The implementation is selected from
    operand shapes in `operator_lowering`; it does not special-case `trim` or
    GATK.
  - Coverage: existing `EqualsAndObjects.java` still renders direct Rust
    equality; new `NullableEqualsComparison.java` covers `Objects.equals` for
    two nullable strings and `.equals` where only the argument is nullable.
    `EnumEqualityComparison.java` covers binary `==` and `!=` on parsed Java
    enums and is included in the generated-crate compile gate.
    `CustomEqualsMethod.java` covers a resolved source `.equals(Value)` method
    and generated Rust compile behavior. `NullableCustomEqualsMethod.java`
    covers nullable `Objects.equals(left, right)` where the nullable element
    type has a source-defined `equals` method. Design coverage asserts the
    equality planner and renderer live in `operator_lowering`, and call-plan
    coverage asserts resolved source receiver calls take precedence over
    native Java instance fallbacks for `.equals`.
    `NullableTrimEqualsOriginal.java` covers optional string values compared
    with borrowed string-view expressions; design coverage asserts
    `RightOptionalStringView` and `LeftOptionalStringView` render through
    `.as_deref()`. The focused GenomicsDB gate for this slice reports
    `cargo_check_errors=104`, down from `105`, with zero tracked invalid
    patterns; the previous `expected Option<&&str>, found Option<&String>`
    residual is gone.
    including `!=` for optional comparisons.
  - Residual: custom object equality helpers still need representation-aware
    policies when direct `PartialEq` is not available. Current nullable custom
    equality coverage assumes owned `Option<T>` operands and an owned
    source-lowered `equals(T)` argument; borrowed option carriers, external
    equality metadata, overload disambiguation, and adapter insertion for
    borrowed/custom parameter shapes remain open.

### 9. Control Flow, Exceptions, And Constructor Initialization

- [ ] `CTRL-1`: Lower assignment-in-condition through a planned loop form.
  - Owner: `src/loop_lowering.rs`, `src/semantic_analysis.rs`,
    `src/lower.rs`
  - Ideal Rust Target: Java loops such as `while ((x = next()) != null)` should
    become explicit Rust `loop` forms that perform the assignment once at the
    top, test the solved optional carrier with `is_none`/`is_some`, break on
    the Java exit condition, and run the body with branch-refined knowledge of
    `x`. If the loop condition proves a value non-null after exit, later code
    should receive a planned non-null conversion instead of carrying
    unnecessary `Option<T>` pressure.
  - Implementation Rule: `loop_plan` owns recognition of assignment-in-null
    tests, negated variants, post-loop refinements, and unsupported compound
    condition shapes. Semantic analysis/signature planning marks assigned
    condition locals mutable and optional. `loop_lowering` renders the planned
    loop skeleton and applies branch/post-loop refinements; expression lowering
    must not render raw Java assignment expressions inside boolean contexts.
  - Required Facts: assigned local symbol, assignment expression shape, null
    test polarity, optional target slot, mutation requirement, body refinement
    scope, post-loop non-null refinement, and break/continue/label effects.
  - Owning Layer: `loop_plan` owns control-flow classification;
    `semantic_analysis` owns mutation/refinement facts; `signature` owns local
    slot shape; `loop_lowering` renders the Rust loop; `conversion_plan` owns
    optional-to-non-null recovery after proven exits.
  - Fallback Shape: unsupported compound or labelled variants should remain
    structured control-flow diagnostics, not raw `(x = next())` expressions or
    placeholder Rust embedded in a condition.
  - Evidence: `while ((x = next()) != null)`, assignment with negation, and
    nested condition variants emit valid Rust with correct scope and
    optionality.
  - Status: Partial. `loop_plan` now recognizes assignment-in-null-test while
    conditions as a dedicated `WhileStmtPlan`, and `loop_lowering` renders the
    supported shape as `loop { x = next(); if x.is_none()/is_some() { break; }
    ... }`. Signature constraints now mark locals assigned in null-tested
    condition expressions as mutable `Option<T>` storage, and unknown external
    calls flowing into optional slots trust the target shape instead of
    downcasting to a nominal `Optional`. Pending control-flow diagnostics are
    filtered for implemented assignment-in-condition plans. `loop_plan` also
    exposes a `PostLoopRefinementPlan` so the signature layer can prove
    `while ((x = read()) == null) { ... }; return x;` returns a non-null value
    while keeping local storage optional. Tail-recursive rewrites remain
    pending.
  - Coverage: `WhileAssignmentNotNull.java` and
    `WhileAssignmentNullVariants.java` no longer emit `(x = call())` in
    conditions or assignment-in-condition pending diagnostics, and the
    `== null` loop variant now returns the plain element type via the generic
    `Option<T>` to `T` conversion path.
  - Residual: negated forms such as `!((x = next()) == null)`, compound
    conditions such as `ready && (x = next()) != null`, and labelled breaks
    still require richer control-flow facts.

- [ ] `CTRL-2`: Replace synthetic switch state machines where simple `match` is valid.
  - Owner: `src/switch_lowering.rs`
  - Ideal Rust Target: Java switches that are simple value-selection or
    return-only forms should become ordinary Rust `match` expressions or
    statements. Integer, string, and enum selectors should use native Rust
    patterns, grouped Java labels should become combined Rust arms, and enum
    labels should preserve the parsed enum representation. Only switches that
    rely on Java fallthrough or labelled control flow should keep a structured
    state-machine fallback.
  - Implementation Rule: `switch_plan` owns classification of simple
    return/value switches versus fallthrough-preserving switches. It records
    selector shape, label groups, default behavior, and arm completion.
    `switch_lowering` renders either a Rust `match` from the plan or the
    existing explicit state-machine fallback. Lowering must not decide
    simplicity from ad hoc statement text after rendering has begun.
  - Required Facts: selector type/shape, enum variant spelling, string/int
    label values, grouped labels, default arm, arm completion kind, fallthrough
    presence, and labelled break/continue effects.
  - Owning Layer: parser/resolver preserve selector and enum/label facts;
    `switch_plan` owns switch classification; `switch_lowering` owns match or
    fallback rendering; identifier hygiene owns Rust-safe variant names.
  - Fallback Shape: fallthrough, labelled control flow, or ambiguous selector
    shapes should emit the structured state-machine fallback or a switch
    diagnostic, not a misleading partial `match`.
  - Evidence: enum/string/int switch fixtures render `match` or documented
    fallback, and GATK-style return switches avoid loop-state scaffolding when
    not needed.
  - Fixtures: `SwitchReturnsEnumMatch.java` should emit `match`, while
    fallthrough fixtures keep the state-machine path.
  - Status: Partial. `switch_plan` now detects simple return-only switches,
    including label-only grouped cases such as `case 2: case 3: return ...`,
    before lowering. `switch_lowering` renders these as Rust `match` statements
    with direct return arms and a default `_` arm; string selectors render via
    `selector.as_str()` so string literal labels become valid Rust patterns.
    Switches with real fallthrough bodies continue to use the existing
    state-machine renderer.
  - Coverage: new `SimpleReturnSwitches.java` covers int grouped labels and
    string selector patterns; `SwitchReturnsEnumMatch.java` now verifies the
    state-machine scaffolding is removed for return-only enum-shaped switches;
    `SwitchFallthrough.java` still exercises the fallthrough-preserving path.
    Design coverage asserts simple return-switch planning remains in
    `switch_plan` and match rendering remains in `switch_lowering`. Java enum
    declarations now have a distinct `ObjectRepresentation::JavaEnum` carrying
    unit variants derived from parsed enum constants; nested enum declarations
    are emitted before their containing type, and enum switch labels qualify as
    `Type::Variant`. `SwitchReturnsEnumMatch.java` now has a generated Rust
    compile/behavior regression.
  - Residual: enum variant naming is still reconstructed from parser-normalized
    identifiers (`s_n_p` -> `SNP`), so preserving source spelling or applying a
    stricter Rust naming policy should move into a parser/identifier hygiene
    pass. Exhaustive Java enum switches with a redundant `default` currently
    still emit `_`; that is valid Rust but can produce unreachable-pattern
    warnings.

- [ ] `EXC-1`: Design exception lowering around `Result`.
  - Owner: `src/exception_lowering.rs`, `src/signature.rs`,
    `src/semantic_analysis.rs`
  - Ideal Rust Target: Java methods that can throw should expose a Rust
    `Result<T, JavaException>` boundary only when callers must observe that
    exceptional control flow. Java `throw` should become `return Err(...)` or
    an expression-level `Err(...)` in a planned carrier; calls to throwing
    methods inside a `Result` context should use `?`; catch blocks should be
    ordered matches over structured exception metadata; and unchecked or
    uncaught exceptions at non-`Result` boundaries should be explicit panic or
    unsupported policies rather than hidden casts.
  - Implementation Rule: semantic analysis records thrown-effect and catch
    hierarchy facts; signature planning decides which functions require
    `Result` carriers; `exception_plan` classifies throw, try, catch, finally,
    and resource completion shapes; `exception_lowering` renders those plans
    into `Result` closures, propagation, catch guards, cleanup scopes, and
    runtime helpers. Ordinary expression/statement lowering must consume the
    planned carrier rather than rediscovering exception behavior from syntax.
  - Required Facts: thrown exception types, checked/unchecked boundary policy,
    catch assignability, method return shape, enclosing `Result` carrier,
    try-body completion, finally/resource cleanup behavior, cause chain, and
    external API thrown effects.
  - Owning Layer: `facts`/`semantic_analysis` own finite thrown/catch facts;
    `signature` owns `Result` return carriers; `exception_plan` owns Java
    exception control-flow classification; `exception_lowering` owns Rust
    rendering and runtime-helper selection.
  - Fallback Shape: if exception effects or completion cannot be planned,
    emit a structured unsupported exception diagnostic at the try/throw/call
    boundary. Do not lower exception hierarchy via nominal Rust downcasts or
    silently erase checked exceptions.
  - Evidence: throw, catch hierarchy, rethrow, wrapping, checked/unchecked
    boundary, and finally/try-with-resources fixtures produce valid plans.

- [x] `EXC-1B`: Classify exception constructor argument roles before rendering.
  - Owner: `src/exception_lowering.rs`, `src/lower.rs`
  - Ideal Rust Target: Java exception construction should always produce a
    `JavaException` value. Constructors with message/context arguments should
    render as `JavaException::new(class_name, message)`, combining multiple
    non-cause message parts into one display string. Constructors whose second
    argument is exception-like should render as
    `JavaException::with_cause(class_name, message, cause)`. Argument count
    alone must not decide cause semantics.
  - Implementation Rule: throw lowering asks expression-shape inference for
    the second constructor argument. If that argument is a known exception-like
    Java type (`Throwable`, `Exception`, `Error`, `JavaException`, or a name
    ending in `Exception`/`Error`), exception rendering emits `with_cause`.
    Otherwise, `exception_lowering` combines constructor message/context
    arguments with `format!` and emits `JavaException::new`.
  - Required Facts: thrown expression is `new ExceptionType(...)`, lowered
    constructor argument expressions, and the Java shape of the second
    argument when present.
  - Owning Layer: `exception_plan` identifies new-exception throws;
    `lower` classifies the second argument using expression shape facts;
    `exception_lowering` renders the `JavaException` constructor shape.
  - Fallback Shape: unknown non-exception arguments are treated as message
    parts, preserving a valid `JavaException` instead of guessing a cause.
  - Evidence: `exception_lowering` tests cover empty-message construction,
    message/context argument combination, and explicit cause construction. The
    GenomicsDB focused gate drops from `cargo_check_errors=223` to `220`, and
    the `expected JavaException, found String` constructor residual is gone.

- [ ] `EXC-2`: Lower try/catch/finally after throw-only support.
  - Owner: `src/exception_lowering.rs`, `src/lower.rs`, `src/facts.rs`
  - Ideal Rust Target: Java exception control flow should become ordinary
    `Result<T, JavaException>` control flow with catch clauses represented as
    ordered `match` arms over structured exception metadata. Rethrow patterns
    should preserve the existing exception value, cause-chaining should keep a
    typed `cause: Option<Box<JavaException>>`, and Java idioms such as
    `throw (ce.getCause() instanceof RuntimeException
        ? (RuntimeException) ce.getCause()
        : ce);`
    should render as a structured helper such as
    `return Err(ce.cause_or_self_if_instance(&["RuntimeException"]));`, not as
    a nominal Rust `downcast::<RuntimeException>()`.
  - Implementation Rule: `exception_plan` owns recognition of supported
    try/catch/finally and throw-expression shapes, including cause-or-self
    rethrow as a semantic exception plan. `exception_lowering` renders those
    plans into `Result` closures, catch guards, cleanup scopes, and exception
    runtime helpers. Generic conversion lowering must not rediscover exception
    hierarchy patterns or insert typed Rust downcasts for Java exception
    rethrow idioms.
  - Evidence: typed catch order, base catch covers subclass, cause chaining,
    and rethrow fixtures compile as `Result`-based Rust or explicit structured
    unsupported diagnostics.
  - Fixtures: `ExceptionSubclassCatchOrder.java`,
    `ExceptionBaseCatchCoversSubclass.java`, `ExceptionCauseChaining.java`,
    `ExceptionRethrowSpecific.java`,
    `ExceptionRuntimeCheckedHierarchy.java`, `SimpleTryCatchReturn.java`,
    `SimpleRethrowCatch.java`, `SimpleCauseChaining.java`.
  - Status: Partial. `exception_plan` now owns `TryStmtPlan` and recognizes a
    conservative catch-only shape: no resources, no `finally`, at least one
    catch clause, and try/catch bodies that either terminate at an exception
    boundary or fall through as unit. The generic unsupported-statement filter
    now allows those planned shapes to reach lowering while still rejecting
    resources/finally cases.
    `exception_lowering` renders supported try/catch blocks as a `match` over
    an immediately invoked `Result<_, JavaException>` closure, preserving catch
    order with `Err(ex) if ex.is_instance(&["Type"])` guards and propagating or
    panicking on uncaught exceptions according to the enclosing method's
    `Result` shape. Catch guards now consume semantic `CatchFact`
    assignability facts, so a superclass catch expands to the known reachable
    subclasses in its match list without recomputing hierarchy in lowering.
    `ThrowExprPlan` now distinguishes new exception construction, existing
    exception rethrow, and fallback thrown values; the runtime support carries
    `cause: Option<Box<JavaException>>` so constructor cause chains survive as
    structured data. The shared `JavaException` support item now also exposes
    Java-compatible `get_message()` and implements `std::fmt::Display`, so
    translated `Throwable.getMessage()` and exception `toString()`/formatting
    have a real runtime boundary instead of producing generated-crate method or
    trait errors. `call_plan` now owns `CallExceptionPropagationPlan`, so
    resolved source calls and unresolved external calls with explicit thrown
    effect facts are emitted with `?` in existing `Result` contexts rather than
    as plain side-effect calls. The external call receiver-key path is shared
    with semantic call-effect collection and resolves receiverless static
    utilities plus field-access receivers when explicit external field facts or
    built-in external-pack field metadata provide the receiver type.
    Expression-position calls with proven thrown effects now use the same
    propagation plan as statement-position calls when they are lowered into a
    typed slot. A local initializer or assignment such as
    `Header h = readHeader(path);` inside a `Result<_, JavaException>` method
    should become `let h = read_header(path)?;`, not a mismatched
    `Result<Header, JavaException>` value assigned to `Header`.
    Method-effect lookup accepts both the Java source spelling stored in
    semantic facts and the Rust snake-case spelling carried by resolved call
    targets, so effect propagation is selected by the same method fact instead
    of adding lowering-time aliases for individual method names.
    Source methods that call throwing source methods now inherit the thrown
    effect through finite direct call-graph propagation. A caller such as
    `String top() { return middle(); }` should receive a
    `Result<String, JavaException>` shell when `middle()` can throw, including
    recursive SCCs, so expression-position `?` operators always appear inside
    a compatible Rust carrier.
  - Coverage: `SimpleTryCatchReturn.java` has generated Rust compile/behavior
    coverage; `SimpleRethrowCatch.java` verifies unit fallthrough plus
    `throw ex;`, `SimpleCauseChaining.java` verifies `new E(message,
    cause)` preserves the cause, and `InternalThrowingCallInTry.java` verifies
    a source `throws` callee is propagated through `?` and caught. An explicit
    external-effect test injects a `Validator.validate throws
    ValidationProblem` fact and verifies the unresolved external call is also
    rendered as `validator.validate()?;`.
    `ExternalFieldThrowingCallInTry.java` verifies a GATK-shaped
    `Registry.DEFAULT_VALIDATOR.validate()` call can use an external field fact
    to find the `Validator.validate` thrown effect and emit `?`.
    `ThrowingCallLocalResult.java` verifies expression-position source calls
    with thrown effects use `?` in a local initializer. Method-effect design
    coverage asserts a Java fact named `readHeaderFromPath` matches a resolved
    Rust call target named `read_header_from_path` and still plans
    `PropagateResult`.
    `ThrowingCallChainResultShell.java` verifies transitive source-call thrown
    effects promote caller method shells and render both return-position and
    local-initializer calls with `?`. Semantic-fact coverage verifies thrown
    method effects propagate through both acyclic and recursive source call
    graphs as finite facts. The focused GenomicsDB gate with Tribble and
    GenomicsDB external packs moves from `cargo_check_errors=121` to `119`,
    and the prior `? operator can only be used in a method that returns
    Result or Option` residuals are gone.
    `try/finally` with no catches/resources and fallthrough unit bodies now has
    a dedicated `TryStmtPlan::FinallyOnly`: lowering stores the body
    `Result<(), JavaException>`, emits the cleanup block, and only then
    propagates or panics on the saved result. This covers cleanup-only finally
    patterns without pretending to support return-through-finally yet.
    `ExceptionSubclassCatchOrder.java`,
    `ExceptionBaseCatchCoversSubclass.java`,
    `ExceptionCauseChaining.java`, `ExceptionRethrowSpecific.java`, and
    `ExceptionRuntimeCheckedHierarchy.java` now verify ordered catch guards,
    rethrow/cause rendering, and superclass catch guards expanded with known
    subclass names instead of the old unsupported try/catch marker.
    `CauseRuntimeRethrow.java` covers the Java completion/cause idiom
    `throw (ce.getCause() instanceof RuntimeException ? (RuntimeException)
    ce.getCause() : ce)` and verifies that the generated Rust uses
    `ce.cause_or_self_if_instance(&["RuntimeException"])` instead of a nominal
    `downcast::<RuntimeException>()`. The focused GenomicsDB compile gate with
    the Tribble external pack now has `invalid_java_cast_downcast	0` for this
    package, with the former `ce.get_cause().downcast::<RuntimeException>()`
    path removed. The focused exception-runtime refresh keeps invalid-pattern
    buckets at zero and reduces cargo-check errors from `345` to `341` by
    removing missing `JavaException::get_message`/`Display` failures from the
    generated crate.
    `SimpleTryFinallyCleanup.java` has generated Rust compile/behavior
    coverage and is included in the neutral generated-crate gate. Design
    coverage asserts try planning lives in `exception_plan` and match/finally
    rendering lives in `exception_lowering`.
  - Residual: catch assignability is finite-fact based and limited to known
    reachable exception types; unknown external subclass worlds still need
    conservative pack facts or fallback diagnostics. External calls inside a
    planned try block now need better fact coverage for real package envelopes;
    without external packs, source summaries, or user overrides, a call such as
    `validator.validate()` remains a plain external call because no thrown
    effect has been proven. Caught exceptions can conservatively keep the
    enclosing method as `Result` until catch-coverage dataflow proves otherwise.
    `finally` remains limited to no-catch/no-resource fallthrough unit bodies;
    returns, thrown cleanup, catch+finally, and resource+finally still need a
    control-flow completion plan that can run cleanup on every exit path.

- [ ] `EXC-3`: Lower try-with-resources through scope/drop plans.
  - Owner: `src/exception_lowering.rs`, `src/lower.rs`
  - Ideal Rust Target: Java try-with-resources should become an explicit Rust
    scope that initializes each resource, runs the try body inside a
    `Result<T, JavaException>` closure, closes resources in reverse declaration
    order, and only then propagates either the saved body result or the matching
    catch result. A return-bearing Java body such as
    `try (Reader reader = open()) { return reader.readLine(); } catch (...) {
    return fallback; }` should preserve the returned value as
    `Result<String, JavaException>`, not collapse the body to `Result<()>` or
    emit a try/catch placeholder.
  - Implementation Rule: `exception_plan` classifies try-with-resources by
    resource declarations, catch coverage, finally presence, and try-body
    completion (`FallsThroughUnit` vs `Terminates`). `exception_lowering`
    owns the scope/drop rendering and receives the planned `ok_type` plus
    completion shape from `lower`; it must not infer Java resource semantics
    from raw syntax while rendering individual statements. External resource
    method calls such as `close`, `getHeader`, or `readLine` remain external
    API/facade responsibilities after the resource scope is structurally
    correct.
  - Evidence: resources with `close`, early return, catch, and finally fixtures
    do not emit placeholder syntax.
  - Status: Partial. `exception_plan` now recognizes a conservative
    `TryStmtPlan::WithResources` shape for resource declarations with no
    `finally`, mandatory catches, and either a fallthrough/unit body or a
    return/throw-terminating body. Lowering consumes that plan by creating an
    explicit resource scope, running the body inside a nested
    `Result<T, JavaException>` closure, closing resources in reverse
    declaration order, and then propagating the saved body result. For
    return-bearing bodies, the resource scope returns the saved
    `Result<T, JavaException>` after cleanup so the enclosing catch renderer
    can preserve normal `Ok(value)` and `Err(ex)` behavior. This avoids
    skipping cleanup when a body statement lowers with `?`, while keeping catch
    rendering on the existing `try_catch_stmt` path. `TryWithResourcesCopy` is
    now a positive regression for resource initialization, body calls, and
    reverse close ordering, and `TryWithResourcesReturn` covers value-returning
    resource bodies. The focused GenomicsDB compile gate now reports
    `try_placeholder	0` and shows `getHeaderFromPath` as a structured
    try-with-resources `Result<Option<VCFHeader>, JavaException>` scope rather
    than `compile_error!("unsupported Java statement: try/catch/finally")`.
    `finally` composition remains unsupported until a value-capturing plan and
    finalizer/suppressed-exception policy are added. Identifier hygiene for
    Java locals that are Rust keywords is a separate parser/name-hygiene task,
    not owned by exception lowering.

- [ ] `INIT-1`: Plan constructor initialization before rendering bodies.
  - Owner: `src/signature.rs`, `src/item_assembly.rs`, `src/lower.rs`
  - Ideal Rust Target: Java constructor delegation should either become a Rust
    constructor call that fully initializes the local Rust struct, or remain a
    structured unsupported inheritance diagnostic. Same-class `this(...)`
    delegation should return `Self::target(...)` plus planned trailing field
    updates. Ordinary `super(...)` with inherited state should not be guessed.
    A narrow external exception superclass constructor such as
    `class X extends UserException { X(String message) { super(message); } }`
    should initialize only the local Rust struct fields while thrown values are
    represented through `JavaException::new("X", message)` at throw sites.
  - Implementation Rule: `constructor_plan` owns the distinction between
    same-class delegation, external exception superclass delegation, and
    unsupported inheritance delegation before lowering. `lower` consumes that
    plan: same-class delegation calls another `Self::new_*`, external exception
    superclass delegation reuses the normal local field-initialization plan,
    and ordinary superclass delegation still emits the explicit inheritance
    diagnostic. Exception value construction remains owned by
    `exception_plan`/`exception_lowering`, not by constructor lowering.
  - Evidence: `super(...)`, `this(...)`, field initializers, extra statements
    after delegation, and inherited field initialization have tests and no
    invalid placeholder Rust.
  - Status: Partial. `constructor_plan` now distinguishes pure same-class
    delegation from `this(...)` followed by supported trailing initialization
    work. `ConstructorDelegationPlan::SameClassCallWithTrailing` captures
    trailing `this.field = value` assignments and resolved implicit instance
    calls without letting lowering rediscover Java constructor rules. Lowering
    consumes the plan by initializing `let mut __java_self = Self::target(...)`,
    applying the planned trailing field updates and instance calls to that temp
    object with normal slot/call argument coercion, and returning
    `__java_self`. `ConstructorDelegationWithExtraWork.java` is now a positive
    regression instead of an unsupported placeholder. A narrow
    external-exception superclass plan now accepts constructor bodies that only
    delegate to `Exception`, `RuntimeException`, `UserException`, or
    `GATKException`; lowering initializes local fields and leaves actual thrown
    exception values to the `JavaException` runtime.
    `ExceptionSubclassSuperConstructor` covers this neutral shape, while
    `SuperConstructorState` still proves ordinary inherited-state `super(...)`
    remains unsupported. The focused GenomicsDB compile gate now reports
    `super_constructor	0` and the nested
    `UnableToCreateGenomicsDBWorkspace` constructor no longer emits
    `compile_error!("unsupported Java constructor: super(...) requires
    inheritance lowering")`.
  - Residual: ordinary `super(...)` still needs inheritance-aware
    initialization, inherited fields need a base-state composition plan, and
    arbitrary trailing statements after `this(...)` remain unsupported unless
    classified into a constructor-initialization plan. Rust keyword hygiene for
    Java local names is still a parser/name-hygiene concern, not
    constructor-specific lowering.

- [x] `CTRL-3`: Add tail-recursion and recursion fallback policy.
  - Owner: `src/semantic_analysis.rs`, `src/lower.rs`, `src/signature.rs`
  - Ideal Rust Target: recursion should remain ordinary Rust recursion unless
    a local, safe, direct tail-recursive accumulator shape can be proven. That
    proven shape should become a `loop` with mutable accumulator/parameter
    slots and no semantic dependency on Rust tail-call optimization. Mutual
    recursion, receiver recursion not proven to target `this`, exception-
    wrapped recursion, and non-tail recursion should remain finite facts plus
    ordinary calls or explicit diagnostics.
  - Implementation Rule: recursion detection and tail-shape classification
    belong to semantic/control-flow planning before lowering. Lowering consumes
    only a planned safe tail-recursive rewrite, and otherwise emits normal
    calls while preserving recursion diagnostics. Numeric promotions,
    ownership adapters, and result carriers remain in their existing
    coercion/signature/exception layers; tail recursion lowering must not add
    operation-specific conversion rules.
  - Required Facts: call graph SCCs, direct vs mutual recursion, receiver
    identity, tail position, accumulator parameter updates, method return
    carrier, exception `Result` carrier, argument slot shapes, and unsupported
    recursion diagnostics.
  - Owning Layer: `graph`/SCC analysis records recursion groups;
    `semantic_analysis` records recursion and TCO fallback facts;
    control-flow planning recognizes safe tail accumulator shapes; `signature`
    owns mutable parameter/local slots; lowering renders only the approved loop
    plan.
  - Fallback Shape: emit ordinary recursive calls for supported Rust recursion
    and structured `recursion_tco_fallback` diagnostics for Java code that
    relies on tail-call optimization the translator cannot prove or lower.
  - Evidence: tail-recursive accumulator can lower to a loop when safe; mutual
    recursion remains ordinary functions with finite facts; unsupported TCO
    assumptions produce diagnostics, not silent rewrites.
  - Status: Implemented. Safe self tail-recursive accumulator methods with the
    exact `if (base) return value; return self(args...)` shape now lower through
    a dedicated control-flow rendering module into a Rust `loop` with parameter
    reassignment. The rewrite is gated by semantic control-flow plans, skipped
    for `Result`-wrapped exception methods, and covered by both string-level
    idiom tests and the generated-crate compile gate. The accumulator lowering
    uses slot-shaped argument lowering, so numeric Java promotion such as
    `long acc * int n` is handled in the operator/coercion layer instead of as a
    tail-recursion special case. Mutual recursion remains ordinary calls backed
    by finite recursion facts. Unsupported TCO-style assumptions now produce the
    semantic warning code `recursion_tco_fallback` for direct recursion that
    does not match the safe accumulator loop shape, mutual tail-style recursion,
    receiver-recursive calls not proven to be `this`, and exception-wrapped
    methods where the safe loop shape is not lowered because the method becomes
    `Result`.
  - Tests: `cargo test --test semantic_facts recursion_policy`; `cargo test
    --test semantic_facts tail_recursive`; `cargo test --test slot_shape_design
    control_flow_normalization_plans_are_owned_before_lowering`;
    `cargo test --test idiom_gaps neutral_idiom_fixtures_generated_crate_cargo_checks`.

### 10. Emitted Rust Compile Gates

- [x] `GATE-1`: Add a generated-crate harness for neutral idiom fixtures.
  - Owner: `tests/idiom_gaps.rs`, test helpers
  - Evidence: selected fixtures are checked with `cargo check`, not only string
    assertions.
  - Implemented as `cargo_check_generated_modules` in `tests/idiom_gaps.rs`.
    The aggregate `neutral_idiom_fixtures_generated_crate_cargo_checks` test
    builds a temporary crate under `/tmp`, writes generated fixture outputs as
    isolated modules, and runs `cargo check --tests --quiet`. Current neutral
    coverage spans `ForEachLambdaSideEffect.java`,
    `MethodReferenceFlatMap.java`, `SwitchReturnsEnumMatch.java`,
    `SimpleTryCatchReturn.java`, and `InternalThrowingCallInTry.java`, covering
    lambda side effects, method references/collectors, enum switch `match`, and
    `Result`-based try/catch propagation.

- [x] `PKG-1`: Add a focused GATK package compile gate.
  - Owner: `scripts/`, integration tests if runtime cost is acceptable
  - Evidence: package translation plus selected source summaries/external
    packs creates a temp crate and runs `cargo check`.
  - Start with the smallest resolver-clean package envelope. Current best
    candidates from recorded reports are `tools/genomicsdb`,
    `tools/spark/sv/evidence` with source summaries, and
    `tools/walkers/variantutils` with source summaries.
  - Implemented by `scripts/translate-focused-package.sh`; first recorded gate
    is `tools/genomicsdb` in `docs/gatk-resolver-benchmark-report.md`.

- [x] `GATE-2`: Track invalid Rust patterns as hard failures for focused gates.
  - Owner: `scripts/`, `tests/idiom_gaps.rs`
  - Evidence: focused gate fails on `compile_error!`, Java `.add(` on `Vec`,
    `None` assigned to concrete slots, unresolved downcast placeholders, and
    lambda/method-reference placeholder markers.
  - Implemented in `scripts/translate-focused-package.sh` with
    `invalid-patterns.tsv`; the gate exits nonzero when any tracked invalid
    pattern appears.

- [x] `GATE-3`: Keep idiom sweep as triage, not a pass/fail oracle for full GATK.
  - Owner: `docs/gatk-current-idiom-audit.md`, scripts
  - Evidence: sweep report ranks patterns by count/files and maps them to
    generalized fixture tests.
  - Status: Implemented as a reproducible triage tool. The tracked
    `scripts/sweep-generated-idioms.py` scans generated Rust, ranks
    regex-defined idiom signals by occurrence/file count, maps examples back to
    Java source through `status.tsv`, and names the owning layer plus
    generalized fixture coverage for each signal.
    `scripts/translate-focused-package.sh` now runs the sweep after translation and writes
    `idiom-sweep.md` / `idiom-sweep.tsv` beside the focused gate outputs,
    while keeping compile status and invalid-pattern checks as the only hard
    gate failures. `docs/gatk-current-idiom-audit.md` documents the direct
    command, focused-gate output locations, and the signal-to-fixture ownership
    map.
  - Residual: the signal list is intentionally heuristic and should be updated
    whenever a new high-count corpus pattern appears; a count decrease alone is
    not accepted as correctness evidence without fixture and compile-gate
    coverage.

- [x] `GATE-4`: Record every package compile gate in the benchmark report.
  - Owner: `docs/gatk-resolver-benchmark-report.md`
  - Evidence: report has command, selected dependency envelope, pass/fail,
    top compile errors, owning layer, and next planned slice.
  - First entry recorded under "Focused Rust Compile Gate: GenomicsDB".

## Recommended Execution Order

1. Add the package-level generated Rust compile harness.
2. Run it on the cleanest focused package envelope, likely `tools/genomicsdb`.
3. Classify the first compile failures by owning layer.
4. Implement SCC/worklist phase counters and lattice audit tests in parallel
   with the first compile-gate harness.
5. Tackle invalid Rust patterns in this order:
   - typed downcasts and `None`/`Option` mismatches
   - Java collection/stream API spelling
   - lambdas and method references
   - arrays and class literals
   - constructor delegation and `super(...)`
   - exceptions and try-with-resources
6. After each slice, run:
   - focused layer tests
   - `cargo test --quiet`
   - focused package resolver benchmark
   - focused generated Rust compile gate
7. Update the benchmark report with corpus evidence and generalized rule.

## First Concrete Slice

Implement the compile harness before more semantic saturation work. The
resolver/semantic reports already show several focused packages can be made
clean enough at the fact layer; the next unknown is which planner/lowering gap
blocks `cargo check` first.

Tasks:

- [x] `FIRST-1`: Add `scripts/translate-focused-package.sh` or equivalent.
- [x] `FIRST-2`: Inputs:
  - primary Java package path
  - optional `--source-root` summary/co-translate roots
  - optional external pack manifest/facts
  - output temp crate path
- [x] `FIRST-3`: Output:
  - generated Rust files
  - `Cargo.toml`
  - status file mapping Java source to Rust output
  - compile log
- [x] `FIRST-4`: Run `cargo check` on the generated crate.
- [x] `FIRST-5`: Add one non-default ignored integration test or documented command so
  the gate can be run deliberately without slowing ordinary `cargo test`.
- [x] `FIRST-6`: Run first against `tools/genomicsdb`, because the recorded resolver
  benchmark is clean for parser, resolver, semantic, missing-pack, unexplained,
  and object-representation diagnostics.
- [x] `FIRST-7`: Record the first compile errors in `docs/gatk-resolver-benchmark-report.md`
  under a new "Focused Rust Compile Gate" section.

Acceptance:

- The harness runs from a clean checkout with only local corpus inputs.
- It does not require broad GATK to finish.
- It captures top compile errors in a machine-readable file.
- It gives the next implementation slice an owning layer instead of a generic
  "GATK does not compile" result.

## Clearing Rules

An item can be checked off only when:

- a test, script, benchmark report entry, or direct code reference proves it
- the owning module is the one changed
- every changed transformation rule still puts **Ideal Rust Target** before
  **Implementation Rule**, status notes, and lowering details
- the **Ideal Rust Target** section states the Rust API, expression, type,
  ownership/nullability shape, dispatch form, or fallback diagnostic we would
  want to maintain by hand before the implementation rule explains mechanics
- new behavior is not hidden in lowering if an earlier layer owns the decision
- focused tests and full `cargo test --quiet` pass
- the benchmark report records whether GATK movement occurred

Do not check off an item merely because a narrow string assertion changed or
because one GATK file no longer shows the symptom.
