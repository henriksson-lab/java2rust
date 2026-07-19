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
- **Implemented Extension**: external static support now covers const-safe
  scalar field facts, not only strings. `String` constants render as
  `&'static str`, primitive constants render as Rust primitive associated
  constants, and owners present in the co-translated source context are
  suppressed so sibling source types keep ownership of their constants and
  constructors. Built-in external field metadata can provide the field type
  even when the focused gate has not enabled that full external pack.
- **Evidence**:
  `context_static_string_constant_owner_stays_sibling_source_type`,
  `external_static_primitive_constant_support_is_generated_from_field_facts`,
  and focused gate
  `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_static_scalar_implicit_ctor_v2`.
  In that gate, `GenomicsDBArgumentCollection::new()` is emitted on the real
  sibling source type, `GenotypeLikelihoods` gets a typed scalar support
  constant, and `cargo_check_errors` moves from `18` to `17`.

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
  - Inherited support field type shell rule:
    - Ideal Rust Target: when an inherited source-support field is flattened
      into an emitted Rust struct and its Java type is not otherwise emitted,
      the generated Rust should include a minimal opaque support shell such as
      `#[derive(Clone, Debug, Default)] pub struct ProgressMeter;`. The field
      type should therefore compile as `progress_meter: ProgressMeter` without
      requiring the whole superclass or dependency body to be lowered.
    - Implementation Rule: external facade support generation scans inherited
      instance fields discovered from the support program for each emitted
      type, adds their declared Java field types using only the emitted primary
      program as the local-type suppression set, and reuses the existing
      opaque facade item renderer. It does not change direct sibling source
      fields or lower methods from the support body.
    - Required Facts: emitted primary type set, support superclass field
      declarations, inherited-field flattening plan, Java field type, and
      builtin facade-type exclusion.
    - Owning Layer: `inherited_members` owns support-field discovery;
      `stdlib` owns opaque external facade support item emission; `lower` only
      requests facade support items before emitting translated types.
    - Fallback Shape: if a field type is local to the emitted primary program
      or is a builtin facade type, no extra opaque shell is emitted. Direct
      source-summary field types still require a source-summary/full-context
      distinction before broad shell emission.
    - Evidence:
      `inherited_source_summary_field_types_emit_opaque_support_shells` and
      the focused GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_inherited_type_shells_v1`,
      where `cargo_check_errors` moved from `63` to `57` and inherited engine
      field types such as `ProgressMeter`, `IntervalArgumentCollection`,
      `ReadInputArgumentCollection`, `ReadsDataSource`,
      `ReferenceDataSource`, `FeatureManager`, and
      `SequenceDictionaryValidationArgumentCollection` are emitted as opaque
      support structs.
  - Direct source-summary type shell rule:
    - Ideal Rust Target: when an emitted primary declaration references a type
      that is available only through source-summary context, the Rust output
      should include a minimal opaque shell for that summary type while still
      suppressing shells for full co-translated sibling context. For example,
      a primary field `FeatureInput headerOverride` should become
      `header_override: Option<FeatureInput>` plus
      `#[derive(Clone, Debug, Default)] pub struct FeatureInput;` when
      `FeatureInput` came from source-summary context, but not when it came
      from full context.
    - Implementation Rule: translation carries a separate summary-only
      program alongside the merged support program. External facade support
      computes emitted local types, full support local types, and
      source-summary types separately. It emits opaque shells for primary
      declarations whose type key appears in the summary-only set and not in
      the emitted primary set, while preserving the existing full-context
      sibling suppression rule.
    - Required Facts: emitted primary type set, full support type set,
      source-summary-only type set, declared Java field/method/constructor
      types, and builtin facade-type exclusion.
    - Owning Layer: `lib` owns context separation; `lower` passes the summary
      context to support generation; `stdlib` owns opaque facade shell
      emission. Resolvers and renderers do not inspect source-root labels.
    - Fallback Shape: if source-summary provenance is unavailable, the type is
      treated like ordinary full support context and no direct opaque shell is
      emitted.
    - Evidence:
      `source_summary_direct_field_type_emits_opaque_support_shell` proves
      summary-only field types emit shells while full-context sibling types do
      not.
  - Selected source-summary method surface rule:
    - Ideal Rust Target: an opaque source-summary shell should expose only the
      summary-declared or summary-inherited methods that are actually called
      on that selected shell. A summary-only `FeatureInput extends GATKPath`
      whose inherited `toPath()` is used should emit
      `impl FeatureInput { pub fn to_path(&self) -> Path { ... } }`, but the
      translator should not emit methods for every type in the summary root.
    - Implementation Rule: facade support first selects opaque shells from
      emitted primary declarations and inherited support fields. It then scans
      the source-summary method hierarchy only for those selected shells,
      filters methods by used Rust method name and arity, and emits generic
      instance facade methods with return types projected through the existing
      external facade type renderer.
    - Required Facts: selected opaque shell set, source-summary superclass
      chain, parsed method names, arities, return types, and used call arities
      from the emitted primary program.
    - Owning Layer: `stdlib` owns source-summary facade method surfaces;
      `inherited_members` owns field-based shell selection; `call_plan` and
      lowering continue to render ordinary method calls.
    - Fallback Shape: if a method is not in the selected shell's summary
      hierarchy or its arity is not used by the primary output, no facade
      method is emitted and the compile gate exposes the missing surface.
  - Evidence:
      `source_summary_opaque_type_gets_used_inherited_method_surface` and the
      focused GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_summary_method_surfaces_v2`,
      where `cargo_check_errors` moved from `57` to `56` and
      `FeatureInput::to_path` is emitted without pulling the entire
      source-summary package surface into opaque support.
  - Late-selected source-summary return surface rule:
    - Ideal Rust Target: when an external fact selects a source-summary return
      type after the first facade-support pass, that returned shell should get
      the same narrow, used-only method surface as a directly selected
      summary shell. If a selected summary method returns a dependency type
      outside the summary envelope, the generated Rust should still emit a
      minimal support shell for that return type, for example
      `pub struct SAMFileHeader;` plus
      `pub fn get_header(&self) -> SAMFileHeader`. Java raw iterator return
      types should not render as Rust's prelude `Iterator` trait in concrete
      return position; they should use an explicit opaque iterator boundary
      such as `CloseableIterator`.
    - Implementation Rule: `stdlib` runs source-summary method-surface
      selection as a finite saturation step over the selected opaque shell
      set. Each emitted summary method contributes its return and parameter
      Java types back into facade support selection, including rendered
      facade boundary types. Existing weak external methods are replaced only
      when the Java method name and arity match and the replacement return
      type is stronger than `()` or `Box<dyn Any>`. Facade return rendering
      maps Java `Iterator`/`java.util.Iterator` to the explicit
      `CloseableIterator` boundary instead of the Rust trait name.
    - Required Facts: selected opaque shell set, source-summary method
      hierarchy, used method name and arity, Java return and parameter types,
      external facade return renderer, and weak-return classification.
    - Owning Layer: `stdlib` owns facade support saturation and weak method
      replacement; `slot_render` still owns ordinary source signature
      rendering; call lowering only consumes the resulting method surface.
    - Fallback Shape: if a summary method is unused, no method surface is
      emitted. If a return type cannot be rendered as a valid facade boundary,
      the existing external fallback remains responsible for the compile
      diagnostic.
    - Evidence:
      `source_summary_method_surface_can_be_added_after_external_return_selects_type`
      covers a late-selected summary shell, a summary method returning an
      external non-summary type, and Java raw `Iterator` facade returns. The
      focused GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_summary_return_surface_v2`
      moves from the prior best `cargo_check_errors=52` to `51`; the earlier
      invalid `SAMFileHeader` and raw Rust `Iterator` regressions are gone.
  - External-pack typed collection/dictionary getter rule:
    - Ideal Rust Target: external-library getters with known collection or
      dictionary returns should expose concrete Rust facade return types. For
      example, HTSJDK `VCFHeader.getMetaDataInInputOrder()` should render as
      `std::collections::HashSet<VCFHeaderLine>`, and
      `VCFHeader.getSequenceDictionary()` should render as
      `SAMSequenceDictionary`, not as `()` or `Box<dyn Any>`. HTSJDK
      `VCFHeader.getGenotypeSamples()` should render as `Vec<String>`, so
      indexed sample access composes through ordinary `Vec::get`.
    - Implementation Rule: external pack facts must carry the Java return
      type when the pack knows it. Facade support renders Java `Set<T>` and
      `HashSet<T>` as `std::collections::HashSet<T>`, while preserving the
      existing `Vec<T>` projection for list-like returns. Lowering consumes
      only the planned method surface; it does not recognize specific HTSJDK
      method names.
    - Required Facts: external API effect receiver, method name, arity,
      return Java type, return nullability, and facade collection projection.
    - Owning Layer: `external_packs` owns HTSJDK API return facts; `stdlib`
      owns facade return projection; call lowering only renders the selected
      method call.
    - Fallback Shape: if the pack lacks a return type, the external facade
      remains a weak boundary and compile diagnostics expose the missing
      precision. Unknown collection element types still use the existing
      erased-object fallback.
    - Evidence:
      `htsjdk_vcf_pack_models_header_getters_and_mutators` now asserts typed
      returns for `VCFHeader.getMetaDataInInputOrder` and
      `VCFHeader.getSequenceDictionary`.
      `external_pack_typed_set_and_dictionary_getters_shape_facade_returns`
      proves generated facade support typechecks for these returns.
      `external_pack_typed_string_list_getter_supports_indexed_sample_access`
      proves typed list returns support immediate Java `List.get(0)` use. The
      focused GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_vcf_typed_getters_v1`
      moves from `cargo_check_errors=51` to `48`; the `E0308` bucket drops
      from `21` to `16`. The later
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_domain_getter_surfaces_v3`
      gate includes this list-return precision and removes the previous
      `no method named get found for unit type ()` residual.
  - Imported static external return facade rule:
    - Ideal Rust Target: a Java imported static call whose external pack knows
      the return type should emit a concrete support method and declare any
      required opaque return facade once. For example,
      `BucketUtils.getPrefetchingWrapper(int)` should render as
      `BucketUtils::get_prefetching_wrapper(...) -> Function`, with
      `pub struct Function;` available in support, not as a generic
      `R: Default` stub that asks Rust inference to rediscover a Java API
      boundary.
    - Implementation Rule: external pack API-effect facts carry the receiver,
      method, arity, return Java type, and return nullability for imported
      static methods. `stdlib` registers the fact-backed return Java type while
      adding imported static owner calls, then emits the support method through
      the normal external facade renderer. Lowering only renders the static
      call selected by call planning; it does not special-case `BucketUtils`,
      GATK, or functional interfaces.
    - Required Facts: imported static owner alias, method name, arity,
      external API effect return type, return nullability, and facade type
      projection.
    - Owning Layer: `external_packs` owns library-specific return facts;
      `stdlib` owns opaque facade declaration and support-method emission;
      call lowering owns only the already-selected call syntax.
    - Fallback Shape: if an external pack does not know the return type, the
      previous generic `R: Default` imported-static fallback remains as
      compile-gate pressure. If the return facade is too weak to model yet,
      the pack may intentionally choose `Object` and get `Box<dyn Any>`, but
      that decision belongs to the pack fact, not the call site.
    - Evidence:
      `broad_http_pack_declares_retry_settings_boundary` now proves
      `BucketUtils.getPrefetchingWrapper(int)` is pack-owned and returns
      non-null `Function<SeekableByteChannel, SeekableByteChannel>`.
      `broad_http_pack_emits_function_facade_for_bucket_prefetch_wrapper`
      proves emitted support declares `Function` and no longer emits a generic
      `get_prefetching_wrapper<R: Default, ...>` fallback. The focused
      GenomicsDB gate moves from
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_feature_reader_factory_v1`
      at `cargo_check_errors=18` to
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_bucketutils_prefetch_v2`
      at `cargo_check_errors=17`; the `Function` missing-type and
      `BucketUtils` generic inference residuals are gone.
  - Imported static owner shadowing rule:
    - Ideal Rust Target: a Java local, parameter, catch binding, field, or
      enhanced-for binding whose name normalizes to the same Rust spelling as
      an imported type should still call instance methods through an instance
      facade. For example, `ImportConfig importConfig; importConfig.setX(...)`
      should emit `import_config.set_x(...)` and support
      `impl ImportConfig { pub fn set_x<T0>(&self, _arg0: T0) -> () { ... } }`,
      not reserve a static `ImportConfig::set_x(...)` support method.
    - Implementation Rule: imported-static support discovery threads a
      member-scope shadow set through expression traversal. The shadow set is
      built from fields, params, local declarations, enhanced-for variables,
      resources, and catch bindings, storing both Java spelling and
      snake-case Rust spelling. A receiver variable that appears in this set
      cannot match a normal-import owner alias, while real type-owner calls
      such as `BucketUtils.getPrefetchingWrapper(...)` still match the import
      alias path.
    - Required Facts: normal import aliases, static import member owners,
      member-scope shadow names, call receiver variable spelling, method
      arity, and external API effect role.
    - Owning Layer: `stdlib` owns support-surface discovery and static-owner
      collection. External packs own whether a method is mutator/getter/etc.,
      but they do not decide whether a receiver token is a local value or a
      type owner. Lowering only emits the resolved instance/static call shape.
    - Fallback Shape: when shadow evidence is unavailable, preserving the
      existing static-owner candidate is acceptable because it keeps imported
      utility calls working. If a future lexical edge case appears, extend the
      shadow collector rather than adding method-name exceptions.
    - Evidence:
      `genomicsdb_import_config_setters_emit_instance_facades_when_local_shadows_import_alias`
      proves an `ImportConfig importConfig` parameter emits an instance setter
      facade and does not emit the static associated-function setter fallback.
      `translates_imported_static_utility_owner_without_module_receiver` and
      `broad_http_pack_emits_function_facade_for_bucket_prefetch_wrapper`
      continue to prove real imported static owner calls still work. The
      focused GenomicsDB gate moves from
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_bucketutils_prefetch_v2`
      at `cargo_check_errors=17` to
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_importconfig_instance_setters_v1`
      at `cargo_check_errors=12`; the five `ImportConfig` setter `E0599`
      rows are gone.
  - Known imported static return and passthrough facade rule:
    - Ideal Rust Target: known Java static utilities should not emit
      unconstrained `R: Default` support stubs when the Java API already
      defines either a concrete return facade or a type-preserving
      passthrough. For example, `JsonFormat.parser()` should return `Parser`
      and expose `Parser::merge(...) -> ()`, while GATK
      `Utils.nonNull(value)` should render as `fn non_null<T0>(value: T0) ->
      T0` so statement-position validation and expression-position
      passthrough both compile.
    - Implementation Rule: imported static support first checks pack facts,
      then known external return facts, then known first-argument passthrough
      utilities before falling back to generic `R: Default`. Known imported
      static returns are also registered as opaque facade types, so chained
      calls can receive support methods. Returned opaque facades may declare
      known side-effect instance methods such as `Parser.merge` through the
      same support-surface path.
    - Required Facts: imported static owner, method name, arity, known return
      Java type, passthrough classification, and chained facade method role.
    - Owning Layer: `stdlib` owns Java platform/general utility support shapes
      that are not specific to one co-translated source package. External
      packs still override with precise facts when a dependency pack owns the
      API. Lowering only renders the static call and chained instance call.
    - Fallback Shape: if no fact or known utility shape exists, keep the
      generic `R: Default` fallback as compile-gate pressure. Do not add
      call-site type annotations to silence inference; the API boundary should
      be modeled once.
    - Evidence:
      `imported_static_known_return_declares_chained_facade_surface` proves
      `JsonFormat.parser().merge(...)` declares `Parser`, emits
      `parser() -> Parser`, and emits `Parser::merge(...) -> ()`.
      `imported_static_non_null_validator_returns_first_argument_facade`
      proves GATK `Utils.nonNull` emits `T0 -> T0` and not `R: Default`.
      The focused GenomicsDB gate moves from
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_importconfig_instance_setters_v1`
      at `cargo_check_errors=12` to
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_static_known_returns_v1`
      at `cargo_check_errors=10`; the `JsonFormat::parser` and
      `Utils::non_null` generic inference rows are gone.
  - Typed external call optional receiver adaptation rule:
    - Ideal Rust Target: a Java local or field represented as `Option<T>` in
      Rust should be unwrapped through the normal receiver access path before
      a read-only external instance call. For example,
      `header.getSequenceDictionary()` where `header` is
      `Option<VCFHeader>` should render as
      `header.as_ref().unwrap().get_sequence_dictionary()`, not
      `header.get_sequence_dictionary()`.
    - Implementation Rule: all external-call lowering paths must use the
      shared call-target receiver adaptation before applying the target. The
      typed external call fast path in `lower_external_call_expr_for_slot`
      consumes the same `adapt_call_target_receiver` helper as ordinary call
      lowering, so optional receiver handling is selected from the slot shape
      and target kind rather than from a method-specific branch.
    - Required Facts: call target kind, receiver expression, receiver slot
      shape, external API return type, and optional storage shape.
    - Owning Layer: `lower` owns receiver adaptation reuse across call
      lowering paths; `signature` owns the optional local/field shape; external
      packs only provide the return fact that makes the typed fast path active.
    - Fallback Shape: if the receiver shape is not optional, the receiver is
      emitted unchanged. If the call is unsupported or unresolved, the
      existing fallback/diagnostic path remains responsible.
    - Evidence:
      `optional_local_external_receiver_getter_is_unwrapped_before_call`
      proves a direct typed external call on an optional local lowers through
      `as_ref().unwrap()`. The focused GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_optional_external_receiver_v1`
      moves from `cargo_check_errors=48` to `46`; the two
      `Option<VCFHeader>.get_sequence_dictionary` errors are gone.
  - Weak opaque domain getter strengthening rule:
    - Ideal Rust Target: when an opaque external/source-summary support type
      already has a weak method surface for a known zero-argument domain
      getter, generated Rust should expose the domain value type directly. For
      example, `SimpleInterval.getContig()`, `getStart()`, and `getEnd()`
      should render as `String`, `i32`, and `i32`; downstream map lookups,
      formatting, and numeric comparisons should then use ordinary Rust types.
    - Implementation Rule: external facade support may replace an existing
      weak instance method (`()` or `Box<dyn Any>`) with the
      `known_external_method_return_type` result for the same name and arity.
      It must not add every known getter to every opaque type, and it must not
      create an instance method when the type already has a non-matching
      static/helper method of that name.
    - Required Facts: used call name/arity, opaque support type method
      surface, weak-return classification, and known domain getter return
      table.
    - Owning Layer: `stdlib` owns weak facade method replacement and the
      known domain getter table; external packs own explicit API facts when a
      library-specific return is known; lowering only consumes the resulting
      method signature.
    - Fallback Shape: if no weak method surface exists for that owner, no
      method is added. If an existing method is already concrete or conflicts
      by name with a static/helper method, the support pass leaves it alone and
      the focused compile gate exposes any remaining type gap.
    - Evidence:
      `external_pack_typed_locatable_coordinate_getters_support_numeric_and_string_use`
      proves typed coordinate getter facades typecheck for generic
      `Locatable` use. The focused GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_domain_getter_surfaces_v3`
      moves from `cargo_check_errors=46` to `42`; the prior
      `String: Borrow<()>`, `() doesn't implement Display`, and
      `get found for unit type ()` buckets are gone. Generated
      `SimpleInterval` now exposes `get_contig() -> String`,
      `get_start() -> i32`, and `get_end() -> i32`.
      `selected_opaque_support_type_gets_used_known_method_surfaces_by_arity`
      extends this rule to selected opaque support shells with owner-specific
      known method surfaces and used arities: `IntervalArgumentCollection`
      gets `intervals_specified() -> bool` and
      `get_intervals(...) -> Vec<SimpleInterval>`, while unrelated opaque
      shells do not get those methods. The same regression keeps
      `SimpleInterval` locatable getters owner-specific. The focused
      GenomicsDB gate with `com.intel.genomicsdb` and
      `htsjdk.samtools.util` packs moves from
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_final_domain_collection_arg_v1`
      at `cargo_check_errors=39` to
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_known_opaque_surfaces_v3`
      at `cargo_check_errors=37`; the missing
      `IntervalArgumentCollection.intervals_specified` and
      `IntervalArgumentCollection.get_intervals` buckets are gone.
  - External locatable facade boundary rule:
    - Ideal Rust Target: a Java parameter typed as an external domain
      interface with known value getters should preserve a typed facade
      boundary when the translated body calls those getters. For HTSJDK
      `Locatable`, the target shape for a read-only helper is
      `fn new(reader: ..., interval: &Locatable) -> ...`, so calls such as
      `interval.get_contig()`, `interval.get_start()`, and
      `interval.get_end()` are ordinary typed facade calls, not method calls
      on `&Box<dyn Any>`.
    - Implementation Rule: the HTSJDK samtools-util external pack declares
      `Locatable` and `htsjdk.samtools.util.Locatable` as concrete external
      facade strategies instead of erased-object strategies. The existing
      signature and support-surface pipeline then preserves the concrete
      parameter slot and renders the typed getter methods from external API
      facts. This rule is deliberately pack-owned and narrow; it does not
      promote broad GenomicsDB concrete facades before the shared facade
      identity follow-up is implemented.
    - Required Facts: external object strategy for the receiver type,
      external API effects for `getContig`, `getStart`, and `getEnd`, and the
      translated method parameter's Java type.
    - Owning Layer: `external_packs` owns the library-specific object strategy
      and typed method facts; `signature` consumes the representation fact;
      `stdlib` emits the facade support methods; lowering only renders the
      selected slot and method calls.
    - Fallback Shape: external interfaces without a pack-owned concrete,
      dyn-trait, closed-enum, or crate-mapped strategy remain erased-object
      boundaries with explicit compile-gate pressure. Broad concrete promotion
      remains blocked until shared external facade identity is available.
    - Evidence:
      `htsjdk_util_locatable_pack_strategy_preserves_typed_facade_boundary`
      proves both simple and qualified `Locatable` pack facts use
      `HierarchyStrategy::ConcreteStruct`.
      `external_pack_typed_locatable_coordinate_getters_support_numeric_and_string_use`
      continues to prove the typed getter facade typechecks. `cargo check`
      passes. The focused GenomicsDB gate moves from
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_typed_builder_products_v4`
      at `cargo_check_errors=29` to
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_locatable_concrete_facade_v1`
      at `cargo_check_errors=27`; the previous three `&Box<dyn Any>` missing
      `get_contig`, `get_start`, and `get_end` errors are gone. The remaining
      `InitializedQueryWrapper` error is now the separate
      `FeatureReader.query` overload/facade method-shape problem.
  - Borrowed concrete facade view coercion rule:
    - Ideal Rust Target: when a borrowed concrete value is passed to a
      read-only facade-supertype parameter, generated Rust should adapt at the
      slot boundary without cloning, downcasting, or introducing a
      constructor-specific rewrite. For example, a `Vec<SimpleInterval>.get(0)`
      flowing to `&Locatable` should become a borrowed facade view expression
      that preserves Java null/index checks and lets the callee keep its typed
      `&Locatable` signature.
    - Implementation Rule: `conversion_plan` treats a concrete-to-concrete
      read-only argument as a borrowed concrete facade coercion when hierarchy
      facts or known structural subtype facts prove the actual Java type is a
      subtype of the target Java type. `lower` uses constructor-signature
      fallback by constructed type and selected Rust constructor name when a
      `new` expression's resolver metadata is missing, so constructor
      arguments still receive the callee slot shapes. `coercion` renders the
      facade view as a zero-sized borrowed facade value while preserving
      evaluation of the actual Java value.
    - Required Facts: actual slot shape, target slot shape, concrete dispatch
      on both sides, shared-borrow target ownership, hierarchy/subtype or
      structural assignability fact, and constructor signature for shaped
      constructor argument lowering.
    - Owning Layer: `conversion_plan` owns the facade-view coercion decision;
      `lower` owns constructor signature fallback before argument lowering;
      `coercion` owns rendering the view expression; external packs and
      source facts own subtype evidence.
    - Fallback Shape: if subtype evidence is unavailable, the normal
      dispatch/ownership coercion path remains in force and any mismatch is
      left to structured conversion diagnostics or the compile gate. Broad
      dyn-trait promotion for `Locatable` remains deferred because it causes
      owned positions such as `Vec<Locatable>` to require a larger trait-object
      representation pass.
    - Evidence:
      `optional_borrowed_concrete_element_unwraps_to_concrete_facade_target`
      proves an optional borrowed concrete element can feed a borrowed
      concrete facade target without owned recovery, and
      `htsjdk_util_locatable_pack_strategy_preserves_typed_facade_boundary`
      keeps the HTSJDK pack strategy concrete. `cargo check` passes. The
      focused GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_concrete_facade_zst_view_v1`
      moves from `cargo_check_errors=24` to `23`; the previous
      `Option<&SimpleInterval>`/`&Locatable` mismatch is gone. A broad
      dyn-trait probe for `Locatable` regressed to `cargo_check_errors=83`
      because owned `Locatable` positions became invalid trait object values,
      so that strategy is intentionally not used for this slice.
  - External facade overloaded method arity naming rule:
    - Ideal Rust Target: Java external instance overloads should become stable
      Rust method names when Rust cannot overload by arity. For a
      `FeatureReader` facade, one-argument and three-argument Java `query`
      overloads should be emitted as `query_1(...)` and
      `query_3(chr, start, end)`, with the three-argument range query
      returning `CloseableTribbleIterator`.
    - Implementation Rule: `call_plan` owns external overloaded method naming
      from receiver type, Java method name, arity, and external API facts. If
      multiple external API arities exist for the same receiver/method family,
      fallback receiver and external-crate receiver targets use
      `{snake_method}_{arity}`. `stdlib` uses the same call-plan helper when
      emitting external facade support from external API facts. External packs
      must model overloaded methods with explicit arities and return types.
    - Required Facts: receiver Java type, method name, argument count,
      external API effect arities for that receiver/method family, and
      external API return types/nullability.
    - Owning Layer: `call_plan` owns call-target Rust names; `external_packs`
      owns library overload facts; `stdlib` owns facade support emission and
      consumes the call-plan naming helper; lowering only applies the selected
      target name.
    - Fallback Shape: methods with a single modeled arity keep the ordinary
      snake-case Rust name. If no external facts exist, fallback receiver calls
      keep the existing snake-case target and compile-gate pressure remains
      visible.
    - Evidence:
      `external_method_support_uses_arity_names_for_overloaded_facades`
      proves neutral external `query` overloads emit `query_1`/`query_3` and
      call the three-argument overload through `cursor.query_3(...)`.
      `htsjdk_tribble_feature_pack_models_coordinate_getters` proves the
      HTSJDK `FeatureReader.query` one- and three-argument facts are distinct,
      with `query/3` returning `CloseableTribbleIterator`. `cargo check`
      passes. The focused GenomicsDB gate moves from
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_locatable_concrete_facade_v1`
      at `cargo_check_errors=27` to
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_external_method_overload_arity_v1`
      at `cargo_check_errors=26`; the prior three-argument `query` arity and
      `CloseableIterator`/`CloseableTribbleIterator` mismatch is gone.
  - Struct literal direct-move field ordering rule:
    - Ideal Rust Target: constructor field initialization should not move a
      local into one field before another field initializer reads that local.
      For a Java constructor assigning `this.reader = reader` and
      `this.query = reader.query(...)`, the Rust struct literal should evaluate
      the query before moving the reader:
      `InitializedQueryWrapper { interval: ..., query: Some(reader.query_3(...)), reader }`.
    - Implementation Rule: `item_assembly` reorders only direct variable-move
      struct fields after sibling fields whose expressions read that variable.
      Structured `RustExpr::Var` and raw identifier moves are both recognized;
      raw sibling expressions are scanned by identifier token so generated
      fallback expressions participate in the same ordering rule.
    - Required Facts: assembled constructor field expressions and direct
      variable-move detection. No Java-specific method or class name is
      required.
    - Owning Layer: `item_assembly` owns final struct literal field ordering;
      constructor planning still owns field-value selection; lowering still
      owns expression lowering for each field.
    - Fallback Shape: if the moved value is not referenced by any sibling
      field expression, field order is preserved. Non-direct move expressions
      are not reordered, avoiding changes to side-effect evaluation order.
    - Evidence:
      `item_assembly_owns_struct_impl_and_function_shells_from_signatures`
      now covers structured and raw direct-move fields being delayed after
      sibling reads. `cargo check` passes. The focused GenomicsDB gate moves
      from
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_external_method_overload_arity_v1`
      at `cargo_check_errors=26` to
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_external_method_overload_field_order_v2`
      at `cargo_check_errors=25`; the `borrow of moved value: reader`
      residual is gone, and generated `InitializedQueryWrapper` now places
      `query: Some(reader.query_3(...))` before `reader: reader`.
  - Receiverless current-type call binding in lambdas:
    - Ideal Rust Target: a receiverless Java helper call inside an instance
      lambda should bind to the current Rust receiver when the current type has
      a matching method. For example, `submit(() -> helper(value))` inside an
      instance method should render `submit(|| { self.helper(value.as_str()) })`,
      not a free `helper(value)` call.
    - Implementation Rule: unresolved receiverless calls use the current
      type's signature plan as a fallback. When such a fallback signature is
      found, `call_plan` rewrites the fallback free target to an owned
      `FallbackSelf` target and `call_lowering` renders it as
      `self.{method}(...)`. Argument shape fallback continues to use the same
      signature plan, so borrowed string adapters and other slot adapters are
      still selected by the signature layer.
    - Required Facts: current type, current signature plan, Java method name,
      argument count, and unresolved receiverless call shape.
    - Owning Layer: `lower` owns applying current-callable fallback signature
      context; `call_plan` owns the `FallbackSelf` target shape;
      `call_lowering` owns rendering that target. Lambda lowering only
      preserves the current lowering context.
    - Fallback Shape: if no current-type signature matches, keep the existing
      fallback free call and let the compile gate expose the missing binding.
    - Evidence:
      `executor_submit_lambda_receiverless_helper_call_uses_current_self`
      proves receiverless helper calls inside executor block lambdas render
      through `self.helper(...)`. `cargo check` passes. The focused GenomicsDB
      gate moves from
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_external_method_overload_field_order_v2`
      at `cargo_check_errors=25` to
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_lambda_self_receiver_v1`
      at `cargo_check_errors=24`; the previous `cannot find function
      get_reader_from_path` residual is gone.
  - Unshaped throwing argument propagation rule:
    - Ideal Rust Target: a Java call that can throw and appears as an
      argument to another expression should still propagate through `?` when
      the surrounding Rust function returns `Result`. For example,
      `Wrapper::new(self.open(path)?, path)` is the target, not
      `Wrapper::new(self.open(path), path)`.
    - Implementation Rule: `lower_unshaped_argument` applies the same
      `slot_expr_needs_result_propagation` check as slot-shaped expression
      lowering. `plan_call_exception_propagation` also recognizes unresolved
      receiverless calls to the current type when method-effect facts prove
      the method throws.
    - Required Facts: current type, method-effect thrown facts, unresolved or
      resolved call name, argument count, and enclosing `Result` lowering
      context.
    - Owning Layer: `call_plan` owns exception-propagation classification;
      `lower` owns applying `?` to unshaped argument expressions; exception
      planning still owns method `Result` shells.
    - Fallback Shape: calls with no resolved, external, or current-type
      thrown fact are left unchanged. Non-`Result` context remains exposed by
      the compile gate or exception diagnostics.
    - Evidence:
      `unshaped_constructor_argument_call_that_throws_propagates_result`
      proves nested constructor arguments propagate `self.open(...) ?` through
      the unshaped path, and
      `exception_lowering_policy_owns_result_support_ok_and_err_shapes`
      covers receiverless current-type exception classification.
      `cargo check` passes. The focused GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_lambda_self_receiver_result_arg_v2`
      remains at `cargo_check_errors=24`, but generated
      `InitializedQueryWrapper::new(self.get_reader_from_path(...)? , ...)`
      now has the required `?`. The remaining error on that expression is the
      separate `Option<&SimpleInterval>` to `&Locatable` interface/collection
      element adaptation problem.
  - Emitted subtype source-summary super method surface rule:
    - Ideal Rust Target: when translated Java source extends a superclass that
      is present only as source-summary context, inherited methods used by the
      translated subtype should be callable directly on the emitted Rust type.
      For example, a translated `GenomicsDBImport extends GATKTool` should
      expose
      `get_default_tool_v_c_f_header_lines(&self) -> HashSet<VCFHeaderLine>`
      on `GenomicsDBImport`, because Rust will not inherit inherent methods
      from an opaque `GATKTool` shell.
    - Implementation Rule: external facade support walks emitted type
      superclass chains until they cross into source-summary types. For used
      method names/arities found in that source-summary hierarchy, it emits an
      inherent facade method on the emitted subtype unless the translated
      source already declares or inherits that method locally.
    - Required Facts: emitted type hierarchy, source-summary type hierarchy,
      used call name/arity, Java method return/parameter types, and local
      method override detection.
    - Owning Layer: `stdlib` owns support-surface emission for source-summary
      boundaries. The source-summary parser owns the Java method signatures;
      lowering only calls the method selected by normal call planning.
    - Fallback Shape: if the summary superclass is unavailable, if the method
      is unused, or if the emitted local hierarchy already provides the method,
      no extra subtype surface is emitted. The compile gate exposes any
      remaining missing inherited surface.
    - Evidence:
      `emitted_type_extending_source_summary_super_gets_inherited_method_surface`
      proves an emitted subtype gets a concrete inherited summary method
      surface. The focused GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_emitted_summary_super_methods_v1`
      moves from `cargo_check_errors=42` to `40`; the two previous
      `get_default_tool_v_c_f_header_lines` method-not-found errors are gone.
  - Native collection extend self-read temp rule:
    - Ideal Rust Target: when Java `field.addAll(source())` lowers to Rust and
      the target is a mutable `self` field while the source expression reads
      `self`, the source should be evaluated before the mutable borrow:
      `let tmp = self.source(); self.field.as_mut().unwrap().extend(tmp);`.
      This preserves Java evaluation order and satisfies Rust's borrow rules.
    - Implementation Rule: native collection `addAll` statement lowering
      detects mutable self-field receivers whose source expression reads
      `self`. It emits a temporary for the lowered extend source before
      rendering the mutable receiver `.extend(...)` call.
    - Required Facts: native collection method plan, receiver expression,
      field membership from the shape environment, and source-expression
      self-read detection.
    - Owning Layer: `lower` owns statement-level evaluation ordering for
      native collection operations. Signature and source-summary support only
      determine the method/slot shapes.
    - Fallback Shape: if the receiver is not a self field, or if the source
      does not read `self`, the existing single-expression `.extend(...)`
      lowering is preserved.
    - Evidence:
      `emitted_type_extending_source_summary_super_gets_inherited_method_surface`
      cargo-checks the generated Rust for the inherited `addAll` use. In the
      focused GenomicsDB gate, the two merged-header extension sites now emit
      `let __java2rust_add_all_source =
      self.get_default_tool_v_c_f_header_lines();` before mutably extending
      `self.merged_header_lines`.
  - Source-summary `super` boundary call rule:
    - Ideal Rust Target: Java `super.method(...)` against a superclass that is
      present only as source-summary/support context must not render as raw
      Rust `super.method(...)`, because Rust has no corresponding inherited
      object receiver and the superclass body is intentionally not emitted. A
      throwing void hook should become a no-op `Result` boundary such as
      `Ok(())?;`; an inherited value query in a contextual return slot may use
      `Default::default()` as the explicit external-boundary fallback.
    - Implementation Rule: `call_plan` selects a distinct `SuperBoundary`
      target for resolved instance calls whose receiver is `Expr::Super`.
      `call_lowering` renders that boundary from the resolved Java return type
      and method-effect thrown-exception facts, instead of lowering the
      receiver expression. Resolver facts still treat `super` as current-type
      hierarchy pressure; only the Rust call target changes.
    - Required Facts: resolved declaring type, resolved Rust method name,
      Java return type, thrown-exception method effect, and receiver expression
      identity.
    - Owning Layer: `resolve` owns method resolution; `call_plan` owns the
      `super` target classification; `call_lowering` owns fallback Rust
      expression spelling. `expr_render` must not decide `super` call syntax.
    - Fallback Shape: if a `super` call is unresolved, existing unresolved
      target diagnostics/fallbacks remain visible. The boundary rule does not
      guess a real Rust superclass dispatch.
    - Evidence:
      `source_summary_super_calls_lower_to_boundary_defaults_not_raw_super_receiver`
      covers both throwing void and value-returning summary-super calls, and
      `call_plan_owns_call_and_new_target_selection_before_argument_lowering`
      asserts the `SuperBoundary` target shape. The focused GenomicsDB gate at
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_super_boundary_mutability_v1`
      has no `expected value, found module super` diagnostics.

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
  - Same-receiver mutability propagation rule:
    - Ideal Rust Target: a Java method that calls a mutating helper on the
      same receiver should render with `&mut self` even when the method body
      does not assign fields directly. For example, `outer() { middle(); }`,
      `middle() { this.inner(); }`, and `inner() { count++; }` should make all
      three Rust methods mutable receivers, so callers do not hit late borrow
      checker errors.
    - Implementation Rule: `usage` records whether each resolved call site is
      an implicit/`this`/`super` same-receiver call, then runs a finite
      fixed-point over Java-name and Rust-name method usage maps. If a
      same-receiver call reaches a method already known to mutate `self`, the
      caller is marked `mutates_self`; signature solving then consumes the
      normal receiver-mutability fact path.
    - Required Facts: call receiver identity, resolved declaring type,
      resolved Rust method name, direct field mutation facts, and method usage
      maps keyed by Java and Rust names.
    - Owning Layer: `usage` owns mutability propagation; `semantic_analysis`
      exposes method receiver effects; `signature` turns those effects into
      `ReceiverPlan::Mutable`; lowering only consumes solved receiver plans.
    - Fallback Shape: unresolved or non-self receiver calls do not propagate
      receiver mutability. They remain governed by external API mutability
      facts or conservative slot policy.
    - Evidence:
      `usage_propagates_self_mutability_through_same_receiver_calls` covers a
      neutral transitive self-call chain. The focused GenomicsDB gate at
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_super_boundary_mutability_v1`
      emits `pub fn on_startup(&mut self)` and has no
      `cannot borrow *self as mutable` diagnostics.

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
  - Direct null call-argument rule:
    - Ideal Rust Target: when Java passes literal `null` to a resolved
      reference-typed method or same-class constructor parameter, the resolved
      callee slot should be nullable at the signature level. For example,
      `target(path, null)` should solve the second parameter as `Option<Path>`,
      and `this(null)` should solve the delegated constructor parameter as
      `Option<T>`. Lowering should then render ordinary `None` at the call
      site without a per-call special case.
    - Implementation Rule: `usage` records literal-null argument indexes on
      call-site facts. The signature propagation sweep consumes those indexes
      and adds `RequiredStorageNullability(MaybeNull)` to the resolved method
      parameter or same-class delegated constructor parameter. This propagation
      is independent of binding-use propagation because literal `null` has no
      source binding node.
    - Required Facts: resolved method/constructor target, argument index,
      literal-null actual, call-site boundary, and callee parameter inventory.
    - Owning Layer: `usage` owns null-argument observation; `signature` owns
      nullable parameter constraints and slot solving; conversion/lowering only
      render the solved `Option<T>` shape.
    - Fallback Shape: unresolved external calls keep existing unknown-boundary
      behavior. Ordinary `super(...)` constructor delegation remains under the
      constructor-inheritance plan and is not guessed by this rule.
    - Evidence:
      `direct_null_call_argument_marks_resolved_method_parameter_nullable`,
      `direct_null_constructor_delegation_marks_resolved_constructor_parameter_nullable`,
      and the existing `ternary_null_test_marks_overloaded_path_parameter_nullable`
      cover direct-null and null-tested parameter pressure. The focused
      GenomicsDB gate at
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_null_arg_signature_v1`
      moves from `cargo_check_errors=54` to `52`; the previous
      `None`-to-`GenomicsDBOptions` and `None`-to-`Path` mismatches are gone.

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
    `static_external_facade_uses_nullable_argument_fact_without_call_site_type_hack`,
    `htsjdk_tribble_feature_pack_models_coordinate_getters`,
    `external_api_user_override_beats_later_name_guess`,
    `external_api_receiver_specific_override_beats_global_override_in_signature`,
    `external_collection_api_effects_are_receiver_specific_facts`,
    and `external_map_api_effects_distinguish_read_and_write_methods`.
    `call_plan` now applies `parameter_nullability` when converting external
    argument effects into slot shapes, and external facade support renders
    concrete argument positions from the same facts while leaving unmodeled
    positions generic. The GenomicsDB focused gate
    `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_nullable_external_arg_v1`
    moves from `cargo_check_errors=10` to `9`; the former
    `AbstractFeatureReader::get_feature_reader(..., None::<T>, ...)` inference
    error is gone, and the generated facade has
    `_arg1: Option<String>` with call sites passing plain `None` or the
    concrete index URI.
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
	    Native collection receiver mutability now also flows through lambda
	    captures. If Java captures a `final SortedMap<K,V>` in a lambda and calls
	    `put`, Rust still needs `let mut` for the local collection binding because
	    Java `final` forbids rebinding, not content mutation. `usage` analyzes
	    lambda bodies with scoped lambda parameters, `stdlib` owns the native
	    collection mutator list, and `signature` converts captured receiver
	    mutation facts into `MutableBinding` for the local plan.
	    Coverage includes
	    `signature_plan_marks_final_native_map_receiver_mutation_local_mutable`
	    and `FinalSortedMapReceiverMutation.java`. The focused GenomicsDB gate at
	    `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_lambda_capture_map_mut_v1`
	    moves from `cargo_check_errors=23` to `21`, emits
	    `let mut sample_to_reader_map`, removes the `E0596` bucket, and also
	    removes the previous moved `variant_path` bucket exposed by callback usage.
	    Java collection and map `size()` receiver calls now render as
	    `.len() as i32` in `call_lowering`, preserving Java `int` return width
	    instead of leaking Rust `usize` into public Java-shaped slots. Coverage
    includes `MapSizeReturnInt.java`, `CollectionAddAllAndSize.java`, and
    `constructor_initialized_sorted_map_field_is_not_optional_carrier`. The
    focused GenomicsDB gate for this slice reports
    `cargo_check_errors=110`, down from `111`, with zero tracked invalid
    patterns.
    Native `Arrays.sort` statements are now planned as mutating Rust sequence
    operations:
    - Ideal Rust Target: Java `Arrays.sort(values);` should mutate the Rust
      sequence in place:

      ```text
      let mut values = vec![2, 1];
      values.sort();
      ```

      It should not render as `Arrays::sort(values.clone())`, because Rust has
      no Java `Arrays` namespace and sorting a clone loses Java's mutation
      semantics. The sorted binding must be mutable when it is a local.
    - Implementation Rule: `call_plan` recognizes one-argument
      `Arrays.sort` as a `CallStatementPlan::NativeArraysSort` before generic
      static-owner fallback. `lower` renders the planned target as
      `.sort()`. `signature` treats the first argument of `Arrays.sort` as an
      assignment target for local binding mutability, so local declarations
      receive `let mut` through the normal local plan.
    - Required Facts: unshadowed `Arrays` static receiver, method name `sort`,
      single mutable sequence argument, and local binding mutability for a
      local target.
    - Owning Layer: `call_plan` owns static collection statement
      classification; `signature` owns local mutability; `lower` renders the
      planned statement. Argument lowering does not decide to clone or consume
      the target.
    - Fallback Shape: comparator and range overloads remain outside this rule
      until they have explicit ordering/comparator plans. Non-`Ord` element
      types should later select `sort_by` or a structured unsupported
      diagnostic rather than silently using `.sort()`.
    - Evidence:
      `arrays_sort_statement_lowers_to_mutating_rust_sort` proves neutral
      translation uses `values.sort();`;
      `signature_plan_marks_arrays_sort_target_local_mutable` proves the
      sorted local is declared mutable; and the focused GenomicsDB gate moves
      from
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_conventional_static_strings_v1`
      at `cargo_check_errors=36` to
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_arrays_sort_mutability_v1`
      at `cargo_check_errors=35`. Generated `GenomicsDBImport` now contains
      `let mut partitions: Vec<String>` followed by `partitions.sort();`.
    Domain collection facades and external enum constants now have separate
    support responsibilities:
    - Ideal Rust Target: Java domain collection types such as
      `htsjdk.samtools.util.IntervalList` should remain domain facades when
      their Rust representation has not been proven to be a native collection.
      `new IntervalList(dictionary)` should render as
      `IntervalList::new(dictionary)`, and receiver calls such as
      `intervals.add(interval)` and `intervals.write(path)` should stay on the
      facade surface. Java enum-like static constants such as
      `PairOrientation.FR`, `LogLevel.INFO`, and
      `FileTermination.HAS_TERMINATOR_BLOCK` should be typed tokens or
      associated constants, not recursive Rust struct storage. In particular,
      Rust must not emit `pub struct PairOrientation { pub f_r:
      PairOrientation }`.
    - Implementation Rule: `stdlib` now distinguishes collection-like domain
      facades from native Rust collection constructors and receiver calls via
      native-only collection helpers. `call_plan` uses those helpers for
      native collection lowering, so `IntervalList` no longer becomes a
      native `Vec` constructor or `.push()` target. External facade support
      still consumes external-pack field facts for typing, but skips
      self-typed enum-like static constants when constructing facade struct
      fields, preventing recursive `struct` shells while preserving the
      field facts for resolver/type decisions.
    - Required Facts: concrete target-slot type for facade constructor
      rendering, receiver type evidence for non-native domain collection
      calls, and external-pack field facts for enum-like constants. A
      self-typed uppercase field on the same external receiver is treated as a
      static enum constant rather than instance storage.
    - Owning Layer: `stdlib` owns native-vs-domain collection classification
      and external facade shell construction; `call_plan` owns native
      collection call/constructor selection; `lower` owns slot-aware concrete
      external constructor rendering. External-pack facts describe available
      surfaces but do not decide native collection lowering.
    - Fallback Shape: if the concrete domain facade representation cannot be
      proven, the value may still fall back to erased `Box<dyn Any>` at
      boundary slots. That fallback is allowed, but it must not reintroduce
      recursive facade structs or native collection methods on domain types.
    - Evidence:
      `domain_collection_facade_constructor_is_not_native_collection_constructor`
      proves `IntervalList::new(dictionary)`, `intervals.add(...)`, and
      `intervals.write(...)` are emitted without `intervals.push(...)`.
      `external_enum_constant_facades_do_not_emit_recursive_storage_fields`
      proves pack-owned enum constants do not generate recursive
      `PairOrientation` storage. The focused GenomicsDB gate with
      `com.intel.genomicsdb` and `htsjdk.samtools.util` packs moves from
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_domain_collection_facade_htsjdk_v1`
      at `cargo_check_errors=94` to
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_enum_constant_facade_v1`
      at `cargo_check_errors=40`; the prior `E0072`/`E0391` recursive
      `PairOrientation`, `LogLevel`, and `FileTermination` buckets are gone.
    External generic facade arguments now have their own slot rule:
    - Ideal Rust Target: a concrete final domain facade method whose external
      surface is modeled as generic should accept typed Rust arguments without
      forcing them through `Box<dyn Any>`. For example, Java
      `outputList.add(new Interval(...))` on `IntervalList` should render as
      `output_list.add(Interval::new_...(...));`, and Java enhanced-for
      values passed to that facade should remain ordinary typed or borrowed
      Rust values such as `output_list.add(Interval::new_1(i));`. The generic
      facade method itself already abstracts over `T0`; argument lowering
      should not erase the value before calling it.
    - Implementation Rule: `lower` detects fallback/external calls whose
      receiver is a concrete final collection-like domain facade and whose
      argument plan has unknown placeholder slots. In that context, unknown
      argument slots are treated as generic facade parameters, so each
      argument is lowered against its own inferred actual shape. Explicitly
      modeled external parameter types still use their fact-owned slot shapes;
      true erased storage boundaries remain represented through the normal
      dispatch/coercion plan.
    - Required Facts: receiver expression type, final-type facts,
      collection-like domain facade classification, inferred argument shape,
      constructor facts for concrete external values, and explicit external
      argument facts when the API really owns a parameter type.
    - Owning Layer: `call_plan` owns external argument facts and native
      collection exclusion; `lower` owns the generic-facade call-site
      adaptation because it has both the chosen call target and the concrete
      argument expression. `coercion` still owns real erased-object conversion.
    - Fallback Shape: if the receiver cannot be proven to be a concrete final
      domain facade, or if the external pack states an explicit erased/object
      parameter, the existing erased-boundary behavior remains available.
    - Evidence:
      `pack_enabled_final_domain_facade_local_stays_concrete_not_erased`
      now proves `IntervalList.add(new Interval(...))` emits
      `intervals.add(Interval::new_3(...))` rather than boxing a tuple as
      `Box<dyn Any>`. The focused GenomicsDB gate with `com.intel.genomicsdb`
      and `htsjdk.samtools.util` packs moves from
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_final_domain_collection_facade_v1`
      at `cargo_check_errors=40` to
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_final_domain_collection_arg_v1`
      at `cargo_check_errors=39`; the prior `lifetime may not live long
      enough` bucket caused by `output_list.add(Box::new(i) as Box<dyn Any>)`
      is gone, and generated `GenomicsDBImport` now emits
      `output_list.add(Interval::new_1(i));`.
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
	    Indexed reads now have their own borrowed-element ownership rule:
	    - Ideal Rust Target: Java `values[i]` over a Rust `Vec<String>` or
	      translated Java `String[]` should be treated as a borrowed element read.
	      If the target slot is owned `String`, render `values[i].clone()`. If the
	      target slot is `Option<&str>`, render `Some(values[i].as_str())`. If the
	      target slot is a borrowed receiver such as `.trim()`, keep the indexed
	      receiver borrowed and do not clone.
	    - Implementation Rule: `shape_projection::indexed_read_shape` projects
	      non-copy indexed read elements as `SharedBorrow` regardless of container
	      ownership; copy-capable elements still project as `CopyValue`.
	      `lower_array_access_expr_for_slot` mirrors the existing `List.get`
	      slot path: it renders the indexed expression once, marks the actual as a
	      borrowed sequence element, and delegates owned clone, string-view, and
	      optional wrapping decisions to `conversion_plan`/`coercion`. The string
	      view policy now also handles borrowed native `String` to `&str`, not only
	      owned `String` to `&str`.
	    - Required Facts: indexed receiver sequence element type, copy capability,
	      target slot ownership/view/nullability, and existing type capability
	      overrides for value-semantics external elements.
	    - Owning Layer: `shape_projection` owns indexed-read shape; `lower` owns
	      connecting array-access syntax to the slot conversion boundary;
	      `conversion_plan` owns borrowed-to-owned clone and borrowed-string-view
	      adaptation; `expr_render` only renders `target[index]`.
	    - Fallback Shape: if the receiver is not sequence-like, retain ordinary
	      expression lowering and let existing diagnostics expose the unresolved
	      shape.
	    - Evidence:
	      `indexed_shape_projects_owned_container_non_copy_reads_as_borrows`,
	      `borrowed_native_string_can_coerce_to_string_view`, and
	      `indexed_string_array_reads_clone_only_for_owned_slots` pass. The focused
	      GenomicsDB gate moves from
	      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_try_ok_arm_v1` at
	      `cargo_check_errors=25` to
	      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_indexed_string_borrow_v1`
	      at `cargo_check_errors=23`; the previous two `cannot move out of index
	      of Vec<String>` errors are gone. Generated `SampleNameMap` now emits
	      `Some(split[0 as usize].clone())` for the owned optional sample-name
	      slot and `let sample: String = split[0 as usize].clone();`.
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
    A later `submit`/`Future` result-slot refresh propagates the expected
    call-result slot into value-returning callback arguments. For
    `ExecutorService.submit(...)` in a `Future<T>` result slot, the ideal Rust
    target is a closure whose local return and nested try/catch `Ok` carrier
    are shaped as `T`, while the surrounding `submit` call returns the
    `Future` facade. The implementation rule is split: `lower` derives the
    callback return slot from the shaped `Future<T>` call result before generic
    external-call lowering, and `stdlib` emits
    `ExecutorService::submit<T0>(&self, T0) -> Future` from the external API
    support surface. The slice is covered by
    `executor_submit_future_slot_shapes_block_lambda_result`; the focused
    GenomicsDB gate drops from 23 to 21 compile errors. Remaining callback
    failures are true exception-carrier issues. A follow-up result-carrier
    refresh lowers throwing value-returning `submit` callbacks as
    `Result<T, JavaException>` closures, so normal returns become `Ok(value)`,
    catch arms return `Err(...)`, and uncaught Java exceptions return
    `Err(ex)`. The same regression now checks `Ok(value) => return Ok(value)`
    and `return Err(...)`; the focused GenomicsDB gate drops from 21 to 20
    compile errors. Remaining executor-adjacent failures are now separate
    rules: optional `Path` assignment for `getVCFIndexForSampleAsPath`, and
    later `Future.get()` catch propagation from a method whose signature is not
    yet `Result<..., JavaException>`.
    A follow-up callback-local optional shape refresh keeps callback locals
    initialized from nullable planned method calls as `Option<T>` when the
    signature solver has no callback-local plan. The ideal Rust target is
    `let variant_index_path: Option<Path> = ...get_v_c_f_index_for_sample_as_path(...)`
    followed by passing `variant_index_path` directly to an `Option<Path>`
    parameter, not `Some(variant_index_path)` and not an invalid `Path` local.
    The implementation rule is owned by `lower`: fallback lambda-local shape
    planning asks the current signature plan for receiverless and receiver
    method-call return shapes, and slot conversion uses those planned shapes
    before optional wrap/unwrap decisions. The regression
    `nullable_method_result_in_callback_local_stays_optional_for_optional_arg_slot`
    covers the neutral case; the focused GenomicsDB gate drops from 20 to 19
    compile errors.

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
    Existing opaque facade owners now receive static string constants through
    additional impl blocks instead of duplicate support structs:
    - Ideal Rust Target: if support generation already emitted
      `pub struct ExternalConstants;` for an opaque external or source-summary
      facade, a later static string constant surface should extend that type as
      `impl ExternalConstants { pub const NAME: &'static str = "NAME"; }`. It
      should not emit a second `pub struct ExternalConstants;`.
    - Implementation Rule: `lower` assembles opaque facade support before
      static string constant support, asks `stdlib` for the type names declared
      by those support items, and passes that set into static-constant support
      generation. Static-constant support emits only an impl block for owners
      already declared by prior support, while preserving the old struct+impl
      fallback for standalone constant owners.
    - Required Facts: current support item inventory, generated facade owner
      names, static constant owner and field names, local static string values,
      and Rust identifier validity.
    - Owning Layer: `lower` owns support-item assembly order and passes the
      declared-owner inventory; `stdlib` owns opaque facade and static-constant
      support item rendering. Resolver, semantic facts, and ordinary lowering
      do not special-case duplicate type names.
    - Fallback Shape: if no prior support item declares the owner,
      static-constant support still emits the conservative `pub struct Owner;`
      plus impl block. If a prior support item declares the owner, the static
      constant support item must be impl-only.
    - Evidence:
      `static_string_constant_support_extends_existing_opaque_facade_owner`
      passes. `cargo check --quiet` passes. The focused GenomicsDB gate moves
      from `cargo_check_errors=20` to `19` at
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_static_const_facade_owner_v2`;
      the previous `E0428` duplicate
      `GenotypeCalculationArgumentCollection` row is gone, and generated
      `00005_GenomicsDBOptions.rs` now has one `pub struct
      GenotypeCalculationArgumentCollection;` plus separate constructor and
      static-constant impl blocks.
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
  - Static logger field rule:
    - Ideal Rust Target: Java logging calls through a static logger field such
      as `private static final Logger logger = LogManager.getLogger(T.class);`
      should lower to the same no-op logging facade as inherited or external
      logger calls: `logger::info(message)`, `logger::warn(message)`,
      `logger::debug(message)`, `logger::error(message)`, or
      `logger::trace(message)`. The logger field itself should not force an
      emitted Rust instance field, associated const, external crate, or
      unresolved `Self::logger` value when the only observable behavior is a
      logging side effect.
    - Implementation Rule: `call_plan` recognizes receiver calls whose
      receiver canonicalizes to `logger`/`Logger` and whose method is a known
      Java logging side-effect method before fallback receiver lowering. It
      plans those calls as `CallTargetPlan::StaticOwner { owner: "logger",
      rust_name }`, so `call_lowering` renders the existing facade call. The
      rule is selected from receiver/method capability, not from GATK class
      names or rendered `Self::logger` text.
    - Required Facts: receiver expression name, canonical receiver key,
      logging method name, positive argument arity, and platform-facade support
      detection for logger calls.
    - Owning Layer: `call_plan` owns logger facade target selection;
      `call_lowering` only renders the selected static-owner call; `stdlib`
      owns the no-op logger facade support item and support detection.
    - Fallback Shape: non-logging calls on `logger` or unknown logging-like
      receivers continue through ordinary resolved/fallback receiver planning.
    - Evidence:
      `java_logging_facade_supports_static_logger_field_calls`,
      `java_logging_facade_supports_inherited_logger_calls`, and the focused
      GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_logger_v1`, where
      `cargo_check_errors` moved from `84` to `63` and the
      `Self::logger`/missing associated item bucket disappeared.
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
  - Focused Gate Update: conventionally named external static string constants
    now become typed field facts even when no dependency pack or selected
    source-summary package supplies the literal declaration.
    - Ideal Rust Target: Java dependency constants such as
      `CommandNames.INPUT_LONG_NAME` and `ExternalDelimiters.FIELD_DELIM`
      should become ordinary Rust associated constants with borrowed string
      shape:

      ```text
      CommandNames::INPUT_LONG_NAME
      ExternalDelimiters::FIELD_DELIM
      ```

      Those constants should be available through the same support facade as
      explicitly modeled external `String` fields. A hand-written Rust boundary
      would use `pub const FIELD: &'static str`, then rely on normal slot
      conversion if an owned `String` is required.
    - Implementation Rule: `stdlib::known_external_field_type_for_receiver`
      treats unresolved external all-caps fields with conventionally string
      suffixes, including `_KEY`, `_FIELD`, `_NAME`, `_LONG_NAME`,
      `_SHORT_NAME`, `_DELIM`, `_DELIMITER`, `_SEPARATOR`, `_PREFIX`,
      `_SUFFIX`, `_REGEX`, `_PATTERN`, `_EXTENSION`, `_PATH`, `_URI`, and
      `_URL`, as `JavaType::String` after numeric, boolean, explicit pack, and
      more specific built-in field models have had priority. Existing
      member-access planning and support emission consume that typed fact; no
      call-site or GATK-name-specific lowering rule is added.
    - Required Facts: static/type receiver shape, all-caps Java constant
      spelling, no local or field shadowing of the receiver, and absence of a
      stronger explicit field type fact.
    - Owning Layer: `stdlib` owns the conservative external field type
      heuristic; `member_access_plan` owns borrowed static string shape;
      `external_static_string_constant_support_items` owns facade emission;
      lowering only renders the planned associated-constant access.
    - Fallback Shape: constants without known types and without string-like
      names remain unknown. If a dependency pack later supplies a precise field
      type or literal value, that fact should override the convention.
    - Evidence:
      `conventional_external_static_string_constants_are_typed_and_supported`
      proves neutral `_LONG_NAME` and `_DELIM` owners emit support facades.
      The focused GenomicsDB gate moves from
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_emitted_summary_super_methods_v1`
      at `cargo_check_errors=40` to
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_conventional_static_strings_v1`
      at `cargo_check_errors=36`; the unresolved
      `StandardArgumentDefinitions` and `AnnotationUtils` owner buckets are
      gone.
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
  - Future-Family Generic Instance Facade Slice:
    - Ideal Rust Target: Java future-like dependency values should expose
      `get()` as an instance method whose Rust result type is inferred from
      the assignment, return, argument, or local target slot:
      `let reader: FeatureReader = future.get();`. The facade should therefore
      render `pub fn get<T: Default>(&self) -> T`, not an associated function
      such as `Future::get::<T>()`. Imported simple aliases and fully qualified
      facade names should share the same instance surface.
    - Implementation Rule: `stdlib` treats opaque facade owners ending in
      `Future` and a used zero-arity `get` call as a known generic instance
      surface. This rule runs in generated facade support and replaces any
      preexisting same-signature generic static fallback before inserting the
      instance method, so imported-static fallback collection cannot freeze the
      wrong ownership shape. Lowering remains a consumer: it continues to emit
      normal instance calls and lets the Rust target slot infer `T`.
    - Required Facts: used call name and arity, opaque facade owner names,
      imported type aliases, and the target slot type at the call site.
    - Owning Layer: `stdlib` owns generated facade method shape and replacement
      ordering. `call_plan`/`lower` own only the fact that the Java syntax is an
      instance call. Slot/type inference owns the concrete `T` chosen by Rust.
    - Fallback Shape: if the owner is not known future-like, keep the ordinary
      external facade fallback or require an external API fact. Do not add a
      call-site rewrite for `find/get`-like names, because ownership and
      generic result inference should be centralized in the support surface.
    - Evidence:
      `selected_opaque_future_get_is_generic_instance_surface` covers both
      `Future` and `java.util.concurrent.Future` facade aliases and rejects the
      old `pub fn get<R: Default>() -> R` static fallback. The focused
      GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_future_get_instance_v2`
      moves from `cargo_check_errors=37` to `36`; the prior `no method named
      get` bucket is gone, and generated `Future` plus
      `JavaUtilConcurrentFuture` support both render
      `pub fn get<T: Default>(&self) -> T`.
  - Numeric relational promotion rule:
    - Ideal Rust Target: Java numeric relational comparisons should compare
      operands at Java's promoted primitive type before emitting Rust. For an
      `int` getter compared to a `long` local, the Rust target is:
      `((interval.get_end() as i64) < contig_length)`. The cast must be
      parenthesized so Rust parses `<` as a comparison, not as generic
      arguments on the cast target.
    - Implementation Rule: `operator_lowering` applies the same primitive
      promotion plan to ordered relational operators (`<`, `<=`, `>`, `>=`)
      that it already applies to arithmetic and bitwise numeric operators.
      `coercion` renders primitive widening as a parenthesized cast expression
      so binary-expression rendering can compose it safely in any operator
      context.
    - Required Facts: inferred Java operand primitive types and the Java
      binary operator. No package, method-name, or GATK-specific facts are
      required.
    - Owning Layer: `types` owns operand type inference;
      `operator_lowering` owns numeric promotion choice for Java binary
      operators; `coercion` owns cast spelling. Lowering only wires the
      inferred operands and selected operator plan together.
    - Fallback Shape: nonnumeric or unknown ordered comparisons continue
      through the existing ordered-comparison fallback, including the native
      collection length cast rule. Missing type facts should surface as a
      focused type-inference gap rather than a call-site-specific cast.
    - Evidence:
      `operator_lowering_owns_binary_and_unary_rust_spelling` now proves
      `int < long` selects `Long` promotion and renders `(end as i64) < limit`.
      `translates_java_numeric_relational_promotion_before_rust_comparison`
      proves a neutral Java snippet emits `return ((end as i64) < limit);`.
      The focused GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_numeric_relational_promotion_v2`
      moves from `cargo_check_errors=36` to `35` relative to the comparable
      future/facade gate and drops the latest `E0308` bucket from `17` to
      `16`; the prior `interval.get_end() < contig_length` `i32`/`i64`
      mismatch is gone with no replacement Rust parse error.
  - Ternary null-refined receiver adaptation rule:
    - Ideal Rust Target: a Java null-guarded ternary branch should treat the
      tested local as non-null inside the non-null branch before receiver
      adaptation. For
      `path == null ? null : path.toAbsolutePath().toUri().toString()`, the
      Rust target is
      `if path.is_none() { None } else { Some(path.unwrap().to_absolute_path().to_uri().to_string()) }`.
      It must not emit `path.unwrap().as_ref().unwrap()`, because the branch
      refinement has already consumed the optional carrier.
    - Implementation Rule: call receiver adaptation now consumes
      `ExprRefinements`. When the receiver expression is a local marked
      non-null by the ternary null-refinement plan, optional instance receiver
      adaptation is skipped. The existing lowerer still owns the actual
      `unwrap()` spelling for the refined bare local, and ordinary optional
      calls outside a proven branch still adapt through `as_ref()`/`as_mut()`.
    - Required Facts: ternary condition shape, tested local name, branch
      polarity, receiver expression, and call target optional-adapter policy.
    - Owning Layer: `nullability_plan` owns branch refinement selection;
      `lower` owns threading refinements into call receiver adaptation; call
      planning and external packs do not special-case `Path` or any GATK API.
    - Fallback Shape: if the condition is not a recognized local/null
      comparison, receiver adaptation remains unchanged and optional receivers
      are still unwrapped by the normal call-target policy.
    - Evidence:
      `nullable_ternary_call_receiver_uses_branch_refinement_before_optional_adapter`
      proves a neutral `java.nio.file.Path` ternary emits
      `path.unwrap().to_absolute_path().to_uri().to_string()` and rejects
      `path.unwrap().as_ref().unwrap()`. The focused GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_ternary_refined_receiver_v1`
      moves from `cargo_check_errors=35` to `34`; the previous
      `no method named as_ref found for struct Path` top-error row is gone,
      and generated `get_reader_from_path` now emits
      `variant_index_path.unwrap().to_absolute_path().to_uri().to_string()`.
  - Stream block-lambda prefix local receiver rule:
    - Ideal Rust Target: block-lambda locals declared before the final return
      expression should be visible to that return expression. A Java stream
      body such as
      `map(x -> { Builder builder = Owner.newBuilder(); ...; return builder.build(); })`
      should emit `builder.build()`, not `Builder::build()` or
      `OwnerBuilder::build()`. Local receiver identity must survive the block
      prefix.
    - Implementation Rule: `stream_lowering` now renders a block-lambda prefix
      together with the local-name set it introduces, then renders the final
      return expression with that same local set. Static-owner detection still
      applies for real static owners, but it no longer runs before prefix
      locals are known.
    - Required Facts: lambda parameter name, block prefix statements, local
      declarations introduced by the prefix, and the return expression. No
      package-specific owner names are required.
    - Owning Layer: `stream_lowering` owns stream closure rendering and its
      local-name environment; `call_plan` still owns ordinary call target
      classification outside stream closure rendering.
    - Fallback Shape: if a block-lambda statement cannot be rendered as a
      prefix statement, the existing stream fallback remains responsible. This
      rule only changes the local environment passed to already-rendered return
      expressions.
    - Evidence:
      `stream_block_lambda_return_uses_declared_local_receiver_not_static_owner`
      proves a neutral builder-style stream lambda emits `builder.build()` and
      rejects `ProductBuilder::build()`/`Builder::build()`. The focused
      GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_stream_block_return_locals_v1`
      moves from `cargo_check_errors=34` to `32`; the previous undeclared
      `BeginBuilder` and `EndBuilder` rows are gone. Generated
      `generate_partition_list_from_intervals` now emits
      `self.create_partition_with_begin_and_end(begin_builder.build(), end_builder.build())`.
      The remaining related error is the real return-shape mismatch
      `Vec<Partition>` vs `Vec<Box<dyn Any>>`, not local receiver resolution.
  - Typed External Builder Product Slice:
    - Ideal Rust Target:
      Fact-backed Java builder terminals such as
      `GenomicsDBImportConfiguration.Partition.Builder.build()` should return
      the builder product facade, not `Box<dyn Any>`. A private helper declared
      as returning a final facade should keep that exact Rust return type:
      `fn create_partition_with_begin_and_end(...) ->
      GenomicsDBImportConfigurationPartition`. A `Vec<Partition>` push should
      pass the concrete helper result directly, not box it first.
    - Implementation Rule:
      `external_packs` owns typed `build` return facts for GenomicsDB
      protobuf-style builders. `stdlib` consumes those facts when generating
      opaque facade support, matching nested aliases such as
      `GenomicsDBImportConfigurationPartitionBuilder` to their concrete builder
      implementation. `signature` keeps exact final erased facades concrete for
      method params and returns when facts prove the declared type is final,
      including public read-only borrowed parameters. Public `Object`, wildcard,
      interface, and non-final erased boundaries remain conservative. `call_plan`
      keeps final erased facade collection elements concrete so `Vec<T>::push`
      uses the same slot shape that `slot_render` renders for `Vec<T>`.
    - Evidence:
      `external_nested_builder_facade_support_is_generated_from_facts_and_used_calls`
      proves typed builder `build` support and a concrete private helper
      return. `genomicsdb_pack_declares_constructors_and_nested_builder_qualifiers`
      proves GenomicsDB builder product facts are pack-owned.
      `call_plan_owns_call_and_new_target_selection_before_argument_lowering`
      proves final erased facade list-push arguments stay concrete while
      non-final erased map values still use erased-object dispatch. The focused
      GenomicsDB gate
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_typed_builder_products_v4`
      moves from `cargo_check_errors=32` to `30`; `E0308` drops from `16` to
      `14`. Generated `GenomicsDBImport` now emits concrete
      `create_partition_with_begin_and_end`, direct `config_partitions.push(...)`,
      and stream `.collect::<Vec<_>>()` over `Partition` results.
      `external_nested_builder_facade_support_is_generated_from_facts_and_used_calls`
      now also proves a public final facade return and a public read-only final
      facade receiver render as named facade boundaries instead of
      `Box<dyn Any>`.
    - Follow-up: the focused gate at
      `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_final_facade_public_boundary_v2`
      clears the `VidMappingPB.get_fields`/`to_builder` erased-receiver bucket,
      but exposes crate-level external facade ownership as the next blocking
      design issue. `GATKGenomicsDBUtils::get_protobuf_vid_mapping_from_json_file`
      returns the `VidMappingPB` support type generated in its module, while
      `GenomicsDBImport` expects its own module-local `VidMappingPB`. The
      package harness currently hoists only a fixed platform support set
      (`JavaException`, `Path`, `URI`, `File`, `IOUtils`). To make concrete
      external facades viable across modules, external-pack facade structs,
      aliases, and impl surfaces need one crate-level owner and duplicate
      per-module support must be suppressed or imported from that owner.
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
      Optional external getters under explicit Java casts now preserve the
      optional carrier at optional return slots:
      - Ideal Rust Target: if Java writes `return (VCFHeader) reader.getHeader();`
        and the solved Rust return slot is `Option<VCFHeader>`, generated Rust
        should return the getter carrier directly: `return reader.get_header();`
        or, in throwing methods, `return Ok(reader.get_header());`. It should
        not emit `Some(reader.get_header())`, because the external facade
        already models the Java nullable getter as `Option<VCFHeader>`.
      - Implementation Rule: `lower_expr_for_slot` treats casts to `T` as a
        carrier no-op when the target slot is `Option<T>` and the cast source is
        already shaped as the same optional Java type. The source shape is read
        first through `external_call_lowered_type`, then through ordinary shape
        inference, so the rule is fact-driven and not tied to `FeatureReader` or
        any method name.
      - Required Facts: target slot optional element type, Java cast target
        type, external call return type/nullability, and fallback expression
        shape.
      - Owning Layer: `lower` owns slot-aware cast bypass; external packs own
        nullable getter facts; coercion still owns ordinary `T` to `Option<T>`
        wrapping when the source is not already optional.
      - Evidence:
        `typed_feature_reader_header_cast_uses_external_pack_return_fact` now
        expects direct optional getter return and rejects
        `Some(reader.get_header())`. The focused GenomicsDB gate at
        `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_optional_cast_return_v1`
        still has `cargo_check_errors=10`, but the specific
        `expected VCFHeader, found Option<VCFHeader>` error in
        `get_header_from_path` is gone; generated code now emits
        `return Ok(reader.get_header());`.
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

- [x] `EXC-1C`: Project synchronous callback lambda throws into enclosing method
  result carriers.
  - Owner: `src/semantic_analysis.rs`, `src/lower.rs`
  - Ideal Rust Target: Java callbacks that execute synchronously at the call
    boundary, such as collection `forEach`, should make the enclosing Rust
    method return `Result<..., JavaException>` when the callback can throw. The
    callback body should still lower through a local `Result<(), JavaException>`
    carrier so catch arms can use ordinary `return Err(...)` inside the
    enclosing method. Deferred boundaries such as `executor.submit(...)` should
    not force the caller method to become throwing; their failures belong to the
    submitted future/result carrier.
  - Implementation Rule: thrown-effect collection descends into lambda bodies
    only from call sites whose callback executes at the current boundary, while
    known deferred execution methods (`submit`, `execute`, `supplyAsync`,
    `runAsync`) suppress enclosing-method throw projection. Lowering consumes
    the resulting method thrown-effect fact through the normal `Result`
    signature and exception-plan paths; it does not special-case `forEach` at
    return rendering time.
  - Required Facts: call name, lambda body thrown types, deferred callback
    boundary names, method return shape, and existing catch hierarchy facts.
  - Owning Layer: semantic analysis owns whether a lambda throw reaches the
    enclosing method; exception/signature planning owns the `Result` carrier;
    lower owns only rendering the planned callback and catch shape.
  - Fallback Shape: unknown callback calls conservatively project lambda throws
    unless they are known deferred boundaries. This may over-wrap some
    framework callback APIs, but avoids losing exceptional control flow; precise
    external API facts can later override the boundary classification.
	  - Evidence: `lambda_block_throw_marks_enclosing_method_throwing` proves
	    semantic facts see throws inside a `forEach` lambda;
	    `foreach_lambda_throw_wraps_enclosing_method_in_result` proves the Rust
	    method signature becomes `Result<..., JavaException>` and returns
	    `Ok(readers)`; `executor_submit_future_slot_shapes_block_lambda_result`
	    proves `submit` remains a future/result-carrier boundary rather than
	    wrapping the containing method. The GenomicsDB focused gate
	    `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_lambda_throw_method_result_v2`
	    keeps the effective compile-error state at 19 previous errors while fixing
	    `get_feature_readers_in_parallel` to return
	    `Result<HashMap<..., ...>, JavaException>`.

- [x] `EXC-1D`: Classify fall-through try/catch blocks as fall-through for
  method epilogue insertion.
  - Owner: `src/exception_plan.rs`, `src/lower.rs`
  - Ideal Rust Target: a Java `void` method that throws through
    `Result<(), JavaException>` and ends with a `try/catch` whose try body can
    fall through should keep the generated `match` as a statement and then emit
    the normal `return Ok(());` method epilogue. Catch arms still return
    `Err(...)` early, but the successful `Ok(())` arm does not need to be a
    terminal expression.
  - Implementation Rule: `exception_plan` treats a supported `try/catch` or
    try-with-resources statement as terminating only when the planned try body
    completion is `Terminates`. Fall-through unit try/catch blocks remain
    supported, but they no longer suppress the method-level `Ok(())` epilogue
    owned by `lower_method`.
  - Required Facts: planned try statement kind, planned try body completion, and
    method exception carrier state.
  - Owning Layer: `exception_plan` owns control-completion classification;
    `lower_method` owns epilogue insertion; `exception_lowering` owns only the
    match spelling.
  - Fallback Shape: unsupported try/catch/finally shapes still use the existing
    unsupported marker rather than guessing completion.
  - Evidence: `simple_rethrow_catch_generated_rust_compiles`,
    `simple_cause_chaining_generated_rust_compiles`, and
    `internal_throwing_call_in_try_generated_rust_compiles` pass. In the focused
    GenomicsDB gate
    `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_try_ok_arm_v1`, the previous
    `Ok(()) => {}` mismatched-type error is gone; remaining failures are
    ownership, external facade, duplicate support shell, and unresolved receiver
    shape issues.

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
	    `TryCatchReusesMovedArgument.java` verifies that catch-body future uses are
	    visible while lowering the try body: if an owned argument is moved into a
	    try-body call and later read by a catch arm, the call receives the normal
	    owned-recovery clone rather than moving the only value. The focused
	    GenomicsDB gate at
	    `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_try_catch_future_use_clone_v1`
	    moves from `cargo_check_errors=21` to `20`; the previous
	    `borrow of moved value: sample_to_file_map_path` bucket is gone, and
	    `SampleNameMap` now emits
	    `Files::read_all_lines(sample_to_file_map_path.clone()/* TODO(translation): validate added clone */)`.
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
    correct. Cleanup propagation is a late call-emission decision: `lower`
    plans each resource `close` call through the shared
    `CallExceptionPropagationPlan`, and `exception_lowering` renders the
    already-planned cleanup statements in reverse declaration order. A modeled
    void/infallible cleanup becomes `reader.close();`; a cleanup with a
    thrown-effect fact remains `reader.close()?;`. The renderer should not
    blindly attach `?` to every cleanup call, because facade methods with return
    type `()` are valid Rust side-effect calls.
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
    `exception_lowering_policy_owns_result_support_ok_and_err_shapes` now also
    covers external try-with-resources cleanup where `FeatureReader.close` has
    `thrown=[]` versus `thrown=[IOException]`, proving the `?` choice is made
    from call facts instead of a hardcoded close rule. The focused GenomicsDB
    gate `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_twr_unit_cleanup_v1`
    moves from `cargo_check_errors=19` to `18`; the `E0277` diagnostic for
    `reader.close()?` on `()` is gone and generated cleanup now contains
    `reader.close();`.
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
    Classes with no explicit Java constructors now receive a synthetic Rust
    `pub fn new() -> Type` only when the co-translated support context contains
    an actual zero-argument `new Type()` use. The constructor reuses the normal
    field-initializer/default field-value plan instead of call-lowering
    fabricating a fallback. This preserves Java's implicit default constructor
    for used source types such as `GenomicsDBArgumentCollection`, while avoiding
    broad emission of risky default constructors for large framework classes
    whose inherited initializers still need unresolved instance context.
    `item_assembly_owns_struct_impl_and_function_shells_from_signatures` covers
    the neutral implicit-constructor shape.
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

## Emitter Quality

- [x] `EMIT-1`: Render Rust expression parentheses from precedence, not Java
  syntax.
  - Owner: `src/emit.rs`
  - Ideal Rust Target: generated Rust should look like hand-written Rust for
    ordinary operator expressions: `if a && b`, `while i < n`, `return -1;`,
    and `(a + b) * c` only when parentheses are required to preserve meaning.
    Java-origin parenthesized expressions should not survive as noisy Rust
    warnings when the structured Rust expression already carries operator
    precedence.
  - Implementation Rule: the Rust emitter assigns precedence to `RustExpr`
    variants and prints child expressions with parentheses only when the child
    precedence is lower than the parent, or when a right-hand binary child of
    equal precedence would change associativity. Semantic lowering and
    conversion planning must keep producing structured `RustExpr::Binary` and
    `RustExpr::Unary` nodes when they want this cleanup; raw snippets remain
    outside the precedence printer until they are promoted to structured IR.
  - Required Facts: Rust operator token, child expression kind, parent
    precedence, and right-child associativity position.
  - Owning Layer: `emit` owns Rust syntax precedence and cosmetic parentheses.
    Parser, semantic analysis, coercion, and lowering own meaning and should
    not add ad hoc de-parenthesizing rules for individual Java constructs.
  - Fallback Shape: when lowering emits `RustExpr::Raw`, the emitter preserves
    it verbatim. If a warning remains in raw output, the fix is to move that
    construct into structured Rust IR or add a narrow raw-fragment renderer, not
    to strip parentheses by string search.
  - Evidence: `emit::tests::binary_expressions_do_not_emit_outer_parentheses`,
    `emit::tests::lower_precedence_children_still_get_parentheses`,
    `emit::tests::unary_expressions_do_not_emit_outer_parentheses`, and
    `emit::tests::casts_drop_statement_position_parentheses_but_keep_binary_operand_parentheses`
    cover the rule. `RustExpr::Cast` now lets primitive widening print
    `let x: i64 = value as i64;` in assignment position while preserving
    `(value as i64) < limit` and `acc * (value as i64)` where Rust precedence
    requires parentheses. The GenomicsDB `compile.log` warnings about `if (...)`,
    `while (...)`, `return (...)`, assigned `(-1)`, and simple unary wrappers
    are therefore an emitter concern; remaining warnings identify raw snippets
    or constructs not yet represented as structured `RustExpr` trees.

- [x] `EMIT-2`: Promote stream closure bodies out of raw strings.
  - Owner: `src/stream_lowering.rs`
  - Ideal Rust Target: stream closures should render as ordinary Rust closures
    whose condition and body expressions participate in precedence rendering:
    `.filter(|x| x.start() >= lo && x.end() <= hi)` and
    `.map(|partition| { let parts = ...; if parts.len() as i32 != 3 { None } else { ... } })`.
  - Implementation Rule: stream lowering carries enough expression structure to
    render Java lambda expressions with Rust precedence before they are appended
    to stream adapter text. The stream lambda renderer assigns precedence to
    binary, unary, call, field, array, and cast-like fragments and only adds
    parentheses when a child expression would otherwise bind incorrectly. It
    does not repair generated closure text with search-and-replace.
  - Required Facts: closure parameter names, lowered local statements, lowered
    condition expression, lowered tail expression, and target stream adapter.
  - Fallback Shape: raw stream fragments that bypass the lambda expression
    renderer remain verbatim. If they produce `unused_parens`, the fix is to
    route that fragment through structured stream expression rendering or the
    main `RustExpr` emitter, not to strip parentheses by string search.
  - Evidence: `tests/fixtures.rs` now expects stream closures such as
    `.filter(|v| **v > 0)` and `.map(|v| *v + base)` without outer binary
    parentheses, while preserving cast operand parentheses such as
    `.filter(|v| (v.len() as i32) > 3)`. The focused GenomicsDB gate at
    `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_stream_parens_v3` has zero
    `warning: unnecessary parentheses` rows; the remaining warnings are
    `unused_mut`.

- [x] `EMIT-3`: Render stream lambda local `mut` from local reassignment, not
  from Java declaration defaults.
  - Owner: `src/stream_lowering.rs`
  - Ideal Rust Target: temporary locals inside iterator closures should look
    like hand-written Rust: `let parts = value.split(",").collect::<Vec<_>>();`
    when the binding is only read, and `let mut current = ...; current = ...;`
    only when the lambda body reassigns the binding.
  - Implementation Rule: stream lambda rendering computes the set of mutated
    lambda-local names from assignments and pre/post increment/decrement inside
    the Java lambda block before rendering prefix statements. `VarDecl`
    statements render `let mut` only when the corresponding local is in that
    set. This rule is limited to stream-lambda local text; broader method-body
    local mutability remains owned by `signature`/ordinary statement lowering.
  - Required Facts: lambda block statements, local declaration names, assignment
    targets, update-expression targets, and snake-case Rust local names.
  - Owning Layer: `stream_lowering` owns stream closure text that has not yet
    been promoted to ordinary `RustStmt` blocks. It may decide closure-local
    `mut` spelling for that text, but it must not override method-level local
    mutability facts.
  - Fallback Shape: non-stream method locals continue to use
    `LocalPlan::binding_mutable`. If those produce `unused_mut`, the next fix
    must improve the local mutability fact or lower the construct to structured
    Rust that can prove whether a mutable receiver is required.
  - Evidence:
    `stream_plan_lowers_flat_map_block_returning_stream_pipeline`,
    `stream_plan_accepts_value_returning_block_lambdas`, and
    `stream_lambda_lowers_split_length_and_integer_parse_in_nullable_map` now
    expect read-only stream temporaries without `mut`.
    `stream_plan_keeps_mut_for_assigned_block_lambda_locals` proves reassigned
    lambda locals still keep `let mut`. The focused GenomicsDB gate at
    `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_stream_unused_mut_v1`
    keeps `cargo_check_errors=19` and reduces total warnings from 11 to 4.

- [ ] `EMIT-4`: Narrow ordinary method-body local `mut` without breaking
  mutable receiver and assignment cases.
  - Owner: `src/signature.rs`, `src/lower.rs`, and only then `src/emit.rs` if a
    final Rust-IR check is still needed.
  - Ideal Rust Target: ordinary method locals should use `let mut` exactly when
    Rust requires mutable binding syntax: direct reassignment, assignment to a
    field/index through that binding, passing as `&mut`, or calling a known
    mutable-receiver Rust API such as native collection `insert`, `sort`, or
    `push`. Generated facade setter calls that take `&self` should not force
    `let mut`.
  - Implementation Rule: do not apply a broad emitter-only `mut` removal pass.
    A trial broad pass regressed the GenomicsDB gate from 19 to 45 errors by
    dropping required `mut` from raw assignment/update patterns and native map
    mutation sites. The next implementation must classify ordinary locals by
    structured binding use and receiver mutability facts before changing
    emitted declarations.
  - Required Facts: local assignment targets, raw assignment/update fallback
    sites, native collection mutating method facts, external receiver
    mutability facts, and whether the lowered call actually requires `&mut
    self`.
  - Fallback Shape: keep `let mut` rather than risking compile errors. An
    `unused_mut` warning is preferable to dropping a required mutable binding.
  - Evidence: current remaining GenomicsDB warnings are four ordinary
    method-body `unused_mut` rows in
    `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_stream_unused_mut_v1`.

- [ ] `EMIT-5`: Resolve cosmetic `()`/parenthesis warnings at the final
  expression/block rendering boundary.
  - Owner: `src/emit.rs`, `src/statement_render.rs`, and block-completion
    planning in `src/lower.rs` only where the warning reflects a real unit
    block shape.
  - Ideal Rust Target: generated Rust should not contain syntactically
    redundant wrappers such as `if (condition)`, `while (condition)`,
    `return (value);`, or expression-position `{ ...; () }` when the unit value
    is only an artifact of Java statement/block lowering. Rust precedence
    should still preserve necessary parentheses, such as
    `(value as i64) < limit`, and explicit unit should remain when it is the
    meaningful return value of a closure, callback, or fall-through branch.
  - Implementation Rule: treat these warnings as late syntax/block-shape
    normalization. The `RustExpr` emitter owns operator-priority parentheses;
    statement/block rendering owns redundant return/condition wrappers and
    fall-through unit expression elision. Earlier semantic layers should emit
    structured expressions and block completion facts, not local string fixes.
  - Required Facts: Rust expression precedence, statement position, block
    completion kind, expected expression type, and whether `()` is an explicit
    semantic value or a lowering artifact.
  - Owning Layer: `emit` owns expression precedence; `statement_render` and
    `lower_method` own statement-position unit elision; exception/stream
    lowering own callback and try/catch result carriers. No Java library pack
    or call-lowering rule should strip parentheses or unit values by method
    name.
  - Fallback Shape: keep explicit `()` or parentheses when removing them would
    change type, precedence, associativity, control flow, or callback return
    shape. A warning is acceptable until the source construct is represented as
    structured Rust IR with enough completion facts.
  - Evidence: the existing `EMIT-1` and `EMIT-2` tests cover precedence-based
    parenthesis removal; current GenomicsDB compile gates still report four
    warnings, so `EMIT-5` remains open as the catch-all for non-precedence
    unit/block cleanup. `emit::tests::statement_position_unit_expressions_are_elided`
    and `emit::tests::return_unit_expr_renders_as_bare_return` cover the first
    late cleanup slice: bare statement-position `()` is dropped, `return ();`
    becomes `return;`, and result/callback carriers such as `Ok(())` are
    preserved.

## External Utility Facades

- [x] `EXT-UTIL-1`: Render external validation utilities as side-effect
  facade methods, not generic-return placeholders.
  - Ideal Rust Target: a Java static validation call such as
    `Utils.validate(condition, message)` or `Utils.validateArg(condition,
    message)` should render as an ordinary Rust side-effect call:
    `Utils::validate(condition, message);` with a facade like
    `pub fn validate<T1>(_arg0: bool, _arg1: T1) -> () { () }`. The condition
    argument should be concrete `bool`; the message argument may remain generic
    unless facts prove a specific `String`, supplier, or error type overload.
    The call must not require `R: Default` inference.
  - Implementation Rule: external pack API-effect facts with `return_type:
    None` and role `SideEffect` own the unit return shape. Facade generation
    consumes those facts for static/imported static methods and renders `()`
    before falling back to generic `R: Default`. Argument-effect facts may
    concrete only the slots they prove, leaving unmodeled overload-sensitive
    positions generic.
  - Required Facts: resolved static owner, method name/arity, API role,
    absent return type, receiver mutability, and per-argument type facts for
    slots that are safe across overloads.
  - Owning Layer: external library packs own `Utils.validate` and
    `Utils.validateArg` side-effect facts; `stdlib` facade generation owns the
    fallback order from facts to generic placeholders; lowering only emits the
    resolved call.
  - Fallback Shape: if no fact is available, imported static utility methods
    keep the existing generic facade fallback or the name-prefix side-effect
    heuristic for `validate_*`. Unknown message overloads remain generic
    instead of being forced to `&str`.
  - Evidence: `builtin_external_member_lookup_uses_cached_pack_facts` proves
    the built-in Broad Hellbender utility pack exports
    `Utils.validate(boolean, ...)` as a side-effect API with no return type and
    a concrete boolean condition argument.
    `external_static_validation_facade_uses_side_effect_fact_return` proves a
    neutral static validation call emits `pub fn validate<T1>(_arg0: bool,
    _arg1: T1) -> () { () }` and avoids `R: Default`. The focused GenomicsDB
    gate at
    `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_utils_validate_void_v2`
    remains at `cargo_check_errors=9`, but the previous
    `Utils::validate(header.is_some(), ...)` generic return inference issue is
    gone. The first remaining inference blocker is now
    `Files::write_string<R: Default, ...>`.

- [x] `EXT-UTIL-2`: Render Java platform file-write static utilities with a
  concrete return facade.
  - Ideal Rust Target: Java `Files.writeString(path, contents)` should
    ultimately map to a Rust filesystem write boundary such as
    `std::fs::write(path, contents)?;` when exception/result lowering and
    path/string conversions are available. At the current external-facade
    phase, the maintainable fallback is a fact-backed Java-platform facade:
    `Files::write_string(path, contents);` with support
    `pub fn write_string<T0, T1>(_arg0: T0, _arg1: T1) -> Path {
    Default::default() }`. The facade should not ask Rust to infer
    `R: Default` for an ignored Java return value.
  - Implementation Rule: Java platform static utility return facts own the
    concrete Java return type for `Files.writeString`. The facade generator
    consumes those facts before the generic imported-static fallback. Argument
    slots may remain generic until `CharSequence`/`Option<String>` recovery and
    path borrowing are modeled without fighting local nullability bugs.
  - Required Facts: resolved static owner, method name/arity, Java platform
    method return type, return nullability, and static utility role.
  - Owning Layer: `stdlib` owns Java platform static utility signatures;
    `stdlib` facade generation owns support method rendering; lowering only
    emits the selected static call.
  - Fallback Shape: if the Java platform method is unknown, keep the existing
    `R: Default` fallback. If content/path argument shape facts are unknown,
    keep those arguments generic rather than inserting a misleading borrow or
    unwrap.
  - Evidence:
    `java_platform_files_write_string_facade_uses_known_path_return` proves a
    neutral `java.nio.file.Files.writeString(Path, String)` call emits
    `Files::write_string(path, contents);`, generates
    `pub fn write_string<T0, T1>(_arg0: T0, _arg1: T1) -> Path`, and avoids
    the generic `R: Default` facade. The focused GenomicsDB gate at
    `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_files_write_string_return_v1`
    moves from `cargo_check_errors=9` to `8`; the previous
    `Files::write_string<R: Default, ...>` inference blocker is gone.

## Package Crate Assembly

- [ ] `PKG-ASM-1`: Give duplicate external support facades one crate identity
  across focused-package modules.
  - Ideal Rust Target: when two translated Java files reference the same
    external facade type, generated Rust should have one nominal crate type and
    all module-specific support methods should attach to that type. For
    example, calls crossing `GATKGenomicsDBUtils` and `GenomicsDBImport`
    should use a single `crate::VidMappingPB` and a single
    `crate::GenomicsDBImporter`, not distinct
    `generated_00001_*::VidMappingPB` and
    `generated_00004_*::VidMappingPB` types.
  - Implementation Rule: package/crate assembly owns support-facade identity
    across independently translated files. The short-term focused gate harness
    may unify duplicate declarations only when they are proven to be
    support-prelude declarations before a module's real translated source
    type. It must not alias all duplicate simple names, because real source
    classes from different packages can share a Rust name. Longer-term Rust
    lowering should split module lowering from shared support generation so
    the support registry is structured rather than parsed from rendered Rust.
  - Required Facts: generated module name, real translated source type for the
    module, support-prelude declared type names, duplicate support type counts,
    and deterministic canonical owner module for each duplicate support type.
  - Owning Layer: crate assembly owns cross-module support identity;
    `stdlib` owns per-program facade surfaces; `lower` should eventually expose
    shared-support emission as a first-class phase. Individual call lowering
    must not paper over duplicate identities with casts.
  - Fallback Shape: if a duplicate type is not proved to be a support-prelude
    facade, leave it module-local and let the compile gate report the nominal
    mismatch. If duplicate method impls conflict after unification, record the
    surface-merge conflict instead of silently dropping methods.
  - Evidence: a raw harness-level duplicate support-prelude unification probe
    was rejected after the focused GenomicsDB gate regressed from
    `cargo_check_errors=8` to `611` at
    `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_shared_support_identity_v1`.
    The regression was dominated by duplicate inherent methods and conflicting
    trait impls after method surfaces from several modules were attached to
    the same raw-text-unified facade type. This keeps `PKG-ASM-1` open and
    narrows the acceptable implementation: shared support identity must merge
    structured facade declarations, impl methods, trait impls, aliases, and
    fields deliberately, not by stripping duplicate rendered struct lines.

## External Parameter Nullability Defaults

- [ ] `EXT-NULL-1`: Default external reference and callback parameters to
  non-null unless facts prove or strongly suggest nullable.
  - Ideal Rust Target: external library calls should expose ordinary Rust
    parameter shapes by default: `T`, `&T`, `&mut T`, or `F: Fn...`, not
    `Option<T>`/`Option<F>` everywhere. If a Java call site passes `null`, or
    an external/user/source-summary fact marks that parameter nullable, the Rust
    slot becomes optional and non-null branch values are wrapped once at the
    slot boundary. For example, a Java constructor argument such as
    `bypass ? null : this::createSampleToReaderMap` should target
    `Option<callback>`; the non-null branch should become `Some(...)`, and the
    method reference should be adapted against the callback facts, such as
    `(Map<String, URI>, int, int) -> reader map`.
  - Implementation Rule: parameter nullability is selected by ordered facts,
    not by ad hoc lowering. Primitive parameters are always non-null. For
    external reference/callback parameters, observed `null` at any resolved
    owner/kind/arity/parameter slot makes the slot nullable. Explicit external
    pack facts, user facts, source-summary annotations, or source bodies may
    also mark the slot nullable or non-null; a contradiction with an observed
    `null` call site should emit a conflict diagnostic and choose the safe
    optional shape unless a future explicit override policy says otherwise. If
    no nullable evidence exists, the slot is `DefinitelyNonNull` with
    provenance `AssumedNonNullExternalDefault`. This rule applies only to
    parameters; return nullability is governed by return facts.
  - Required Facts: resolved callable owner, callable kind
    (constructor/static/instance), method name where applicable, arity,
    parameter index, Java parameter category (primitive/reference/callback),
    observed call-site nullness, source-summary/annotation nullability, and
    external or user fact provenance.
  - Owning Layer: `semantic_analysis`/usage records observed call-site
    nullness; `external_packs` and future user fact injection own declared
    external parameter facts; `signature`/slot-shape planning chooses
    optional versus non-optional parameter shape; `conversion_plan` wraps
    `Some(...)`/`None` at the argument boundary; method-reference planning
    consumes callback slot signature facts for arity and argument types.
    Rendering must not rediscover nullable external parameters from raw
    expression strings.
  - Fallback Shape: when an external reference/callback parameter has no
    nullable evidence, assume non-null and record the assumption. When nullable
    evidence exists but the target callback or reference type is unknown, emit
    `Option<T>`/`Option<F>` with generic arguments plus a diagnostic rather
    than forcing a non-null slot. Future injected facts may replace the
    conservative generic shape without changing lowering.
  - Evidence: add tests proving a neutral external constructor callback
    parameter remains non-optional, the same parameter becomes optional when
    any call passes `null`, non-null alternatives in a nullable ternary are
    wrapped with `Some(...)`, and explicit fact injection can mark a parameter
    nullable/non-null with conflict diagnostics. This item is open until the
    focused GenomicsDB import constructor no longer emits an incompatible
    `None` versus closure branch or a one-argument method-reference callback
    for the reader-map constructor parameter.
  - Status: Partial. Constructor argument shaping now consumes external
    argument facts through `external_constructor_arg_shapes`, defaulting
    unmodeled neighboring external constructor parameters to conservative
    non-optional slots. `conversion_plan` treats lambdas and method references
    as target-shaped callback expressions before optional wrapping, and
    method-reference planning can render current-receiver method references
    with arity from the current type signature. The GenomicsDB pack now marks
    `ImportConfig::new` parameter 6 as a nullable escaping `Function` slot.
    `nullable_external_constructor_callback_slot_wraps_method_reference_branch`
    proves `bypass ? null : this::readerFactory` lowers to
    `None`/`Some(|arg0, arg1, arg2| ...)` without a callback downcast.
    `call_plan_owns_call_and_new_target_selection_before_argument_lowering`
    proves one constructor argument fact shapes all constructor slots while
    unmodeled neighbors remain non-optional. The focused GenomicsDB gate at
    `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_ext_null_arity_v1` moves
    from `cargo_check_errors=8` to `7`; the previous incompatible
    `None`/closure branch, one-argument callback, and `Function` downcast on
    the `ImportConfig::new` argument are gone. This item remains open because
    general observed-null fact production and explicit injected-fact conflict
    diagnostics are not implemented yet.
  - Follow-up Status: Partial. The GenomicsDB pack now also models
    `ImportConfig::new` parameter 4 as a non-null escaping
    `Set<VCFHeaderLine>` slot. This gives the existing optional-self-field
    recovery a typed owned target, so Java `mergedHeaderLines` passed to that
    constructor lowers as
    `self.merged_header_lines.as_ref().expect("Java null receiver").clone()`
    rather than moving out of `self`. The neutral
    `external_constructor_owned_set_param_clones_optional_self_field_element`
    test covers the same pattern for an external constructor with a typed
    owned set parameter. The focused GenomicsDB gate at
    `/tmp/new_java2rust_gatk_genomicsdb_compile_gate_importconfig_header_lines_v1`
    moves from `cargo_check_errors=7` to `6`; the `E0507` move out of
    `self.merged_header_lines` is gone.

- [ ] `EXT-NULL-2`: Produce observed-null external parameter facts and conflict
  diagnostics.
  - Ideal Rust Target: if any translated call site passes Java `null` to a
    resolved external reference/callback parameter, later call sites for the
    same external callable slot should consistently target `Option<_>`.
    Explicit user or external facts should be able to mark a slot nullable or
    non-null, and contradictory source evidence should produce a visible
    diagnostic rather than silently picking whichever fact was inserted last.
  - Implementation Rule: `semantic_analysis` records finite call-slot
    nullability evidence keyed by external receiver/constructor type, callable
    kind, rendered method/constructor name, arity, and parameter index.
    `FactSet` merges that evidence with external/user facts using the
    documented priority: observed `null` requires a safe optional shape unless
    a future override policy explicitly escalates the contradiction.
  - Required Facts: resolved external callable identity, arity, parameter
    index, argument expression nullability, external/user fact provenance, and
    diagnostic location.
  - Owning Layer: `semantic_analysis` owns observed evidence and diagnostics;
    `facts` owns merge/query behavior; `signature` and `conversion_plan`
    remain consumers.
  - Fallback Shape: if the callable identity cannot be resolved, keep the
    current conservative per-call shape and emit no global fact.
  - Evidence: add tests where one external call passes `null` and another
    non-null call to the same slot becomes optional, plus tests for explicit
    non-null facts conflicting with observed null.

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
