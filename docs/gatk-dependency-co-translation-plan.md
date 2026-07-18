# GATK Dependency Co-Translation Plan

## Purpose

GATK should not be translated as if every imported library is either a direct
Rust standard-library mapping or an opaque unknown. Some dependencies are
better treated as source that participates in the resolver and semantic fact
pipeline. Others should remain external API facts, Rust crate mappings, FFI
boundaries, or conservative opaque boundaries.

This document fixes the policy for deciding which mode to use. The goal is to
improve Rust output and analysis quality without turning GATK or HTSJDK into
hard-coded lowering rules.

## Evidence

The current old-corpus GATK import scan is dominated by:

- `org.broadinstitute.hellbender`: GATK source itself.
- `org.broadinstitute.barclay`: command-line and argument infrastructure.
- `htsjdk.variant.variantcontext`, `htsjdk.variant.vcf`,
  `htsjdk.samtools`, `htsjdk.samtools.util`, and `htsjdk.tribble`: domain data
  models and file-format APIs.
- `java.util`, `java.util.stream`, `java.util.function`, `java.io`, and
  `java.nio.file`: Java platform semantics.
- `org.apache.commons`, `com.google.common`, `org.apache.logging`, Spark,
  Hadoop, Kryo, Scala, GenomicsDB, native bindings, JSON, CSV, HDF5, HTTP, SQL,
  and compression libraries: infrastructure and runtime APIs.

The old repository also contains an HTSJDK corpus with roughly 800 Java source
files. That makes HTSJDK a realistic separate full-translation target, but not
something to partially co-translate inside a GATK run by accident.

## Translation Modes

Each dependency should be assigned one of these modes:

- `co_translate_source`: source files participate in parsing, resolution,
  semantic facts, object strategy, slot shape, and eventual Rust emission.
- `source_summary`: source is scanned to extract declarations and facts, but
  method bodies are not emitted.
- `external_fact_model`: behavior is represented through facts with provenance;
  no Rust body is generated.
- `rust_crate_mapping`: calls and types are mapped to a Rust crate or local
  facade with explicit adapter facts.
- `ffi_boundary`: native or runtime-backed APIs are represented as typed
  boundaries with explicit effects and ownership assumptions.
- `conservative_boundary`: only minimal type, effect, and diagnostic facts are
  available.

Precedence is:

1. user overrides
2. co-translated source facts
3. source summaries
4. built-in external fact models
5. Rust crate or FFI mapping facts
6. conservative guesses

Lowering must consume the resolved facts and plans. It should not inspect Java
package names to rediscover dependency policy.

## External Library Workflow

When a GATK import pressure appears, handle it as a dependency-boundary
decision before adding lowering code. The workflow is:

1. Classify the dependency surface as co-translated source, source summary,
   external fact model, Rust crate mapping, FFI boundary, or conservative
   boundary.
2. Pick the smallest coherent library boundary that can be made all-or-nothing
   for the APIs we claim to own. A boundary may be a package family, a runtime
   facade, or a narrow public type family, but it must not be a one-off method
   workaround.
3. Write neutral fixtures inspired by the GATK use site. The fixture should
   exercise the semantic shape, such as constants, factories, getters,
   iterators, builders, exception behavior, or mutating handles, without
   depending on the whole GATK corpus.
4. Add facts, source-summary roots, or crate/FFI mapping facts for the selected
   boundary. If an adjacent public member is visible but not supported, add an
   explicit unsupported fact and diagnostic rather than leaving it to generic
   guessing.
5. Use resolver benchmark output to audit both behavior and provenance. The
   report must show which calls/constructors/fields were explained by source
   dependencies, external packs, crate mappings, unsupported facts, or
   conservative fallback.
6. Only after the fixture proves the boundary should the full GATK benchmark be
   used as a regression signal. Full-corpus improvements are useful evidence,
   but they are not the specification for the boundary.

Source-summary fact providers are part of this workflow. They are used for
dependency surfaces that should be visible to resolution and semantic analysis
without emitting Rust bodies. Benchmark reports therefore expose aggregate
source-summary provider fact counts and per-provider fact-kind columns,
including external API effect facts, argument-effect facts, constructor facts,
field facts, library-pack facts, and unsupported member facts. This makes a
GATK-shaped fixture auditable when, for example, a static constant is covered
by a summary field fact instead of by a built-in fallback or lowering
special-case.

## General External Library Model

External libraries should be handled through explicit library packs, not
scattered method-name heuristics. A pack is the unit of responsibility for one
library, package family, or runtime facade. It may contain precise mappings,
conservative mappings, unsupported diagnostics, crate mappings, and user
override hooks, but every decision must have provenance.

The pack boundary should be chosen deliberately. Good boundaries are:

- a Java package family with a coherent API, such as
  `htsjdk.variant.variantcontext`
- a runtime facade, such as HDF5 or SQLite
- a utility family, such as CSV or JSON
- a platform model, such as Java collections or streams

Bad boundaries are:

- one method discovered during lowering
- one GATK class that happened to call the library
- a Rust ownership workaround tied to a Java spelling
- a package subset that makes adjacent external values use incompatible Rust
  representations

### Pack Contents

Each external library pack should define:

- package or type patterns owned by the pack
- dependency mode: external facts, Rust crate mapping, FFI boundary, or
  conservative boundary
- Rust target: native Rust type, facade type, crate type, erased external
  object, or unsupported
- constructors, factories, builders, getters, setters, mutators, iterators,
  predicates, and value methods
- nullability of return values, fields, and parameters
- mutation effects on receiver and arguments
- aliasing and lifetime boundary assumptions
- collection and iterator element shapes
- exception or error behavior
- subclass, interface, enum, trait-object, and erased-object strategy facts
- unsupported APIs with explicit diagnostics
- precedence behavior for user overrides

The pack should produce facts. It should not lower Rust syntax directly. Rust
crate mappings and facade mappings are facts consumed later by signature,
shape, conversion, and emission stages.

Modeled constructors and fields are pack-manifest declarations. Built-in and
user-authored packs should emit constructor and field facts through the shared
manifest boundary application path, not through per-pack loops. This keeps
coverage, provenance, diagnostics, and user override precedence attached to the
pack model instead of to one dependency-specific helper.

Coverage lookup must be symmetric across modeled APIs, constructors, and
fields. Audits for all-or-nothing external coverage should ask the manifest for
the coverage state of each member kind instead of treating method calls as the
only covered surface. After parsing, user-authored pack manifests are expressed
as facts and diagnostics; parser tests should assert those facts, while direct
manifest helper tests should use concrete built-in or constructed manifests.
Semantic analysis must also be symmetric when admitting those facts: typed
field accesses on external receivers should import pack-declared field facts
through the same pack boundary used for method calls and constructors. The
unresolved-field path remains a conservative fallback and diagnostic source;
it must not be the only way a valid public external field becomes visible to
later stages.

### Coverage Levels

Use explicit coverage levels so "external" does not mean "guessed":

- `declared`: the pack knows the type exists, but no behavior is modeled.
- `conservative`: the pack knows signatures and emits boundary facts, but
  assumes unknown aliasing, unknown nullability, and possible mutation where
  needed.
- `semantic`: the pack knows enough behavior for nullability, mutation,
  builders, element types, and exceptions.
- `mapped`: the pack has a Rust facade or crate mapping with slot and
  conversion facts.
- `unsupported`: the pack intentionally rejects an API with a structured
  diagnostic instead of letting lowering guess.

For all-or-nothing external coverage, every public API in the selected pack
manifest must end in one of these states. It is acceptable for much of the
surface to be `conservative` or `unsupported`; it is not acceptable for unknown
methods to silently bypass the pack and become ad hoc lowering rules.

### Pack Lifecycle

The implementation loop for an external pack is:

1. Select a coherent pack boundary.
2. Mine GATK and fixtures for representative call shapes.
3. Create small neutral Java fixtures inspired by those shapes.
4. Write expected facts, diagnostics, slot shapes, and conversion plans.
5. Add a pack manifest with type ownership and coverage states.
6. Implement declaration and behavior facts in the external fact layer.
7. Add facade or crate mapping facts only after semantic facts exist.
8. Verify that unresolved calls inside the pack produce pack-owned diagnostics.
9. Run the GATK benchmark to measure effect, but do not tune against a single
   GATK file.

The loop should be applied in two passes. The first pass is a contract pass:
define ownership, coverage states, unsupported states, and user-override
precedence before adding many API details. The second pass is a saturation
pass: mine GATK for repeated call shapes, add neutral fixtures for those
shapes, and only then add narrow facts. This prevents the pack from becoming a
bag of one-off method fixes while still letting the benchmark drive useful
coverage.

For each implementation slice, pick exactly one library boundary and one
workflow-shaped fixture before touching the whole GATK corpus. The fixture
should use synthetic names and only the Java API shape borrowed from GATK: for
example, "configure a SQL connection, prepare a statement, execute a query"
rather than a GATK storage reader class. The slice is accepted when the fixture
proves all of the following:

- the pack owns the receiver types and constructors
- method calls are attributed to the pack, not to conservative guesses
- mapped calls also increment Rust crate/facade mapping counters when the mode
  is `rust_crate_mapping`
- return types, nullability, mutation effects, and thrown exceptions are facts
  visible to semantic analysis
- any unmodeled member on an owned type reports a pack-owned missing or
  unsupported diagnostic

Only after this neutral fixture passes should the GATK benchmark be run. The
benchmark result may decide the next slice, but it must not add a rule whose
only test is a GATK file. If a GATK call shape is too specific to express
without GATK names, record it as a candidate for source co-translation or a
larger external-pack saturation pass instead of merging a bespoke mapping.

Each pack should leave behind an auditable coverage matrix:

- owned package and type patterns
- constructor arities that are declared, semantic, mapped, conservative, or
  unsupported
- fields and nested types that are declared, semantic, mapped, conservative, or
  unsupported
- method groups by role: getter, predicate, indexed getter, setter, mutator,
  fluent mutator, iterator, factory, close/finalize, validation, unsupported
- return shape facts: scalar, optional scalar, collection, iterator, facade,
  erased external object, crate-mapped facade, `Result`-like error boundary
- argument effect facts: borrowed, no-alias, escapes, receiver-mutating,
  argument-mutating, unknown
- hierarchy facts: final value, cloneable value, erased object, dyn trait,
  closed enum candidate, exception superclass
- deliberate static-receiver aliases used by the current resolver, such as
  normalized unresolved static keys
- benchmark evidence that motivated each added cluster, not each individual
  method

Pack composition must be explicit. A workflow can cross multiple packs, but no
pack should absorb another library just because one corpus combines them. For
example, SQLite configuration belongs to the `sqlite` crate-mapping pack, while
generic JDBC interfaces such as `java.sql.Connection`, `java.sql.Statement`,
`java.sql.ResultSet`, and `java.sql.ResultSetMetaData` belong to a separate
`java.sql` semantic facade pack. The shared workflow test should load both
providers with built-in packs disabled and prove which calls are attributed to
each provider.

Static Java utility or factory calls need the same treatment as receiver calls.
If the resolver normalizes an unresolved static receiver into a value-like key
(`DriverManager.getConnection` becoming `driver_manager.getConnection`), the
pack may declare that alias as an owned type pattern and model it explicitly.
That alias is a fact-layer compatibility key, not a lowering rule, and it must
be listed in the pack matrix with the canonical Java type. Do not use such
aliases to claim broad platform surfaces like `java.lang.Class`; class loading
and reflection belong to a separate reflection/class-model policy. The current
platform policy covers only narrow `Class` token metadata and loading facts:
`Class.getSimpleName()`, `getName()`, `getCanonicalName()`,
`isAssignableFrom(Class)`, and `Class.forName(String)`. These facts use
`BuiltinExternalModel` provenance, carry typed/nullability/checked-exception
metadata, and do not make any dependency pack own `Class`.

Simple imported type names are not safe global pack boundaries. If a pack needs
to cover wildcard imports such as `import java.sql.*`, parsing or resolution
must qualify the relevant type uses before pack matching. The pack should then
own `java.sql.Connection`, not the unqualified `Connection`, because another
GATK package may import or define a project-domain `Connection`.

When a GATK call is not covered, the default response is not "add this method".
First classify the missing call into one of these buckets:

- inside an owned pack and should be modeled by that pack
- inside an owned pack but intentionally unsupported
- outside every selected pack and should remain a conservative external
  boundary
- better represented by co-translating or summarizing a dependency source root
- better represented by a Rust crate/facade mapping
- actually a resolver/type-inference failure where the receiver was inferred as
  the wrong external type

Only the first two buckets should change the pack manifest. The last bucket
should change resolver/type facts instead, because adding an API fact there
would hide a misattribution.

Benchmark evidence should make the same distinction visible. A focused package
may still report constructor, method, or field pressure after external packs are
clean; before adding new pack facts, rerun it with plausible sibling GATK source
roots as `co_translate_source` or `source_summary` roots. If the pressure
becomes `source_dependency_resolved_*`, the right next action is to
co-translate, summarize, or intentionally exclude that source dependency. It is
not an external-library mapping problem. The benchmark now reports
`source_dependency_resolved_field_accesses` and
`top_source_dependency_field_accesses` alongside the existing method and
constructor counters, so GATK-shaped public-field reads such as
`allele.length` can be distinguished from true external field gaps. Field
accesses also have `top_source_dependency_field_contexts`, matching the method
and constructor context tables, so an inherited dependency field can be traced
from its declaring type to the primary-source member that consumed it.

Nested dependency types follow the same rule. A GATK file may import
`SomeDependency.Owner.Nested` and use the simple name `Nested` throughout a
graph rewrite or serialization workflow. If `Owner` is available through a
source-summary root, the resolver should attribute `new Nested(...)` and
`nested.getTarget()`-style calls to source dependency declarations. The
external pack layer must not claim the simple nested name just because it is
prominent in one package. The regression
`source_summary_root_resolves_imported_nested_graph_types` locks this in with a
FermiLite-shaped assembly graph example.

### Responsibility Boundaries

Resolver owns matching a Java call, constructor, field, nested type, generic
binding, or inherited member to an external pack entry.

The external fact layer owns the library pack manifest, semantic summaries,
coverage states, unsupported diagnostics, and crate/facade mapping facts.

Semantic analysis owns derived program facts that follow from using the
external API, such as a field becoming non-null after construction or a builder
receiver being mutated by a fluent call.

Slot shape and conversion planning own how values cross the boundary into Rust:
owned values, borrowed views, mutable borrows, cloned values, erased objects,
trait objects, facade wrappers, `Option<T>`, and `Result<T, E>`.

Lowering owns only applying the selected plan. It must not know that one Java
method is "really" a getter, builder, HDF5 handle, SQLite connection, VCF
header, or HTSJDK allele.

### Test Strategy For External Packs

External-pack tests should be smaller than GATK but shaped by GATK. For each
pack, add tests at these layers:

- resolver tests for external type, constructor, method, field, nested type,
  inheritance, and generic binding
- external fact tests for coverage state, mutability, nullability, element
  type, and exception facts
- semantic fact tests for builder chains, getter/setter summaries, initialized
  fields, and boundary effects
- slot-shape tests for direct values, optional values, collections, iterators,
  facade wrappers, and erased external objects
- conversion tests for Java call results flowing into Rust parameters, fields,
  returns, and collection terminals
- negative tests for unsupported APIs and unknown methods inside owned package
  patterns
- precedence tests proving user overrides beat pack facts

GATK file names may appear in test comments or benchmark reports as
motivation. Assertions should use neutral source packages unless the package
name itself is the behavior under test.

The test suite should close one library at a time. A library is "covered for
the current milestone" when focused tests prove the contract above, not when a
full GATK run happens to stop reporting diagnostics. The full benchmark is a
backstop and a mining tool; it is not the test oracle.

For each library selected for coverage:

1. Add a direct manifest/fact test proving the pack owns its package, selected
   types, constructor arities, nested fields, method groups, hierarchy facts,
   exception facts, and unsupported entries.
2. Add semantic-analysis tests using small Java snippets that resemble GATK
   usage but do not copy large GATK classes.
3. Add shape/coercion tests only for values that cross a boundary: fields,
   parameters, returns, `Option`, collections, iterators, builder results, and
   crate/facade values.
4. Add a negative test for a method, constructor, or field inside the owned
   pack that is not modeled, proving it produces a pack-owned coverage
   diagnostic instead of a generic fallback.
5. Add a precedence test proving a user fact can override at least one pack
   behavior without changing lowering code.
6. Run the narrow package or selected-pack GATK benchmark after tests pass, and
   record only aggregate movement and top remaining buckets.

The test target for a library is a workflow, not a method list. For example,
SQLite should be tested as "configure a connection, derive properties, create
a connection, prepare or create statements, execute, commit/rollback, close".
HDF5 should be tested as "open a handle, check presence, build/read arrays or
matrices, write, flush/close". VariantContext should be tested as "build a
variant, read alleles/genotypes/attributes, walk collection-like values".
The method list then falls out of the workflow and must still be captured as
manifest-owned coverage rows. This keeps coverage anchored in real usage while
avoiding a bespoke rule for each GATK call site.

## Dependency Decisions

### GATK Source

Mode: `co_translate_source`.

Translate GATK project source as the primary corpus. It is the benchmark for
resolver pressure, semantic facts, object strategy, and idiomatic Rust output.

GATK-owned helper packages outside the main root may also be co-translated when
source is available and the code is algorithmic or data-model code rather than
a runtime shell.

### HTSJDK

Mode: `external_fact_model` or `rust_crate_mapping` as an all-or-nothing
library boundary.

HTSJDK is used pervasively by GATK, but partial co-translation of a few visible
data-model packages risks creating an inconsistent boundary: some HTSJDK values
would be native translated Rust structs while adjacent HTSJDK values would
still be opaque external objects. For external libraries, prefer an all-or-
nothing policy. Unless HTSJDK is chosen as a separate full translation target,
treat the following as external:

- `htsjdk.variant.variantcontext`: alleles, genotypes, variant contexts, and
  builders.
- `htsjdk.variant.vcf`: metadata and header structures.
- `htsjdk.samtools.util`: intervals, locatables, utilities, and helpers.
- `htsjdk.tribble.Feature` and related interfaces.
- selected header and sequence dictionary models.

These packages still deserve rich external facts: nullability, builder roles,
collection element types, subclass/interface shape, exception behavior, and
slot-shape expectations. The important distinction is that those facts should
describe an external library boundary unless the whole HTSJDK translation mode
is deliberately changed.

SAM/BAM/CRAM codecs, readers, writers, indexes, compression, reference IO,
async IO, and native-backed performance code should also remain external by
default. Their Rust target should be an adapter over `rust-htslib`, `noodles`,
or a project facade rather than translated Java internals.

The first implementation target should still be multi-root and dependency-mode
support, but HTSJDK should initially enter through external fact packs or crate
facades, not selective co-translation.

### Picard

Mode: `external_fact_model` by default; selective `source_summary` or
`co_translate_source` only for small data/config classes.

Picard is adjacent project code, but much of it is command-line tooling and
domain plumbing. It should not become part of the GATK translation unless a
specific slice removes many conservative boundaries and is algorithmic enough
to produce useful Rust.

### Barclay

Mode: `external_fact_model` or `rust_crate_mapping`.

Barclay is command-line and argument infrastructure. Prefer mapping it to a
Rust CLI/config facade. Co-translation would mostly preserve Java reflection
and annotation patterns that Rust should not imitate.

### Java Platform

Mode: built-in language and standard-library facts.

`java.lang`, `java.util`, streams, functional interfaces, exceptions,
collections, `java.io`, and `java.nio` are not dependencies to co-translate.
They define Java semantics that the resolver, type system, standard-library
models, control-flow normalization, and conversion planner must understand.

### Guava And Apache Commons

Mode: `external_fact_model` plus `rust_crate_mapping`.

Map common collection, string, math, IO, compression, and utility behavior to
Rust standard types or crates. Add facts for factory methods, immutable
collections, predicates, optional-like values, math functions, and IO effects.
Do not translate these libraries.

### Logging

Mode: `external_fact_model`.

Logging calls should have side-effect facts and should usually lower to a Rust
logging facade. They should not influence ownership or object strategy except
through explicit boundary effects.

### Spark, Hadoop, DISQ, Scala

Mode: conservative external boundary or explicit runtime facade.

These are runtime frameworks. Do not co-translate them. If a project chooses a
Rust backend later, that should be a deliberate rewrite policy, not Java source
translation.

### Native And Runtime-Backed Libraries

Mode: `ffi_boundary` or `rust_crate_mapping`.

This includes Intel GKL, BWA JNI, Fermilite JNI, GenomicsDB, HDF5, SQLite,
HTTP clients, Kryo, Avro, protobuf-generated APIs, and compression backends.
Use typed facade facts for ownership, mutability, errors, and resource
lifetime. Do not translate implementation internals as part of GATK.

Preferred crate mappings:

- HDF5: map to `hdf5-pure-rust`
  (`https://crates.io/crates/hdf5-pure-rust`) where its API coverage is
  sufficient.
- SQLite: try `sqlite` (`https://crates.io/crates/sqlite`) first, but keep the
  mapping behind a facade so the crate can be changed if coverage or ownership
  semantics are not a good fit.

### Small Infrastructure Libraries

Mode: `external_fact_model` or `rust_crate_mapping`.

This includes OpenCSV, JSON/Jackson, Reflections, JGraphT, fastutil, config
libraries, and similar utilities. Prefer Rust crates such as `csv`,
`serde_json`, `petgraph`, or local facades when a real mapping is needed.
Reflection-heavy APIs should remain conservative unless overridden.

Apache Commons Lang remains an external fact-model pack for tuple-like values,
platform constants, exceptions, and mutable numeric helper objects. The
transformer-package slice now models the GATK-observed `MutableInt` surface
there: construction stays pack-owned, `doubleValue`/`getValue` are typed
read-only numeric getters, and `increment`/`decrement` are receiver-mutating
effects. The global external API-effect dispatcher includes this pack so those
method facts are visible to semantic analysis before conservative guesses.

JGraphT graph helpers are now modeled by a narrow `org.jgrapht` external fact
pack instead of stdlib constructor shortcuts. The pack owns `Graph`,
`SimpleGraph`, `DefaultDirectedWeightedGraph`, `ConnectivityInspector`,
`CycleDetector`, `IntegerComponentNameProvider`, and `DOTExporter`, and models
the GATK-shaped operations `addVertex`, `addEdge`, `setEdgeWeight`,
`vertexSet`, `connectedSets`, `detectCycles`, and `exportGraph`. The tests are
intentionally fixture-sized: they exercise graph construction and common
analysis/export calls inspired by GATK, without translating all of GATK or
hard-coding graph lowering policy.

Fastutil primitive collection helpers are now modeled by a narrow
`it.unimi.dsi.fastutil` external fact pack instead of stdlib constructor
shortcuts. The pack owns the GATK-observed primitive maps and lists
`Int2ObjectArrayMap`, `Int2ObjectOpenHashMap`, `IntArrayList`,
`Int2IntArrayMap`, and `Object2IntOpenHashMap`; it declares constructors and
common mutation/access APIs such as `put`, `get`, `getInt`, `add`, `size`,
`containsKey`, and `defaultReturnValue`. This keeps primitive collection
semantics available to analysis while leaving any future mapping to `HashMap`,
`Vec`, `indexmap`, or a facade as a later slot-shape/conversion decision.

The `org.json` object model is now represented by a narrow external fact pack
instead of stdlib constructor shortcuts. The pack owns `JSONObject`,
`JSONArray`, and `JSONTokener`, and models common object/array construction,
mutation, lookup, nullable optional lookup, length checks, and tokener
consumption. This gives the semantic layer mutability and nullability facts for
GATK-shaped JSON configuration snippets while leaving any future lowering to
`serde_json::Value`, typed structs, or a facade as a mapping decision.

Reflections scanning is now represented by a conservative `org.reflections`
external fact pack instead of a stdlib constructor shortcut. The pack owns the
`Reflections` scanner, models construction from a package/config argument, and
provides read-only facts for common discovery methods such as `getSubTypesOf`
and annotated member/type queries. The pack default remains conservative
because reflective discovery affects closed-world proofs and object strategy;
more precise lowering should come from overrides, source summaries, or a
deliberate Rust facade rather than broad method-name guesses.

## Implementation Impact

The resolver benchmark and CLI need to support multiple source roots:

- primary source roots that will be emitted
- dependency source roots that may be emitted
- dependency source roots used only for summaries
- external library pack manifests
- external fact files and generated dependency facts

The resolver must record provenance for every resolved dependency fact. The
same Java type may move between modes across translation runs, so downstream
stages should depend on facts, not on package-name checks.

The external API layer should represent dependency mode explicitly. A missing
source dependency or library pack should produce conservative external facts.
Adding a source root, source summary, external pack, crate facade, or user
override should improve facts without changing lowering logic.

Object strategy must be allowed to use co-translated dependency source when
proving closed families, enums, traits, or erased fallbacks. It must still
fall back when external construction, reflection, unknown implementors, or
runtime libraries prevent proof.

Slot shape and conversion planning should see the same domain shapes whether
they came from co-translated source, a source summary, an external pack, an
external fact file, or a Rust crate mapping.

GATK-owned helper constants should remain source facts when their helper
classes are co-translated or summarized, not external dependency-pack members.
Representative constants such as
`GATKConfig.CONFIG_FILE_VARIABLE_FILE_NAME`,
`ReferenceContext.UNINITIALIZED_REFERENCE_ID`,
`Slice.EMBEDDED_REFERENCE_ABSENT_CONTENT_ID`,
`ReadLikelihoodCalculationEngine.MAX_REPEAT_LENGTH`,
`ReadLikelihoodCalculationEngine.DEFAULT_EXPECTED_ERROR_RATE_PER_BASE`, and
`WDLWorkUnitHandler.POSITIONAL_ARGS` should resolve through source static
fields before conservative external fallback facts whenever those helpers are
available as source or source summaries. Fact-file/source-summary providers can
also carry these constants explicitly with `field Type.field JavaType` entries,
which resolver overload selection and semantic analysis consume ahead of
conservative fallback facts. The same fact-file path can carry declaration
constructor availability with `external-constructor TYPE ARG_TYPE...`, so a
source-summary provider does not need an external-pack manifest merely to
explain constructors for omitted helper declarations. Field facts are exact
member facts, not type-wide wildcards: a user/source-summary override for
`SummaryConstants.MAX_COUNT` must not accidentally type
`SummaryConstants.DEFAULT_NAME` or another helper member on the same class.
Dependency-mode benchmark coverage now verifies that configured source-summary
field and constructor facts explain absent declarations without unresolved
fallback accounting.

The representative helper constants above are no longer builtin external field
models. They remain covered by source-backed resolution when the helper classes
are co-translated and by explicit source-summary facts when helper classes are
absent. The same migration pattern should be used for any later GATK-owned
helper constants before adding new GATK-specific library pack behavior.
Resolver expression inference must carry the active fact set recursively, so
primitive and object-valued source-summary constants affect overload selection
inside nested field accesses, array accesses, ternaries, and call arguments.
The focused resolver regression
`resolver_threads_configured_field_facts_through_nested_expressions` locks in
both responsibilities with GATK-shaped summary constants while avoiding a
dependency on the full GATK corpus.

## Tests To Add

Add tests that cover the dependency-mode mechanism without overfitting to GATK:

1. A multi-root resolver test where an app source imports a dependency source
   class and resolves constructors, fields, inherited methods, and generic
   return types from that dependency.
2. The same app source without the dependency root, proving the call becomes an
   external conservative boundary with structured diagnostics.
3. A source-summary test where declarations from a dependency are available but
   method bodies are not emitted.
4. An external-pack ownership test where calls inside an owned package are
   resolved through pack facts rather than global method-name guesses.
5. An unsupported-entry test where an owned package API produces a structured
   diagnostic instead of falling through to lowering.
6. A precedence test where a user override beats co-translated source,
   source summaries, external packs, crate mappings, and built-in facts.
7. A package-neutral domain model test where a dependency interface and its
   implementations can be considered for enum or trait representation when
   closure is proven.
8. A crate-mapping test where an external library method produces the same slot
   and conversion plan as an equivalent co-translated helper.

Real GATK or HTSJDK snippets may be kept as benchmark references, but the main
tests should use neutral package names such as `app`, `dep.model`, and
`dep.io`.

Current coverage status:

- Items 1 and 2 are covered by `tests/dependency_mode.rs`: the same neutral app
  fixture resolves dependency constructors, fields, inherited methods, and
  generic return types when the dependency root is configured, and falls back
  to external-boundary facts when that root is omitted.
  `tests/resolve_calls.rs` also contains a GATK-shaped source-constant
  regression proving that co-translated GATK helper constants resolve from
  source static fields before unresolved external field fallback facts.
- Item 3 is covered by configured `SourceSummary` fact providers and by a real
  `SourceSummary` Java source root in `tests/dependency_mode.rs`; summary-root
  declarations participate in the resolver source index while summary files
  are not benchmarked as emitted source. Source-summary roots now also emit
  declaration facts for hierarchy and object-strategy consumers, including
  type capability, interface object-safety, and sealed-closure facts, and
  resolver benchmark reports expose those facts through `source_summary_facts`
  and kind-specific source-summary root counters for type capability,
  object-safety, and closed-hierarchy facts.
  Benchmark reports also count resolved method, constructor, and field uses of
  dependency source roots, so co-translated and source-summary roots are
  measurable as resolution providers rather than only as file-count entries.
  Source-dependency field attribution follows superclass declarations and uses
  a shared raw/normalized field lookup, so GATK-shaped reads through subclass
  receivers can be reported against the declaring dependency type instead of
  falling back to an external field gap. The field context table also records
  receiver type, enclosing type/member, and sample files, which gives source
  co-translation/source-summary decisions the same audit trail as method and
  constructor pressure. The source-summary-root regression now asserts the
  same path for direct and inherited dependency fields while still excluding
  summary method bodies from emitted-file benchmarking.
- Items 4 and 5 are covered by `tests/external_packs.rs` through pack-owned
  method/constructor/field facts and unsupported/missing API diagnostics.
- Item 6 is covered for the active external fact paths: user overrides beat
  external pack facts, Rust crate-mapping facts, and built-in external facts.
  `EffectiveFacts` also covers user override precedence over parsed source
  hierarchy/object facts. `tests/dependency_mode.rs` now also covers that
  precedence through the dependency-mode benchmark path: a user representation
  override beats closed-enum strategies inferred from co-translated dependency
  source and from source-summary dependency declarations.
- Item 7 is covered by a neutral dependency-mode fixture in
  `tests/dependency_mode.rs`: a primary app root plus a co-translated
  dependency root resolves the dependency source and lets the object model
  choose a closed enum for a sealed dependency interface. The same test module
  now covers a source-summary root with the same sealed-interface shape,
  proving summary declarations can drive object representation evidence
  without benchmarking or emitting dependency bodies.
- Item 8 is covered at the conversion-planner boundary in
  `tests/type_coercion.rs`: a Rust crate-mapped HDF5 handle and an equivalent
  co-translated helper type both use the same proven clone-recovery slot
  conversion policy. External API effect facts now also carry optional return
  types, and `tests/type_coercion.rs` verifies that HDF5 and VariantContext
  pack return-type facts feed ordinary unresolved-call type inference.
  `tests/slot_shape_design.rs` also verifies that HDF5 crate-mapped method
  calls and constructors are planned as crate/facade targets and render
  through `hdf5_pure_rust::...`, while user overrides keep precedence over
  crate mapping for both receiver-call behavior and constructor facade-symbol
  selection. Crate/facade symbol selection is now also a dedicated fact
  shape: mapped external-pack APIs and constructors emit facade mapping facts,
  and user-authored pack manifests can override Java snake-case defaults with
  `rust=...` and `rust-type=...` metadata. Raw external fact files can also
  carry `external-crate-api` and `external-crate-constructor` symbol overrides;
  the crate target remains owned by the Rust-crate-mapping pack boundary. Call
  planning consumes those facts instead of letting lowering rediscover HDF5,
  SQLite, or any other mapped dependency by Java spelling. Call argument
  planning also consumes external
  argument-effect facts for unresolved external calls: complete argument-effect
  rows shape borrowed strings, borrowed sequences, mutable arguments, and copy
  scalars before lowering, while incomplete rows intentionally leave the
  argument vector unshaped. User external fact files can now express the same
  boundary evidence with `external-argument`, so source summaries and manual
  overrides do not need an external-pack manifest just to model ownership or
  lifetime pressure for a call parameter.
- External API effect facts also carry thrown exception types. The HDF5
  crate-mapping pack models selected read/write/close calls as throwing
  `HDF5Error`, and semantic analysis propagates those external throws into
  the enclosing source method effect so exception/result planning can consume
  them without lowering knowing about HDF5.
- User-authored external fact files can express the same richer API summaries
  with `return=Type` and `throws=TypeA,TypeB` metadata on `external-api`
  entries, keeping pack facts and injected override facts on the same shape.
  External pack manifests also declare exception classes with checked,
  unchecked, or unknown kind, so API `throws=` metadata and exception/result
  planning share pack-owned exception facts.
- Built-in HDF5 and SQLite crate-mapped method behavior is now declared as
  manifest-owned API effect entries and generated through the same boundary
  application path as user-authored `external-pack-api` facts. This starts
  removing duplicated procedural pack summaries for crate/facade mappings.
  Follow-up benchmark mining added GATK-shaped HDF5 facade facts for scalar
  double reads, string-array construction helpers, string-array reads, and
  presence predicates (`readDouble`, `makeStringArray`, `readStringArray`, and
  `isPresent`), keeping those mappings in the HDF5 pack rather than in
  lowering.
- The `htsjdk.tribble.Feature`, `htsjdk.variant.vcf`, `htsjdk.samtools.util`,
  and selected SAM header/dictionary packs also declare their selected getter,
  boolean-getter, indexed-getter, side-effect, mutator, and setter effects as
  manifest-owned API effect entries, proving the same boundary path works for
  HTSJDK external fact-model packs and not only crate mappings. Follow-up
  benchmark mining added SAM header and samtools-util rows for sequence MD5,
  reference length, header sequence index, read-group platform/flow-order
  metadata, and Histogram label/indexed lookup methods as receiver-specific
  pack facts.
- The `htsjdk.variant.variantcontext` migration has started at the Allele and
  VariantContext value sub-surfaces: selected typed Allele getters/predicates,
  VariantContext getters/predicates, and nullable attribute accessors are now
  manifest-owned API effects. VariantContextBuilder and GenotypeBuilder fluent
  mutators and `make` methods are also manifest-owned API effects. Genotype
  value getters/predicates and GenotypesContext sequence/index/stream accessors
  are likewise manifest-owned typed API effects, so the pack no longer uses
  dependency-specific procedural loops to generate selected method summaries.
  The next high-volume GATK-inspired VariantContext/Genotype method gaps from
  the benchmark report, including sample genotype lookup, integer attributes,
  alternate-allele indexing, filter/source/display accessors, genotype depth,
  genotype indexed alleles, and builder attribute removal, are now explicit
  manifest-owned typed effects as well. A follow-up pass also models the next
  receiver-specific `variantcontext` rows: allele counts, allele symbolic and
  display helpers, variant/sample/contig/quality accessors, genotype DP and
  likelihood helpers, builder stop reads, and GenotypesContext mutation. The
  current manifest also covers the next neutral external-boundary slice:
  VariantContext genotype-presence, allele-index, string-list attribute, and
  Genotype phase/hom-var/likelihood predicates, plus GenotypeLikelihoods and
  GenotypeType factory-style APIs. Inherited Java `Object` methods such as
  `equals(Object)`, `hashCode()`, `toString()`, and `getClass()` remain builtin
  facts even when the receiver is pack-owned. A later receiver-specific
  refresh added GenotypesContext create/copy/sample-name helpers, VariantContext
  object/double list attributes, common-info and max-ploidy getters, Allele
  base comparison, and VariantContextBuilder bulk attribute removal as pack
  facts. The next external-pack helper refresh added Histogram count/mutation
  helpers, Genotype heterozygosity/attribute/filter helpers, builder filter and
  genotype getters, GenotypeBuilder quality mutation, likelihood index helpers,
  and GenotypesContext replacement as receiver-specific pack facts. The latest
  receiver-specific helper refresh added VCF header metadata/dictionary
  helpers, Histogram statistic getters, VariantContext aggregate count getters,
  Allele extension, GenotypeBuilder quality/filter cleanup mutators, and SAM
  program-record lookup as pack facts. The remaining top rows are now mostly
  policy decisions rather than obvious pack omissions: Java object methods
  such as `Allele.equals`, validation/assertion helpers, structural variant
  predicates, and selected dictionary/header comparator helpers. A follow-up
  predicate/helper refresh modeled the clear semantic subset of those rows:
  VariantContext filter, type, overlap, and quality helpers, VariantContext
  builder unfiltering, Genotype mixed-state predicates, GenotypeBuilder
  attribute cleanup, VCF comparator/version helpers, SAM header string
  rendering, and Histogram key access. Remaining rows are now centered on
  list-valued attribute access, Java object equality, validation/assertion
  policy, genotype-likelihood vector conversions, and a few HDF5/SAM utility
  helpers. The latest vector/matrix helper refresh moved the clear subset of
  those into manifest-owned facts: list-valued int attributes,
  genotype-likelihood vector and PL helpers, GenotypesContext bulk mutation,
  Locatable containment/length helpers, Histogram prefill, SAM sequence-name
  truncation, and HDF5 library load/matrix reads. The remaining top rows are
  now mostly VCF metadata ordering, allele/variant membership predicates,
  pass-filter builder policy, validation/assertion policy, dictionary
  equivalence, and enum/object method handling. The latest membership/metadata
  refresh moved the clear subset of those into manifest-owned facts after
  checking the local HTSJDK source: VCF sorted metadata and genotype-sample
  counts, VariantContext ID/allele/membership/indel-likelihood helpers,
  VariantContextBuilder pass-filter/end-computation mutators,
  GenotypeLikelihoods PL construction, GenotypesContext sample membership, and
  SAM sequence-dictionary mutation. Remaining high rows are now dominated by
  validation/assertion/equality policy, explicit unsupported APIs, selected
  allele predicates, and a few low-count header/comment helpers.
- Boundary cleanup continues by moving static HTSJDK/VCF/SAM constants out of
  generic stdlib tables into the packs that own their receiver types. The
  variantcontext pack now owns `GenotypesContext.NO_GENOTYPES`, so generic
  stdlib only supplies a conservative unknown-field fallback if that pack is not
  selected. It also owns the HTSJDK `JexlMissingValueTreatment` enum constants
  used by `JEXLMap` and `VariantContextUtils`. The SAM reader pack now owns
  `ValidationStringency` enum constants (`LENIENT`, `SILENT`, `STRICT`) for
  the same reason, and the CRAM pack owns `DataSeriesType.BYTE_ARRAY` instead
  of treating it as a generic named enum constant. The BigQuery Storage API
  pack owns `DataFormat.AVRO`/`ARROW`, matching the observed
  `com.google.cloud.bigquery.storage.v1.DataFormat` import in the GVS
  `StorageAPIAvroReader` snippet. A narrow Jackson databind pack owns
  `MapperFeature.ACCEPT_CASE_INSENSITIVE_PROPERTIES` and
  `DeserializationFeature.UNWRAP_ROOT_VALUE`, replacing the last generic
  stdlib enum shortcuts from this GATK static-constant slice. The same boundary
  rule now applies uniformly to provenance: exact external-pack API,
  constructor, and field facts take precedence over reusable stdlib shortcuts.
  Generic stdlib still infers return shapes and shared constants for unlinked
  or ambiguous libraries, but no longer claims ownership for pack-modeled
  getters such as `Allele.getBases`, `SAMRecord.getReadName`,
  `ReferenceSequence.getBases`, `Feature.getContig`, or `Locatable.getStart`;
  pack-modeled fields such as `SamReaderFactory.Option.CACHE_FILE_BASED_INDEXES`,
  `QueryJobConfiguration.Priority.BATCH`, `HDF5File.OpenMode.READ_ONLY`, or
  `VCFHeader.REFERENCE_KEY`; or pack-modeled constructors such as
  `VariantContextBuilder()`, `HDF5File(File)`, and `SQLiteConfig()`.
  Ambiguous bare enum constants such as `Option.CACHE_FILE_BASED_INDEXES`,
  `Attribute.PRIMITIVE`, and `Priority.BATCH` remain shared stdlib fallbacks
  because the narrow packs deliberately do not own those bare receiver names.
- Current built-in external packs now have explicit per-method,
  per-constructor, per-field, and per-object-strategy manifest coverage entries
  for modeled APIs and representation policy. They also declare type
  capability facts for clone/final proofs, sequence element shapes,
  implements/extends links, closed-hierarchy facts, broad object-safety
  assumptions, and detailed object-safety violation facts. Tests compare those
  entries against the pack-owned method, constructor, field, hierarchy
  override, closed-hierarchy, object-safety, and type capability facts, so
  adding a new modeled member, facade/object
  strategy, or element/capability rule requires an intentional manifest
  classification instead of relying only on pack-level default coverage. The
  built-in-pack coverage regression now iterates the manifest
  registry and selects each pack by manifest name, so newly added built-in
  packs enter both the manifest coverage audit and pack-boundary coherence
  audit automatically instead of being added to a hand-curated test list. This
  means a new built-in pack must be selectable by name, must explicitly
  classify its modeled APIs, constructors, and fields, and must not emit
  external-library-pack facts for types outside its declared ownership
  boundary. A dedicated manifest regression test now verifies that coverage
  lookup is symmetric for APIs, constructors, and fields, including explicit
  unsupported entries and default conservative fallback. The external-pack
  layer also exposes a reusable manifest/fact coverage audit that reports
  pack-owned API, API-effect detail, argument-effect, constructor, field,
  field-nullability, object-strategy, type-capability, exception,
  crate/facade mapping, and unsupported-member facts that lack an explicit
  manifest coverage entry or disagree with the manifest row they came from.
  Closed-hierarchy and detailed object-safety facts are now included in the
  same audit: a pack-owned closed-hierarchy or object-safety fact must have a
  matching manifest row, and stale open/closed values or violation details are
  reported as manifest drift.
  Unsupported facts are audited as exact manifest outputs, not just as generic
  unsupported coverage: APIs must preserve the manifest reason text, and
  unsupported constructors/fields must match the inline unsupported row that
  generated their structured diagnostic.
  For `rust-crate-mapping` packs this audit verifies that emitted method and
  constructor mapping facts match the manifest's declared receiver/type,
  arity, Rust type, and Rust function symbol, so HDF5/SQLite-style facade
  mappings cannot drift from the pack contract silently.
  Resolver benchmark reports now include an
  `external_pack_manifest_coverage_diagnostics` counter and top diagnostic
  table for built-in and configured external-pack provider audits. That top
  table carries the built-in pack or configured provider mode, so manifest
  drift is attributable to `ExternalFactModel`, `RustCrateMapping`,
  `FfiBoundary`, or `ConservativeBoundary` benchmark modes. Built-in benchmark
  audits run both manifest-coverage and pack-boundary coherence checks.
  Configured providers are audited by their actual facts even when
  their provider name matches a built-in pack, so manifest/fact drift and
  accidental facts outside a pack's declared ownership boundary are visible in
  GATK comparison runs rather than only in unit tests. This includes
  `builtin-pack:NAME` providers loaded through the CLI and direct providers
  named after a built-in pack; the audit uses the provider's actual facts, not
  regenerated built-in facts. Provider-name normalization for these selected
  built-in audits is owned by `external_packs` through a narrow manifest lookup
  API, so benchmark/reporting code does not duplicate pack-name parsing or scan
  manifest lists directly. The audit also tightened
  smaller helper packs by adding explicit
  modeled API coverage for Picard program groups, Avro parser helpers, Scala
  tuple accessors, Guava immutable map builders, and Broad HTTP retry
  settings.
- User-authored external pack manifests now have a line-oriented parser for
  package ownership, mode, default coverage, Rust target hooks, modeled API
  effects, modeled constructors, modeled fields, object representation
  strategies, type capabilities, field nullability, and unsupported entries.
  The parser emits the same `FactSet` shapes as built-in packs, so the
  resolver and semantic layers consume external-library-pack provenance
  without lowering package-name checks. The CLI accepts these manifests through
  repeatable
  `--external-pack-manifest` inputs and merges them with `--facts` overrides.
  Dependency-mode configs can also register parsed manifests as fact providers
  with mode inferred from the manifest's external-library-pack boundaries, and
  resolver benchmark reports count those provider facts including pack
  boundaries and unsupported entries. Provider-mode inference is intentionally
  one-mode-per-provider: manifests with no `external-pack` boundary row or
  with mixed external-library modes are rejected before benchmark registration.
  Mixed dependency strategies should be split into separate manifest/provider
  inputs instead of being hidden inside one pack. The `resolver-bench` CLI accepts the
  same `--facts` and `--external-pack-manifest` inputs and registers them as
  benchmark fact providers. Benchmark configs and the `resolver-bench`
  `--no-builtin-external-packs` flag can disable built-in external packs, so
  baseline, explicit HTSJDK-pack, and crate/facade-pack benchmark modes can be
  measured without hidden built-in pack attribution. The `resolver-bench`
  `--builtin-external-pack NAME` option can re-add selected built-in packs as
  ordinary fact providers, which makes the benchmark modes directly runnable
  without changing lowering or semantic-analysis package checks. Parsed
  external pack manifests now also participate in pack-owned missing and
  unsupported diagnostics for methods, constructors, and fields. Unsupported
  constructor and field rows now have their own fact shapes, so a pack can
  distinguish an intentional runtime/native boundary from an unmodeled
  coverage gap instead of reporting both as missing coverage. User-authored
  packs get the same "do not silently fall through to generic guesses"
  behavior as built-in packs. Resolver benchmark reports now attribute
  unsupported external methods, constructors, and fields as separate
  pack-owned counters and top summaries, so intentional external-boundary
  gaps are measurable without being mixed into generic unexplained targets.
  Parsed manifests are also audited for boundary coherence: API effects,
  argument effects, constructors, fields, field nullability, object strategy,
  type capability, exception, unsupported rows, and crate/facade mapping facts
  whose receiver/type is not owned by the declared pack boundary now produce
  structured manifest diagnostics. This keeps user-authored manifests from
  becoming one-method escape hatches that bypass the all-or-nothing pack
  boundary rule. Parsed manifests can now also emit detailed object-safety
  violation facts with external-pack provenance; a direct representation
  regression proves those facts reject a dyn-trait facade request and fall back
  to erased object representation without inspecting package names. A neutral
  runtime-adapter regression also proves a parsed manifest can supply
  erased-object superclass policy and type capabilities as dependency-provider
  facts: a source serializer extending `dep.runtime.Serializer<T>` stays on
  the explicit external-pack object boundary with no lowering package-name
  checks.

## First External Pack Pilot: HTSJDK VariantContext

The first library pack to cover should be `htsjdk.variant.variantcontext`.
This is a good pilot because GATK uses it heavily, the domain is central to the
translated program, and the APIs stress the boundaries we care about:
builders, immutable-ish value objects, collection element shapes, nullable
attributes, typed constants, equality, exceptions, and interface-like domain
values.

This pilot should not attempt to translate all of GATK. It should build a
small suite of GATK-inspired fixtures and use the GATK benchmark only to choose
call shapes and measure progress.

Current status: this pilot is already the best place to close the first
library because `tests/external_packs.rs` contains direct and semantic coverage
for the core VariantContext pack. The existing tests prove pack ownership,
builder and getter effects, constructor/field provenance, exception hierarchy,
sequence-like `GenotypesContext`, Map.Entry non-misattribution, and several
GATK-shaped calls. The next work is not to add a second pilot; it is to finish
VariantContext as a test-defined coverage slice and document the remaining
gaps as pack-owned, unsupported, or intentionally conservative.

The first closure fixture now covers a builder/read flow inspired by common
GATK VariantContext usage: `Allele.create`, `GenotypeBuilder`,
`GenotypesContext.create/add`, fluent `VariantContextBuilder` mutation,
`make`, scalar getters, nullable attributes, `getGenotypes`, and indexed
`getAllele`. That fixture intentionally found and closed a missing
`VariantContext.getAllele` pack entry, demonstrating the intended workflow:
test a GATK-shaped slice first, then add a narrow pack-owned fact.

The follow-up closure fixtures cover the boundary and override parts of the
same pilot. `tests/slot_shape_design.rs` now verifies that external helper
calls preserve facade shapes for `VariantContext`, `Allele`, `Genotype`, and
`GenotypesContext` through parameters, locals, returns, and external method
facts. `tests/external_packs.rs` now also verifies that a user-authored
`VariantContext.getAttribute` override is consumed by semantic analysis while
the call remains inside the pack-owned boundary. Existing unsupported/missing
diagnostic tests cover `VariantContext.fullyDecode` and unmodeled owned-pack
methods.

The read-flow fixture now covers scalar getters, enum-like type access,
string-list attributes, collection getters, and predicates in a small neutral
source file. It closed three fact-layer gaps: `VariantContext.getID` is a
pack-owned non-null string getter, `VariantContext.getType` returns the
pack-owned `Type` enum rather than a string, and `Type.name()` is a pack-owned
non-null enum-name getter. `VariantContext.getAttributeAsString` remains
maybe-null, keeping nullable attribute behavior conservative where HTSJDK
attribute contents are not proven.

The genotype-flow fixture now covers the other high-value VariantContext
domain path without using a large GATK class: `GenotypeBuilder` fluent quality
mutators and `make`, `GenotypesContext.create/add/replace/copy`, sample lookup
through `VariantContext.getGenotype`, indexed `GenotypesContext.get`, iterator
and stream-like accessors, and the `GenotypesContext` sequence element shape.
This confirms the pack facts support genotype workflows without treating Java
collection APIs as VariantContext-owned behavior.

The focused `tools/walkers/variantutils` GATK benchmark now satisfies the
VariantContext pilot acceptance signal: `unexplained_method_calls` is zero,
`external_pack_manifest_coverage_diagnostics` is zero, and the remaining
VariantContext method diagnostic is the deliberate unsupported
`VariantContext.fullyDecode` boundary in `SelectVariants.java`. The benchmark
also forced `VariantContext.Type.values` into the pack as a non-null
`Type[]` enum helper. The VCF constants that then surfaced in this package,
including `VCFConstants.DEPTH_KEY`, `ALLELE_NUMBER_KEY`, and
`INFO_FIELD_ARRAY_SEPARATOR`, are now owned by the `htsjdk.variant.vcf` pack
as typed field facts rather than VariantContext lowering rules or generic
stdlib constants. The focused package now has zero unexplained method,
constructor, and field accesses; its only semantic diagnostics are the two
deliberate `VariantContext.fullyDecode` unsupported-boundary reports.

## Crate-Mapping Pilot: SQLite

SQLite is the smaller crate-mapping pilot. It is useful because it exercises a
runtime-backed external library without the domain breadth of HTSJDK: mutable
configuration, resource handles, statement objects, checked errors, facade
mapping, and all-or-nothing pack ownership.

The selected boundary is `org.sqlite` plus the simple names that Java parsing
and imported source commonly expose:

- `SQLiteConfig`
- `SQLiteConnection`
- `SQLiteStatement`
- `SQLException`

The target Rust boundary is a facade over the `sqlite` crate, not direct
lowering rules. The pack owns constructor and method facts, exception facts,
object representation fallback facts, and crate/facade mapping facts. Lowering
should only see the resolved call plan and facade mapping, not the Java package
name.

The current milestone coverage workflow is:

- construct `SQLiteConfig`
- set configuration flags such as read-only, busy timeout, shared cache, and
  journal mode
- convert local configuration to `Properties`
- create a `SQLiteConnection`
- prepare or create a `SQLiteStatement`
- execute statements through `execute`, `executeUpdate`, and `executeQuery`
- bind positional parameters for string, integer, and floating-point values
- read a `SQLiteResultSet` with `next`, nullable string getters, numeric
  getters, and close
- commit, rollback, and close the connection
- propagate `SQLException` as the declared checked boundary error

The focused tests are intentionally GATK-shaped rather than GATK-specific.
`tests/external_packs.rs` proves direct facts and semantic effects for the
SQLite pack, including constructor ownership, return types, receiver mutation,
checked exception propagation, and missing method/constructor/field diagnostics
inside the owned boundary. `tests/dependency_mode.rs` now adds a resolver
benchmark fixture that disables built-in packs, re-adds only the SQLite pack,
and verifies that the workflow above is explained as `rust_crate_mapping`
method and constructor coverage with zero manifest diagnostics.

The SQLite pilot now also covers the first result-set and parameter-binding
slice. The pack owns `SQLiteResultSet` and `SQLException`, maps statement
binding methods and result-set iteration/getters as crate/facade calls, and
declares borrowed non-null argument effects for bind indices, bind values, and
result-set lookup keys. The richer result-conversion follow-up has started with
`SQLiteResultSet.getLong(int)` and `SQLiteResultSet.getBoolean(String)`, so
numeric, boolean, and nullable string reads all flow through the same
crate-mapping facts. Semantic analysis imports built-in pack argument effects
alongside API effects, so ownership and lifetime planning can consume these
parameter facts without adding SQLite-specific lowering rules.
`tests/slot_shape_design.rs` also verifies that these result-set and binding
calls are selected by call planning as `sqlite::...` facade targets and render
through those targets rather than as Java-style receiver methods. The same
call-plan test proves that `bindString(1, value)` is shaped from pack facts as
a copied integer index plus a borrowed `&str` value before lowering; calls
without a complete modeled argument vector remain unshaped.

Resolver benchmark provider rows expose crate API and constructor mapping
facts separately from generic API, constructor, field, and unsupported facts.
That makes crate/facade coverage auditable for user-authored manifests and
built-in pack providers: a mapped SQLite or runtime boundary must show both
external semantic facts and explicit `external_crate_*_mapping_facts` instead
of hiding facade decisions in the provider total. Benchmark reports also expose
configured-provider aggregate counters for the same fact kinds, so a run that
loads several selected providers can be compared without scraping individual
provider rows. Source-summary providers expose crate API and constructor
mapping counters as well, making raw fact-file facade overrides auditable even
when they are not wrapped in an external-pack manifest. Field-nullability facts
and exception facts are included in the
same provider audit surface because they directly affect struct optionality and
checked-error lowering decisions later in the pipeline. Hierarchy override,
closed-hierarchy, object-safety, and type-capability facts are also exposed
there, keeping object representation and clone/finality/sequence-shape policy
auditable as dependency facts instead of as lowering-specific assumptions.
Provider-supplied analysis facts such as captures, trivial accessors, method
effects, recursion, control-flow rewrite markers, dispatch requirements, and
catch facts are counted as well. These facts guide signature, ownership,
control-flow, and exception planning, so they must stay visible as dependency
inputs instead of disappearing into a raw provider total.
Captures, method effects, trivial accessors, recursion, control-flow markers,
dispatch requirements, and catch facts are now also line-oriented external
facts (`capture`, `method-effect`, `trivial-getter`, `trivial-setter`,
`recursion`, `control-flow`, `dispatch-requirement`, and `catch`), so a
GATK-inspired source summary can report body-derived planning pressure without
choosing the Rust representation or lowering strategy itself. The fact
boundary owns parsing and provenance; object strategy, signature projection,
slot-shape planning, control-flow planning, call planning, and exception/result
planning own the consequences.
Unsupported method, constructor, and field boundaries are also expressible in
raw fact files (`unsupported-api`, `unsupported-constructor`, and
`unsupported-field`) and are counted for configured and source-summary
providers. That keeps "known but unsupported" evidence auditable even when it
comes from a source-summary provider instead of an external-pack manifest.

Acceptance for the SQLite pilot is narrower than "translate every SQLite use
in GATK":

- modeled workflow calls are attributed to the SQLite pack
- mapped calls and constructors emit crate/facade mapping facts
- checked errors are visible as semantic method effects
- unmodeled owned members produce pack-owned diagnostics
- no SQLite-specific behavior is implemented in lowering or generic stdlib
  shortcuts

Future SQLite work should continue to start from new GATK-inspired workflow
tests before adding API facts. The first transaction-configuration follow-up is
covered by `SQLiteConnection.setAutoCommit(boolean)`: it is a checked,
receiver-mutating crate-mapped setter with a modeled boolean argument boundary,
and the neutral SQLite workflow now attributes it to the SQLite pack. The first
richer result-conversion follow-up is also covered by checked crate-mapped
`getLong` and `getBoolean` facts. Prepared-statement lifecycle policy is now
started with checked crate-mapped `SQLiteStatement.clearParameters()` and
`SQLiteStatement.close()` facts, keeping parameter cleanup and handle cleanup
inside the facade boundary. Database metadata is now covered on the cooperating
`java.sql` side with `Connection.getMetaData()`,
`DatabaseMetaData.getDatabaseProductName()`, `getTables(...)`, and
`getColumns(...)` facts. Generic JDBC scalar result conversion is now covered
with `ResultSet.getInt(...)`, `getLong(...)`, `getDouble(...)`, and
`getBoolean(...)` facts next to the nullable `getString(...)` facts. The
primitive conversion companion `ResultSet.wasNull()` is also modeled as a
checked boolean getter, so null-detection policy is visible to later conversion
planning rather than hidden in lowering. The same all-or-nothing boundary
contract is now locked in for the cooperating `java.sql` facade:
`missing_java_sql_pack_members_report_structured_diagnostics` proves that
import-qualified JDBC `Connection` and `ResultSet` methods, fields, and
interface-style constructor attempts that are not modeled produce pack-owned
missing-member diagnostics instead of generic external guesses. The matching
selected-provider benchmark regression,
`resolver_benchmark_java_sql_pack_reports_missing_owned_members`, verifies the
same method/constructor/field gaps are counted as `ExternalFactModel`
missing-member rows with zero generic unexplained calls. A prepared-statement
binding follow-up now adds checked, receiver-mutating `setLong` and
`setBoolean` facts with modeled index/value argument shapes. The direct
semantic test and selected-provider benchmark both exercise these alongside
`setString`, `setInt`, and `setDouble`, keeping common JDBC parameter binding
inside the same fact layer. A result-set metadata follow-up now adds checked
indexed facts for `ResultSetMetaData.getColumnType`,
`getColumnTypeName`, and `getColumnClassName`, with modeled column-index
argument shapes. A primitive result-conversion follow-up now covers the
adjacent checked indexed getters `ResultSet.getFloat`, `getShort`, and
`getByte` next to the existing `getInt`, `getLong`, `getDouble`, and
`getBoolean` facts, including borrowed index/label argument boundaries.
The object/blob/temporal conversion follow-up covers nullable checked
`ResultSet.getObject`, `getBytes`, `getDate`, and `getTimestamp` facts, with
`Date` and `Timestamp` owned as erased java.sql values. The column-property
metadata follow-up covers checked `ResultSetMetaData.isNullable`, `isSigned`,
`isAutoIncrement`, `getPrecision`, `getScale`, and
`getColumnDisplaySize` facts with modeled column-index arguments. Heavyweight
handle-valued result conversions are now explicit unsupported java.sql
boundaries: `ResultSet.getBlob`, `getClob`, `getArray`, and `getSQLXML`
produce pack-owned unsupported diagnostics instead of generic missing-method
rows until a streaming/resource facade policy exists. Remaining candidate
workflows are JDBC conversions and metadata surfaces that need more than
nullable value reads, scalar column properties, or explicit unsupported
resource-handle boundaries. If those workflows require an API shape that does
not map cleanly to the `sqlite` crate, the facade boundary should absorb that
mismatch rather than leaking Java ownership or lifetime decisions into
lowering.

The first follow-up workflow is the COSMIC-style SQLite/JDBC split. GATK uses
`org.sqlite.SQLiteConfig` to set read-only configuration, then calls
`DriverManager.getConnection(..., config.toProperties())`, and queries through
generic `java.sql.Connection`, `java.sql.Statement`, `java.sql.ResultSet`, and
`java.sql.ResultSetMetaData`. The implementation keeps this as two cooperating
packs:

- `sqlite` owns `SQLiteConfig`, its constructor, read-only configuration, and
  `toProperties` as a crate-mapped facade boundary.
- `java.sql` owns generic JDBC connection factories, statement creation,
  prepared-statement parameter binding, query/update execution, result-set
  iteration, scalar result conversion, result-set metadata lookup, database
  metadata lookup, primitive-result null checks, close operations, and checked
  `java.sql.SQLException` facts as a semantic external fact model.
- `DriverManager.getConnection` includes the explicit normalized static-receiver
  alias `driver_manager` because the current unresolved static-call path uses
  that key in benchmark attribution.
- The parser qualifies `java.sql` wildcard and direct imports for the selected
  JDBC types before semantic pack matching. This lets the pack explain COSMIC's
  simple `Connection`/`ResultSet` declarations while not claiming unrelated
  project-domain `Connection` types or `java.lang.Class` reflection calls.

The tests intentionally do not translate the full COSMIC factory. They use a
neutral fixture that configures SQLite, opens a generic JDBC connection, reads
database metadata, executes a query, reads result metadata, performs a prepared
update with string, integer, and floating-point parameter binding, iterates
rows, performs nullable string plus integer/long/double/boolean result
conversion, checks primitive-result null state with `wasNull()`, and closes
resources. The dependency-mode benchmark test disables built-ins, re-adds
`sqlite` alone to prove only SQLite configuration calls are explained, then
re-adds `java.sql` to prove the generic JDBC calls are explained by the
separate semantic pack. This is the pattern for future external-library work:
one reusable workflow fixture first, corpus benchmark second, and no lowering
branches for library-specific ownership or lifetime behavior.
Prepared-statement binding, result conversion, primitive-result null checks,
and metadata lookup are deliberately modeled with ordinary external
argument-effect and API-effect facts, so ownership and lifetime planning
consume parameter shape from the same fact layer used by HDF5, SQLite, and
HTSJDK instead of rediscovering JDBC calls later.

### Initial Pack Boundary

The pilot owns the `htsjdk.variant.variantcontext` package family as an
external pack. The initial manifest should include at least:

- `Allele`
- `VariantContext`
- `VariantContextBuilder`
- `Genotype`
- `GenotypeBuilder`
- `GenotypesContext`
- common enum or constant-like types used by those APIs
- selected exception types thrown by the package

Every public entry in the selected manifest must be classified as `semantic`,
`mapped`, `conservative`, or `unsupported`. Missing entries inside the owned
package should produce a pack-owned diagnostic.

The pilot boundary should also stay adjacent to, but separate from, these
packs:

- `htsjdk.variant.vcf`: header lines, readers, codecs, writer builders, and
  VCF metadata models
- `htsjdk.samtools.util`: intervals, locatable helpers, histograms, iterators,
  compression streams, and sequence helpers
- `htsjdk.tribble`: `Feature` and parser/codec interfaces
- Java collection/stream facts: iteration, `Map.Entry`, `Optional`, and stream
  terminals remain platform facts unless the receiver is a VariantContext type

This separation is important because GATK often chains these packages together:
a VCF reader yields `VariantContext`, a `Feature` may be implemented by a
variant-like value, and collection APIs expose `Allele` or `Genotype`
elements. The pack should own VariantContext semantics without claiming every
method on the surrounding collection or codec objects.

### Rust Representation Policy

The first version should not pretend HTSJDK values are translated Rust structs.
Use facade or erased external values unless the pack explicitly maps a value to
a stable Rust domain type.

Initial defaults:

- `Allele`: facade value, cloneable if facts prove it is immutable enough.
- `VariantContext`: facade value or borrowed facade view depending on slot
  shape.
- `VariantContextBuilder`: mutable builder facade with fluent mutator facts.
- `Genotype`: facade value.
- `GenotypeBuilder`: mutable builder facade with fluent mutator facts.
- `GenotypesContext`: sequence-like facade with `Genotype` element facts.
- attributes maps: conservative map facade unless key and value shapes are
  known.

Later, a user-provided mapping can replace a facade with a native Rust domain
type, but the rest of the pipeline should see the same facts: value category,
nullability, mutation, element shapes, and conversions.

### GATK-Inspired Fixtures

Create small fixtures with neutral package names that mimic observed GATK
usage:

1. Construct alleles from bases and reference flags, then pass them into a
   variant builder.
2. Read `VariantContext` contig, start, end, alleles, genotypes, and attributes
   through getters.
3. Chain `VariantContextBuilder` setters and call `make`.
4. Chain `GenotypeBuilder` setters and call `make`.
5. Iterate over genotypes and alleles, including stream-like and enhanced-loop
   forms.
6. Read optional or nullable attributes and force the result through
   `Option<T>` planning.
7. Compare alleles and variant contexts with `equals`, hash-like methods, and
   collection membership.
8. Use constants or enum-like values from the package.
9. Catch a package-specific exception and a broader exception superclass.
10. Pass `VariantContext` through an external method boundary and preserve its
    facade shape.
11. Store a non-null `VariantContext` in a field initialized by a constructor.
12. Reject or conservatively model a package method that is in the manifest but
    intentionally unsupported.

The Java snippets should look like application code, not HTSJDK
implementations. The Rust assertions should focus on facts and plans first,
with final-output snapshots only after the fact layer is covered.

Immediate fixture plan for closing the pilot:

1. `VariantContextBuilderFlow`: construct reference/alternate alleles, pass
   them to a builder, chain `chr/start/stop/alleles/genotypes/attribute`, and
   return `make()`. Expected facts: fluent receiver mutation, non-null builder
   result, non-null `VariantContext` result, no lowering-specific rule.
   Status: covered by
   `semantic_analysis_uses_variantcontext_pack_for_builder_and_read_flow` in
   `tests/external_packs.rs`; the fixture exercises allele construction,
   genotype construction, `GenotypesContext` mutation, fluent
   `VariantContextBuilder` setters, and non-null `make` results.
2. `VariantContextReadFlow`: read contig, start, end, type, ID, allele count,
   genotype count, nullable attribute, string-list attribute, and predicate
   methods. Expected facts: scalar non-null getters, nullable attribute
   planning, collection element shapes, boolean predicates.
   Status: covered by
   `semantic_analysis_uses_variantcontext_pack_for_read_flow` in
   `tests/external_packs.rs`; this fixture also checks the enum-like `Type`
   surface and nullable string-attribute behavior.
3. `VariantContextGenotypeFlow`: build a `Genotype`, store it in
   `GenotypesContext`, iterate with enhanced-for and stream-like access, and
   perform sample lookup. Expected facts: sequence-like external value with
   `Genotype` element shape and no accidental Java collection ownership by the
   VariantContext pack.
   Status: covered by
   `semantic_analysis_uses_variantcontext_pack_for_genotype_flow` in
   `tests/external_packs.rs`; the fixture exercises builder mutation, sample
   lookup, indexed genotype access, iterator/stream-like methods, and
   sequence-element facts.
4. `VariantContextExceptionFlow`: call validation methods and catch the
   package-specific exception plus a broader runtime superclass. Expected
   facts: pack-owned throw facts and hierarchy assignability.
   Status: covered by
   `variantcontext_exception_catches_use_pack_hierarchy_facts` in
   `tests/external_packs.rs`; the fixture calls multiple validation methods,
   verifies pack-owned throw facts, checks specific and broader catch
   assignability, and rejects missing/unsupported pack diagnostics.
5. `VariantContextBoundaryFlow`: pass `VariantContext`, `Allele`, `Genotype`,
   and `GenotypesContext` through external helper parameters and returns.
   Expected facts: facade/erased object shape is preserved through slots.
   Status: covered by
   `variantcontext_external_method_boundary_preserves_facade_shape` in
   `tests/slot_shape_design.rs`.
6. `VariantContextUnsupportedFlow`: call one selected manifest-declared
   unsupported API and one unmodeled owned-package API. Expected facts:
   explicit unsupported diagnostic for the former and pack-owned missing
   coverage diagnostic for the latter.
   Status: covered by the VariantContext unsupported and missing API tests in
   `tests/external_packs.rs`.
7. `VariantContextOverrideFlow`: override a nullable getter or mutator fact via
   a user fact file. Expected facts: user provenance wins over pack provenance
   while the pack still owns the boundary.
   Status: covered by direct fact precedence and the
   `semantic_analysis_uses_user_override_for_variantcontext_pack_flow`
   semantic fixture in `tests/external_packs.rs`.

### Pilot Acceptance Criteria

The pilot is useful when:

- every selected `variantcontext` type has a pack-owned declaration fact
- builder methods produce receiver-mutation and fluent-return facts
- getter methods expose nullability and collection element shapes
- `GenotypesContext` behaves as a sequence-like external value
- unsupported package methods produce structured diagnostics
- user overrides can change at least one method's nullability or mutability
- slot-shape tests show facade values flowing through fields, parameters,
  returns, collections, and `Option`
- GATK benchmark output shows fewer unexplained external guesses for
  `variantcontext` without adding GATK-specific lowering branches

Current focused benchmark status: the GATK `tools/walkers/variantutils`
package has zero unexplained method calls and no manifest coverage diagnostics
after modeling `VariantContext.Type.values` and the GATK-observed
`VCFConstants` string fields in the VCF pack. It also has zero unexplained
constructor and field accesses. Its remaining semantic diagnostics are only
the expected unsupported `VariantContext.fullyDecode` boundary.

For this milestone, the pilot is considered closed when all immediate fixture
flows above have direct fact tests and semantic-analysis tests, and when a
selected-pack GATK benchmark no longer reports VariantContext-owned APIs as
generic unexplained calls. Remaining APIs may still be conservative or
unsupported as long as that status is manifest-owned and visible in diagnostics.

## Benchmark Plan

Run at least three benchmark modes:

1. GATK alone, with built-in external packs disabled to expose the generic
   Java external boundary baseline.
2. GATK plus HTSJDK external fact packs for variant, VCF, util, tribble,
   header, and sequence dictionary APIs.
3. GATK plus crate/facade mappings for selected external libraries such as
   HDF5 and SQLite.

Compare:

- unresolved methods, constructors, and fields
- external API guesses by provenance
- fact counts and saturation time
- object strategy fallbacks caused by unknown hierarchy closure
- nullability and builder/fluent-mutator facts for HTSJDK-shaped objects
- Rust output idiom issues caused by dependency boundaries

If adding external facts reduces guesses but creates poor Rust output, refine
the facts or facade mappings. Do not partially co-translate external libraries
as a workaround unless the dependency has been promoted to a full translation
target.

## Near-Term Work

1. Add a dependency-mode configuration structure for source roots and fact
   providers. Status: implemented as `DependencyModeConfig` with explicit
   source-root modes, fact-provider modes, benchmark integration, CLI
   `resolver-bench --source-root MODE:PATH` inputs for additional primary,
   co-translated, and source-summary roots, and GATK-shaped tests proving that
   adding a dependency root changes ownership from external boundary to
   cross-root source resolution.
2. Add an external pack manifest format with package ownership, coverage
   states, provenance, unsupported entries, and crate/facade mapping hooks.
   Status: implemented as an in-Rust `ExternalPackManifest` declaration with
   owned type patterns, default coverage, mode, Rust target/facade hook,
   explicit modeled method, constructor, field, field-nullability,
   object-strategy, type capability, API argument-effect, API effect, and
   exception entries for current built-in packs, and unsupported API entries.
   Existing built-in packs now generate boundary, constructor, field,
   field-nullability, hierarchy override, type capability, argument
   nullability/mutability/alias, API effect, exception, and unsupported facts
   from those manifests. A line-oriented serialized manifest parser now covers
   the same core boundary/member/field-nullability/object-strategy/capability/
   argument-effect/API-effect/exception fact shapes for user-authored pack
   files outside the binary, and the CLI accepts those files through
   `--external-pack-manifest`.
3. Teach the resolver benchmark to report fact provenance by source root,
   external pack, crate facade, or conservative guess. Status: implemented for
   configured source roots, configured fact providers/source summaries,
   external fact-model packs, Rust crate-mapping packs, FFI/conservative pack
   buckets, and conservative guesses. Source-summary roots are now parsed into
   the source index for declaration resolution without being benchmarked as
   emitted files, and they emit declaration facts for hierarchy/object-strategy
   consumers through the same external fact set used by configured providers.
   Resolved method and constructor calls into dependency source roots are
   counted separately from unresolved/external calls, making source-root
   provenance visible in benchmark output.
   Parsed external pack manifests can be registered as
   dependency fact providers with inferred dependency mode, and benchmark
   provider fact counts include pack boundaries and unsupported API facts.
   Manifest providers are now rejected if they have no boundary row or mix
   `external-fact-model`, `rust-crate-mapping`, `ffi-boundary`, and
   `conservative-boundary` rows, preserving the provider as the unit of
   dependency-mode attribution. The `resolver-bench` CLI can now run the planned benchmark modes directly with
   `--source-root`, `--facts`, and `--external-pack-manifest` inputs, and
   `--no-builtin-external-packs` allows baseline/provider/crate-mapping modes
   to be measured without implicit built-in pack attribution. Selected built-in
   packs can be re-added with `--builtin-external-pack`, so the GATK benchmark
   can measure HTSJDK packs and HDF5/SQLite crate mappings independently. A
   neutral GATK-shaped regression now runs the same source in three dependency
   modes: baseline with built-ins disabled, explicit HTSJDK fact-model
   provider, and explicit HDF5/SQLite crate-mapping providers. This proves that
   benchmark attribution follows selected dependency mode rather than the Java
   spelling or the default built-in pack set; the controlled crate-mapping
   fixture now asserts exact HDF5/SQLite explained method and constructor
   counts instead of lower-bound movement. The same fixture includes
   `GenotypesContext` indexed/sequence-style calls and an HDF5 nested
   `OpenMode` constructor argument, so selected-provider mode coverage now
   includes generic external values and nested facade constants rather than
   only direct receiver method calls. The
   same report now includes package-level object representation counters for
   concrete structs, dyn trait fallbacks, closed enums, erased objects, and
   hierarchy-closure diagnostics. It also renders
   `top_object_representation_diagnostics` rows with code, source-root mode,
   count, message, and package/file samples, so dependency-mode comparisons
   expose the highest object-model pressure points instead of only resolver coverage. Benchmark
   reports also expose `external_api_return_nullability_facts`,
   `external_api_fluent_builder_facts`, and
   `external_api_argument_effect_facts`, so HTSJDK-shaped getter nullability,
   fluent builder facts, and ownership/lifetime boundary facts are measurable
   comparison axes instead of hidden semantic details. The report also renders
   `total_facts` and `elapsed_ms`, giving the benchmark-plan fact-count and
   saturation/runtime comparison axes concrete fields.
   Unsupported external methods, constructors, and fields also have dedicated
   benchmark counters and top summaries, keeping pack-owned unsupported
   boundaries distinct from unexplained resolver gaps. Missing modeled entries
   inside an owned external pack have the same benchmark treatment:
   pack-owned missing methods, constructors, and fields increment dedicated
   missing-member counters and top summaries instead of appearing as generic
   unexplained resolver gaps. Built-in pack ownership lookup now uses
   manifest boundary declarations directly instead of reconstructing every
   pack's full fact set for each unresolved target, keeping this attribution
   path usable on larger corpus benchmarks. Built-in member lookup now follows
   the same boundary: `external_packs` owns cached pack fact sets and exposes
   narrow lookup functions for API effects, unsupported APIs, constructors,
   fields, crate API mappings, crate constructor mappings, and
   representation-relevant hierarchy/object/capability facts. Semantic
   analysis, call planning, representation setup, and resolver-benchmark
   attribution should consume those typed lookup functions instead of
   reconstructing or searching whole packs by name. Those lookup functions are
   now backed by a cached built-in
   pack lookup index for API effects, unsupported APIs, constructors, fields,
   crate/facade mappings, and per-type representation facts, so repeated
   resolver/semantic/representation probes no longer linearly scan every pack
   fact set. API-effect, argument-effect, constructor, field, field-nullability,
   exception-kind, object/capability, provider-analysis, unsupported-member,
   and facade-mapping lookup now share the same receiver/type alias
   normalization across configured facts and built-in pack caches. That
   normalization includes fully qualified Java names, imported/simple Java
   names, snake-case resolver keys, and canonical external receiver keys, so
   those spellings do not split pack coverage. Resolver-benchmark
   provider-mode inference uses the same type-name matcher for configured
   pack-owned member facts, so selected-provider attribution remains stable
   when a manifest models `external.pkg.Type` but source resolution reports
   `Type`. Unresolved-call type inference also asks
   external packs with the active call arity, so exact-arity typed effects such
   as indexed protobuf getters can feed chained receiver typing instead of
   falling back to arity-agnostic unknowns. A regression test covers this
   against GATK-inspired VariantContext unsupported coverage, HDF5
   crate-mapping constructor/method/field facts, and GenomicsDB protobuf
   `getFields(int).getName()` chaining. A focused GATK `utils/variant`
   benchmark now
   completes in about 11 seconds and reports zero semantic diagnostics, zero
   external-pack missing methods, zero external-pack missing constructors, zero
   external-pack missing fields, and zero unexplained method, constructor, or
   field accesses. The VCF `VariantContextWriterBuilder`/`VariantContextWriter`
   cluster and the remaining VariantContext enum/CommonInfo list gaps are now
   covered by pack facts rather than benchmark-only exceptions. The report also
   now includes top
   source-dependency method and constructor summaries, so multi-root runs show
   which sibling source roots closed a boundary instead of forcing the reader
   to infer that from reduced external guesses. Parsed user-authored external
   pack manifests are also covered for every dependency provider mode: neutral
   regressions prove `external-fact-model`, `rust-crate-mapping`,
   `ffi-boundary`, and `conservative-boundary` manifests infer the correct
   provider mode and attribute explained method/constructor calls to the
   matching benchmark bucket. Parsed `rust-crate-mapping` manifests also share
   the all-or-nothing missing-member contract: unmodeled methods,
   constructors, and fields inside an owned mapped pack increment pack-owned
   missing counters instead of generic unexplained buckets. FFI and
   conservative-boundary manifests now have the same regression coverage for
   missing owned methods, constructors, and fields, with missing calls still
   attributed to the selected provider mode instead of falling through to
   generic unexplained targets.
   Top external-pack and source-dependency method/constructor summaries now
   include an explicit mode column. External-pack rows use the selected
   external-library mode (`ExternalFactModel`, `RustCrateMapping`,
   `FfiBoundary`, or `ConservativeBoundary`), while source-dependency rows use
   the concrete source-root mode (`CoTranslateSource` or `SourceSummary`).
   Pack-owned unsupported/missing field summaries use the same external-library
   mode column. The report model now represents external rows with an explicit
   `ExternalReportMode` (`Known(mode)` or `Unknown`) rather than overloading
   `Option<ExternalLibraryMode>`, so unknown external attribution is distinct
   from source-dependency attribution before rendering.
   Mixed selected-pack runs therefore remain auditable from their
   highest-volume rows instead of requiring separate counter correlation.
   Pack-owned missing and unsupported method calls now also render context
   tables with receiver type, method, dependency mode, arity, enclosing
   type/member, and sample files. Missing and unsupported constructors have the
   same context treatment using target type, receiver type, dependency mode,
   argument shape, enclosing type/member, and sample files. Missing and
   unsupported fields also render field-context tables with field name,
   receiver type, dependency mode, enclosing type/member, and sample files.
   Explained pack-owned methods, constructors, and fields now render matching
   context tables as well. A neutral dependency fixture can therefore prove not
   only that a pack fact exists, but which source member consumed it, without
   running the whole GATK corpus or adding dependency-specific lowering rules.
   Resolved external-pack fields are included in that same explained-field
   accounting, not only fields that first appeared as unresolved fallback
   targets; `resolver_benchmark_reports_explained_external_pack_field_contexts`
   locks this in with HDF5 `OpenMode` field-chain facts loaded as a selected
   provider. The accounting is also statement-shape independent:
   `resolver_benchmark_reports_resolved_pack_fields_across_statement_shapes`
   covers assignments, loops, enhanced-for iterables, switch cases, and
   try/catch/finally bodies so pack provenance does not disappear just because
   a modeled field is consumed outside a return expression. Lambda bodies use
   the same scoped environment rule as resolver analysis:
   `resolver_benchmark_does_not_treat_lambda_params_as_static_pack_receivers`
   proves an untyped lambda parameter with a type-like name shadows external
   static receiver lookup, so benchmark mining cannot turn a parameter-name
   collision into false HDF5 or HTSJDK pack evidence. The same rule applies to
   source-summary field attribution:
   `source_summary_field_contexts_do_not_treat_lambda_params_as_static_receivers`
   proves a lambda parameter cannot become false source-dependency evidence
   just because it shares a spelling with an imported summary type.
   Anonymous class bodies are also part of field provenance:
   `resolver_benchmark_reports_pack_fields_inside_anonymous_class_bodies`
   proves modeled pack fields consumed in anonymous methods are attributed to
   the selected external pack and reported against the anonymous member body,
   while anonymous-object representation diagnostics stay a separate
   object-model concern. Source-summary fields follow the same context rule:
   `source_summary_field_contexts_include_anonymous_class_bodies` proves
   dependency fields consumed inside anonymous methods are reported against the
   anonymous member rather than the enclosing factory method. Source-summary
   methods are covered by the same anonymous-member context rule via
   `source_summary_method_contexts_include_anonymous_class_bodies`, so method
   calls from anonymous adapters are not flattened into the factory method that
   constructs the adapter. Source-summary constructors are also covered by
   `source_summary_constructor_contexts_include_anonymous_class_bodies`, so
   anonymous factory methods keep constructor pressure attached to the adapter
   member that created the dependency value. External-pack method contexts use the same resolver
   context propagation:
   `resolver_benchmark_reports_pack_methods_inside_anonymous_class_bodies`
   proves unresolved calls inside anonymous members keep the anonymous member
   name before benchmark attribution maps them to a selected pack.
   `resolver_benchmark_reports_pack_constructors_inside_anonymous_class_bodies`
   locks in the same behavior for pack-owned constructors created inside
   anonymous adapter methods.
   That lets a selected-provider benchmark
   trace a modeled dependency fact, manifest gap, or intentional unsupported
   method, constructor, or field back to the exact source member that consumed
   it instead of only listing the receiver/member pair.
4. Implement package-owned diagnostics for calls inside an external pack that
   are declared unsupported or missing from the manifest. Status: implemented
   for unsupported methods, missing methods, missing constructors, and missing
   fields. Pack-owned missing and unsupported entries now produce structured
   diagnostics and do not silently fall through to conservative external
   guesses for both built-in packs and parsed user-authored external pack
   manifests. Resolver benchmark attribution now also keeps missing and
   unsupported pack-owned methods, constructors, and fields out of generic
   unexplained buckets.
5. Build the `htsjdk.variant.variantcontext` pilot pack and its GATK-inspired
   fixture tests. Status: implemented. The pilot pack has manifest ownership,
   builder/getter/mutator/nullability facts, typed return facts for central
   builder and getter APIs, unsupported API diagnostics, user-override
   precedence tests, semantic-analysis tests, resolver benchmark attribution
   tests, and GATK-shaped neutral fixtures. A slot-shape regression now uses
   the real `VariantContext.getAttribute` pack fact to prove nullable external
   getter results flow through local and return signature planning as
   `Option`-shaped slots. The fix treats storage-nullability requirements as
   propagating shape constraints, so nullable facts are not lost when an
   external call result is assigned to a local and then returned. External
   exception hierarchy is also pack-owned: the variantcontext pack now declares
   `VariantContextException` and `TribbleException.InternalCodecException` as
   unchecked runtime-exception subclasses, including short imported names. A
   GATK-shaped regression catches `InternalCodecException` before
   `RuntimeException` after calling `VariantContext.validateAlternateAlleles`,
   proving that semantic catch assignability consumes external-pack hierarchy
   facts instead of depending on lowering-time special cases. Slot-shape
   regressions also now cover the remaining facade-boundary fixtures from the
   pilot checklist: a neutral external helper returning `VariantContext`
   preserves the facade type through local, parameter, and method-return
   shapes, and a constructor-initialized `VariantContext` field stays
   non-optional when semantic field-nullability facts prove every constructor
   writes a non-null value and no method nulls it.
6. Add HTSJDK external fact packs for `variant.vcf`, `samtools.util`,
   `tribble.Feature`, and selected header/dictionary models only after the
   `variantcontext` pilot proves the mechanism. Status: implemented for
   `htsjdk.variant.vcf`, `htsjdk.samtools.util`, `htsjdk.tribble.Feature` and
   related simple feature values, plus selected SAM header/sequence dictionary
   models. Follow-up mining added keyed VCF header lookup facts and selected
   SAM header/read-group/sequence record facts such as read-group lookup,
   header clone, sequence assembly, header sequence lookup, and read-group
   sequencing-center metadata. Selected VCF constructors (`VCFHeader`,
   `VCFHeaderLine`, `VCFInfoHeaderLine`, `VCFFormatHeaderLine`,
   `VCFFilterHeaderLine`, `VCFCodec`, and `VCFRecordCodec`) have also been
   removed from the legacy HTSJDK stdlib constructor fallback; without the VCF
   pack they are conservative, and with the pack their precision comes from
   `ExternalLibraryPack` facts. `VCFHeader.HEADER_FIELDS` and
   `VCFHeader.REFERENCE_KEY` are now typed VCF pack fields rather than stdlib
   field shortcuts. The selected SAM header/dictionary constructors
   (`SAMSequenceDictionary`, `SAMSequenceRecord`, `SAMFileHeader`, and
   `SAMReadGroupRecord`) now follow the same rule: no stdlib constructor
   shortcut, pack-owned precision only. `SAMSequenceRecord.UNKNOWN_SEQUENCE_LENGTH`
   is also a typed SAM header pack field rather than a stdlib field shortcut.
   `SAMReadGroupRecord.READ_GROUP_ID_TAG` and
   `SAMReadGroupRecord.READ_GROUP_SAMPLE_TAG` are likewise SAM-header-pack-owned
   string constants, keeping read-group metadata tags in the HTSJDK boundary.
   `SAMFileHeader.SortOrder` and `SAMFileHeader.GroupOrder` follow the same
   typed-field rule. Their enum constants, including `SortOrder.coordinate`,
   `SortOrder.unsorted`, and `GroupOrder.query`, are now SAM-header-pack-owned
   fields rather than generic enum-constant handling; `SortOrder.name()` and
   `GroupOrder.name()` are likewise SAM-header-pack-owned getter effects.
   The selected `htsjdk.samtools.util`
   constructors (`Interval`, `IntervalTree`, `IntervalMergerIterator`,
   `Histogram`, `PeekableIterator`, `MergingIterator`, and
   `RuntimeIOException`) have also moved out of the legacy HTSJDK fallback and
   are precise only through samtools-util pack facts. Seekable stream wrappers
   (`SeekableBufferedStream/1` and `SeekablePathStream/1`) now follow the same
   samtools-util pack-owned rule, as do the low-level `BinaryCodec/0` and
   `BinaryCodec/1` utility constructors. Samtools-util line-reader wrappers
   (`BufferedLineReader/1`, `AsciiLineReaderIterator/1`,
   `LineIteratorImpl/1`, and `SynchronousLineReader/1`) are now pack-owned for
   the same reason, as are the utility wrapper constructors `DefaultingMap/2`
   and `Lazy/1`. The selected SAM header pack now also
   owns `SAMTextHeaderCodec/0` and `SAMFormatException/1`, keeping header
   text parsing and format errors out of the generic constructor fallback.
   The `utils/spark` benchmark then promoted the matching write surface:
   `BinaryCodec.writeBytes`, `BinaryCodec.writeInt`,
   `BinaryCodec.writeString`, and `SAMTextHeaderCodec.encode` are now exact
   arity pack-owned side-effect/mutator facts rather than Spark-specific
   lowering rules.
   The SAM reader pack owns `SamFileHeaderMerger/3` and
   `MergingSamRecordIterator/3`, so reader/header merge support is modeled as
   an explicit HTSJDK pack boundary instead of broad fallback precision.
   `SAMFileWriterFactory/0` now follows the same rule through a narrow
   `htsjdk.samtools.writer` pack, keeping writer factory construction out of
   the generic HTSJDK constructor fallback.
   BAM/index construction now has the same explicit boundary:
   `BAMRecordCodec/1`, `BAMIndexer/2`, and `SBIIndexWriter/2` are owned by a
   narrow `htsjdk.samtools.bam` pack. `SBIIndexWriter.DEFAULT_GRANULARITY`
   and `BAMFileConstants.FIXED_BLOCK_SIZE` are also BAM-pack-owned, so index
   construction and static BAM tuning/format constants no longer depend on
   generic stdlib precision.
   CRAM construction is likewise pack-owned: `CramContainerIterator/1`,
   `Container/3`, and `CRAMReferenceRegion/2` are now modeled by a narrow
   `htsjdk.samtools.cram` pack instead of the generic HTSJDK constructor
   fallback.
   `BCF2Codec/0` is now owned by a narrow `htsjdk.variant.bcf2` pack rather
   than the generic HTSJDK constructor fallback.
   `MetricsFile/0` is now owned by a narrow `htsjdk.samtools.metrics` pack,
   keeping HTSJDK metrics-file construction out of the generic constructor
   fallback while leaving Picard/GATK-specific metric value classes separate.
   The final legacy HTSJDK fallback constructor, `ComponentMethods/0`, is now
   owned by a deliberately narrow `htsjdk.nested_helper` pack because the
   local corpus identifies it only as an HTSJDK nested helper. At this point
   the broad HTSJDK constructor fallback no longer grants precision to any
   constructor; pack facts own the audited HTSJDK surfaces.
   VariantContext builder
   constructors (`GenotypeBuilder` and `VariantContextBuilder`) likewise no
   longer use generic HTSJDK fallback precision; constructor arities are
   pack-owned alongside the builder method facts. `VariantContextComparator/1`
   is now also owned by the same variantcontext pack rather than the generic
   HTSJDK constructor fallback. The tribble
   `SimpleFeature/3`, `FeatureCodecHeader/2`, and
   `CodecLineParsingException/1` constructors follow that same rule, and
   `Gff3BaseData/9` is now treated as the same Tribble feature/GFF boundary
   rather than broad HTSJDK fallback precision.
   `FeatureCodecHeader.NO_HEADER_END` is also pack-owned by the tribble-feature
   pack, keeping static sentinel typing out of the generic stdlib field table.
   The latest receiver-helper refresh added VCF
   sequence-dictionary mutation, header metadata predicates/lookups, SAM
   program-record lookup, samtools-util Histogram statistics, VariantContext
   aggregate counts, Allele extension, and GenotypeBuilder cleanup mutators.
   `SAMProgramRecord/1` has now moved out of the broad HTSJDK constructor
   fallback as well; the selected header/dictionary pack owns the type,
   constructor, and basic program-record getters.
   `SAMRecord/1` is now owned by a narrow `htsjdk.samtools.record` pack rather
   than the broad HTSJDK fallback. The first record pack slice models
   construction from `SAMFileHeader` plus read-only coordinate, read-name, and
   header getters; it now also owns the default record factory and coordinate/
   query-name comparators with read-only `compare` facts. The same record pack
   now owns the adjacent `CigarElement/2` and `Cigar/0`/`Cigar/1`
   constructors plus the `FilteringSamIterator/2` record-iterator wrapper.
   `CigarOperator` constants are now SAM-record-pack-owned fields, so CIGAR
   operator typing no longer depends on the generic enum-constant policy.
   The transformer-package slice also moved the observed CIGAR behavior into
   the same SAM record pack: `Cigar.add`, `getCigarElement`,
   `getFirstCigarElement`, `getLastCigarElement`, `isValid`,
   `numCigarElements`, and `CigarElement.getOperator` now have semantic pack
   facts. This keeps CIGAR construction, element access, operator access, and
   validation as one external-library responsibility instead of spreading
   per-call rules into lowering.
   The `utils/read` package slice extends that same responsibility boundary
   with the high-volume read-adapter surface observed in GATK:
   `Cigar.getCigarElements`, read/reference length and emptiness helpers,
   `CigarOperator` read/reference/clipping/alignment predicates, and common
   `SAMRecord` coordinate, flag, CIGAR, read-group, attribute, base, and
   mapping-quality getters/setters. These facts deliberately stay in the
   SAM-record external pack because ownership, nullability, and receiver
   mutation are library semantics, not lowering special cases.
   The follow-up `utils/read` slice added the adjacent pair/secondary/duplicate
   flag predicates and mutators, mate/reference mutators, and CIGAR-string
   setter (`getFirstOfPairFlag`, `getProperPairFlag`,
   `getDuplicateReadFlag`, `isSecondaryAlignment`,
   `isSecondaryOrSupplementary`, `setProperPairFlag`,
   `setSecondaryAlignment`, `setFirstOfPairFlag`, `setSecondOfPairFlag`,
   `setCigarString`, `setMateAlignmentStart`,
   `setMateNegativeStrandFlag`, and `setReferenceIndex`). SAM reader and
   writer factory methods are modeled in their own packs:
   `SamReaderFactory.makeDefault/open/referenceSequence`,
   `SamFileHeaderMerger.getMergedHeader`, and
   `SAMFileWriterFactory.makeWriter`. This keeps record-state facts separate
   from SAM file IO factory facts while still letting the benchmark exercise
   both from GATK-shaped tests.
   A later read-adapter helper slice moved the next clear HTSJDK rows into
   their owning packs: SAMRecord copy/attribute/alignment/string helpers,
   additional CIGAR operator predicates/conversion, `SAMUtils` read utility
   helpers, `SequenceUtil` reverse-complement/NM-tag helpers, and
   `SAMFileWriterFactory.setCreateIndex`. The remaining `utils/read` gaps are
   now low-count rows that should continue to be handled by their owning packs
   (`SAMRecord`, SAM header/program records, Kryo, and Commons Collections)
   rather than by generic method-name policy.
   The GenomicsDB benchmark then exposed the adjacent Tribble reader boundary:
   anonymous `FeatureReader` implementations should be erased external values,
   and `FeatureReader.getHeader`, `query`, `iterator`, `getSequenceNames`,
   `isQueryable`, and `close` are now pack-owned `htsjdk.tribble.feature`
   facts. This keeps reader lifecycle and query methods with the Tribble
   feature pack instead of treating them as GenomicsDB API behavior or
   anonymous source-class lowering.
   The final `utils/read` external-pack tail now also models the remaining
   SAMRecord pair/mate/transient setters, `SAMFileWriterFactory.setCreateMd5File`,
   Kryo `Output.writeAscii`, Commons Collections `MultiMap.append/get`,
   `SamPairUtil` mate-linking helpers, and
   `SequenceUtil.sumQualitiesOfMismatches`. A follow-up SAM-reader boundary
   cleanup also models `SamReaderFactory.referenceSequence` as a fluent
   mutator returning `SamReaderFactory`, so the reader factory pack owns the
   full GATK-observed reference-configuration chain. The SAM record pack also
   owns `htsjdk.samtools.filter.SamRecordFilter` as an erased external
   boundary, so GATK-shaped anonymous filter implementations no longer appear
   as source object-lowering pressure. The Kryo pack also owns the qualified
   `com.esotericsoftware.kryo.Serializer` erased superclass boundary, and
   representation fact discovery now treats declared `extends`/`implements`
   clauses as external-pack triggers. This covers GATK-shaped nested source
   serializers such as `Pair.Serializer extends
   com.esotericsoftware.kryo.Serializer<Pair>` without adding an ambiguous
   bare `Serializer` pack alias or a lowering special case. The samtools-util
   pack now also owns `htsjdk.samtools.util.DelegatingIterator` as a
   constructor/API/object-representation boundary, so GATK-shaped anonymous
   iterator cleanup wrappers stay at the HTSJDK util boundary instead of
   appearing as unsupported anonymous source classes. The source fact layer now
   also infers a closed enum candidate for abstract classes whose only direct
   known variants are nested subclasses with no public/protected constructors.
   This covers GATK-shaped key families such as `ReadsKey` without treating
   ordinary public inheritance as closed and without adding a GATK-specific
   override. Finally, interface methods named `copy`, `deepCopy`, or `clone`
   that return the interface type and take no parameters are treated as
   clone-like dyn-trait candidates instead of generic non-object-safe
   self-returning factories. This keeps `GATKRead.copy/deepCopy` in the object
   strategy layer as a clone-box design obligation, while ordinary methods such
   as `Factory.next(): Factory` still fall back to erased object
   representation. The package now
   benchmarks at zero resolver diagnostics, zero semantic external-pack
   diagnostics, and zero unexplained method calls while still reporting
   object-representation and source/project constructor or field work
   separately.
   SAMRecord static sentinel fields such as `NO_ALIGNMENT_CIGAR`,
   `NO_MAPPING_QUALITY`, `NULL_QUALS`, and `NULL_SEQUENCE` are likewise
   SAM-record-pack-owned fields rather than generic stdlib field shortcuts.
   `htsjdk.samtools.reference` is now a separate semantic pack for reference
   IO/model boundaries. It owns `FastaSequenceFile/2`,
   `IndexedFastaSequenceFile/1`, `FastaReferenceWriterBuilder/0`, and
   `ReferenceSequence/3`, plus basic `ReferenceSequence` getters and
   `FastaSequenceFile.nextSequence/close` effects. Reference static fields and
   deeper FASTA writer behavior are deliberately left for later, narrower
   slices instead of broadening the stdlib fallback.
   Compression output construction is likewise no longer a broad HTSJDK
   shortcut: `BlockCompressedOutputStream/2` and `/3` now belong to the
   `htsjdk.samtools.util` pack next to `BlockCompressedInputStream`, with
   `flush` and `close` modeled as receiver-mutating resource effects.
   HTSJDK runtime defaults are also pack-owned now:
   `Defaults.COMPRESSION_LEVEL`,
   `Defaults.USE_ASYNC_IO_READ_FOR_SAMTOOLS`,
   `Defaults.USE_ASYNC_IO_WRITE_FOR_SAMTOOLS`, and
   `Defaults.USE_ASYNC_IO_WRITE_FOR_TRIBBLE` are modeled fields of the
   samtools-util pack rather than generic stdlib field shortcuts.
   `SAMUtils.MAX_PHRED_SCORE`, `ReservedTagConstants.XT`,
   `ReservedTagConstants.PROGRAM_GROUP_ID`,
   `ReservedTagConstants.READ_GROUP_ID`, and
   `SequenceUtil.VALID_BASES_UPPER` now follow that same samtools-util
   pack-owned field path. `BlockCompressedStreamConstants` numeric and byte
   array constants, including `BLOCK_HEADER_LENGTH`, `BLOCK_LENGTH_OFFSET`,
   `DEFAULT_COMPRESSION_LEVEL`, and `EMPTY_GZIP_BLOCK`, now use the same
   samtools-util pack-owned field path instead of generic stdlib precision.
   The transformer-package quality/base helper slice also keeps
   `SAMUtils.phredToFastq` and `SequenceUtil.isNoCall` in this same
   samtools-util pack, giving return type, nullability, and predicate facts
   without teaching lowering about either helper name.
   The same boundary now covers the GATK-observed
   FASTQ pair markers `FastqConstants.FIRST_OF_PAIR` and
   `FastqConstants.SECOND_OF_PAIR` from `htsjdk.samtools.fastq`.
   The follow-up predicate/helper refresh added VariantContext filter/type/
   overlap/quality helpers, VCF comparator/version helpers, SAM string
   rendering, and Histogram key access. The latest vector/matrix helper
   refresh added list-valued VariantContext attributes, genotype-likelihood
   projections, GenotypesContext bulk mutation, Locatable containment/length,
   Histogram prefill, SAM sequence-name truncation, and HDF5 matrix/load
   helpers. The membership/metadata refresh raised external fact-model
   explained method calls to 5,507 and reduced unexplained method calls to
   135. The predicate/header/coordinate helper refresh adds GATK-inspired
   fixture coverage for `VariantContext` sample polymorphism and allele-index
   helpers, `Allele` call/breakpoint predicates, `Genotype` allele counting,
   `GenotypeLikelihoods` allele-pair lookup, VCF/SAM header metadata lookups,
   and `Locatable`/`Feature` contig comparisons. Current default GATK
   benchmark evidence shows external fact-model packs explain 5,534 method
   calls and 463 constructors, Rust crate-mapping packs explain 58 method
   calls and 15 constructors, and unexplained method calls are down to 108.
   The follow-up statistic/sample-state refresh adds `VariantContext`
   polymorphic/deletion predicates, boolean attributes, ordered genotype
   iteration, reference length, `VariantContextBuilder.noGenotypes`,
   `GenotypeLikelihoods.numLikelihoods`, and additional Histogram statistics.
   Current default GATK benchmark evidence shows external fact-model packs
   explain 5,553 method calls and 463 constructors, Rust crate-mapping packs
   explain 58 method calls and 15 constructors, and unexplained method calls
   are down to 89. The follow-up builder/header/statistic refresh adds
   `VariantContextBuilder.putAttributes`, `GenotypeBuilder.makeWithShallowCopy`,
   `VariantContext.getNoCallCount`, `VariantContext.hasAlternateAllele`,
   `BEDFeature.getScore`, `VCFHeader.hasFormatLine`,
   `VCFHeader.hasGenotypingData`, and `Histogram.getStandardDeviation`.
   Current default GATK benchmark evidence shows external fact-model packs
   explain 5,564 method calls and 463 constructors, Rust crate-mapping packs
   explain 58 method calls and 15 constructors, and unexplained method calls
   are down to 78. The follow-up predicate/builder/header-line refresh adds
   `Allele.isSingleBreakend`, `Allele.isNonRefAllele`,
   `VariantContext.isComplexIndel`, `VariantContext.isStructuralIndel`,
   `VariantContextBuilder.getAttributes`, `GenotypeBuilder.phased`,
   `Genotype.isHetNonRef`, `Genotype.isNonInformative`, and
   VCF compound header-line fixed-count predicates. Current default GATK
   benchmark evidence shows external fact-model packs explain 5,576 method
   calls and 463 constructors, Rust crate-mapping packs explain 58 method
   calls and 15 constructors, and unexplained method calls are down to 66.
   The follow-up low-count HTSJDK helper refresh adds `Locatable.overlaps`,
   `VariantContext.isSymbolicOrSV`, `VariantContext.subContextFromSamples`,
   `VariantContext.toStringDecodeGenotypes`, `VariantContextBuilder.loc`,
   `VariantContextBuilder.noID`, `Allele.oneIsPrefixOfOther`, and
   `SAMFileHeader.setComments`. Current default GATK benchmark evidence shows
   external fact-model packs explain 5,589 method calls and 463 constructors,
   Rust crate-mapping packs explain 60 method calls and 15 constructors, and
   unexplained method calls are down to 51. The follow-up SAM bulk-mutator and
   genotype predicate refresh adds `SAMFileHeader.setReadGroups`,
   `SAMFileHeader.setProgramRecords`, `SAMFileHeader.setAttribute`,
   `SAMReadGroupRecord.setPlatformUnit`, `SAMReadGroupRecord.setFlowOrder`,
   `SAMReadGroupRecord.setAttribute`, and `Genotype.sameGenotype`. Current
   default GATK benchmark evidence shows external fact-model packs explain
   5,600 method calls and 463 constructors, Rust crate-mapping packs explain
   60 method calls and 15 constructors, and unexplained method calls are down
   to 40. The follow-up low-count external helper refresh adds
   `VariantContextBuilder.source`, `SAMSequenceRecord.setSequenceLength`,
   `SAMReadGroupRecord.setSequencingCenter`, `SQLiteConfig.toProperties`,
   `Histogram.trimByWidth`, `IntervalTree.minOverlapper`, and
   `VCFHeaderLineType.valueOf`. Current default GATK benchmark evidence shows
   external fact-model packs explain 5,607 method calls and 463 constructors,
   Rust crate-mapping packs explain 61 method calls and 15 constructors, and
   unexplained method calls are down to 32. The follow-up final low-count
   external fact refresh adds `Histogram.values` and
   `VariantContext.withinDistanceOf`. Current default GATK benchmark evidence
   shows external fact-model packs explain 5,609 method calls and 463
   constructors, Rust crate-mapping packs explain 61 method calls and 15
   constructors, and unexplained method calls are down to 30. The remaining
   rows are now Java equality, validation/assertion policy, dictionary
   equivalence, enum/object naming policy, explicit unsupported decode
   behavior, project-owned iterators, and one `CloseableIterator.remove`
   contract decision rather than clear external-pack omissions. The follow-up
   overload-aware API-effect refresh adds optional argument-count metadata to
   external API facts, keeps inherited Java `Object.equals(Object)` as a
   builtin fact, and models the HTSJDK-specific `Allele.equals(Allele,
   boolean)` overload as an exact-arity pack fact. Current default GATK
   benchmark evidence shows external fact-model packs explain 5,616 method
   calls and 463 constructors, Rust crate-mapping packs explain 61 method
   calls and 15 constructors, and unexplained method calls are down to 24.
   `Allele.equals` has dropped out of the remaining rows; the remaining
   unexplained calls are validation/assertion policy, dictionary equivalence,
   enum/object naming policy, explicit unsupported decode behavior,
   project-owned iterators, and `CloseableIterator.remove`. The follow-up enum
   naming refresh keeps `name()`, `values()`, and `valueOf(...)` as shared
   external enum policy, teaches semantic API receiver inference to recover
   known external field/enum constants without perturbing resolver source-field
   lookup, and covers `SAMFileHeader.SortOrder.coordinate.name()`. Current
   default GATK benchmark evidence shows external fact-model packs explain
   5,616 method calls and 463 constructors, Rust crate-mapping packs explain
   61 method calls and 15 constructors, and unexplained method calls are down
   to 22. `SortOrder.name` has dropped out of the remaining rows. The
   follow-up unsupported-attribution refresh teaches the resolver benchmark to
   classify pack-declared unsupported APIs as explicitly explained external
   boundaries instead of unexplained method calls. Current default GATK
   benchmark evidence shows external fact-model packs explain 5,617 method
   calls and 463 constructors, Rust crate-mapping packs explain 61 method
   calls and 15 constructors, `external_api_unsupported_method_calls` is 1,
   and unexplained method calls are down to 21. `VariantContext.fullyDecode`
   now appears under `top_external_api_unsupported_methods`; the remaining
   rows are validation/assertion policy, dictionary equivalence, project-owned
   iterators, and `CloseableIterator.remove`. The follow-up iterator-boundary
   refresh adds manifest-owned facts for `IntervalMergerIterator.hasNext`,
   `IntervalMergerIterator.next`, and side-effecting
   `CloseableIterator.remove`, keeping these as samtools-util pack facts
   rather than lowering rules. Current default GATK benchmark evidence shows
   external fact-model packs explain 5,620 method calls and 463 constructors,
   Rust crate-mapping packs explain 61 method calls and 15 constructors,
   `external_api_unsupported_method_calls` remains 1, semantic diagnostics
   are down to 49, and unexplained method calls are down to 18. The remaining
   rows are validation/assertion policy and dictionary equivalence. The
   follow-up validation/dictionary refresh adds read-only check facts for
   `VariantContext` validation APIs, including thrown
   `InternalCodecException` metadata, and SAM sequence-dictionary equivalence
   and assertion facts, including `AssertionError` metadata. The validation
   APIs also carry borrowed/read-only argument facts for reported/observed
   `Allele` values and `Set<String>` RSID inputs, so ownership and coercion
   planning consume the same fact layer as method effects. Current default
   GATK benchmark evidence shows external fact-model packs explain 5,638
   method calls and 463 constructors, Rust crate-mapping packs explain 61
   method calls and 15 constructors, `external_api_unsupported_method_calls`
   remains 1, semantic diagnostics are down to 13, and unexplained method
   calls are now 0. The final missing-pack cleanup for this slice models the
   remaining clear HTSJDK facts from the semantic diagnostic summary:
   `Interval.size`, `IntervalList.fromFile`, `IntervalTree.put`,
   `GenotypeBuilder.noDP`, `GenotypeLikelihoods.forEach`,
   `GenotypesContext.iterateInSampleNameOrder`, and
   `GenotypesContext.spliterator`. The `Allele.getKey/getValue` diagnostics
   were not added as fake Allele APIs; they exposed a semantic-analysis
   responsibility bug where enhanced-for loop variables were not in scope while
   collecting external API effects. Resolver regression tests now prove the
   GATK-shaped `Map.Entry<Allele, List<Haplotype>>` receiver is preserved, and
   semantic regression tests prove the variantcontext pack does not claim
   `Map.Entry` accessors as `Allele` APIs. Current default GATK benchmark
   evidence shows external fact-model packs explain 5,638 method calls and
   463 constructors, Rust crate-mapping packs explain 61 method calls and 15
   constructors, unexplained method calls and constructors are both 0, and
   the only semantic diagnostics are the two explicit unsupported
   `VariantContext.fullyDecode` reports in `SelectVariants.java`. The
   constructor-pressure follow-up adds benchmark context tables for
   conservative constructor guesses, including sample files, enclosing
   types/members, and argument shapes. This keeps the next step aligned with
   the dependency-boundary plan: remaining external constructor guesses can be
   classified as pack/facade/source-summary decisions, while project and
   native-wrapper guesses can be kept out of generic external facts. The first
   source-boundary regression for this pressure is a FermiLite-shaped nested
   graph example: primary source imports `AssemblyGraph.Contig` and
   `AssemblyGraph.Connection` from a source-summary dependency, constructs
   replacement `Connection`/`Contig`/`AssemblyGraph` values, and calls
   `getTarget`, `getConnections`, `rcConnection`, and setters through the
   summary. The benchmark attributes all of these as
   `source_dependency_resolved_*` with zero unresolved calls, zero unresolved
   constructors, and zero external guesses. That means the real GATK
   `FermiLiteAssembly.*` pressure should be addressed by adding the dependency
   as source/co-translation/summary input, not by creating a generic
   `Connection` external pack. A follow-up regression covers the broader
   FermiLite-shaped boundary as well: `FermiLiteAssembler()` and
   `createAssembly`, `FermiLiteAssembly(...)`, and nested
   `Contig`/`Connection` graph rewrites all resolve through a source-summary
   root and appear in the source-dependency constructor/method tables rather
   than in external-pack guess tables. Resolver benchmark reports now also
   include source-dependency method and constructor context tables, so future
   source-summary/co-translation decisions can be audited from concrete
   enclosing types, members, arities, and sample files instead of from aggregate
   counts alone. Those aggregate and context rows now include the source-root
   mode (`CoTranslateSource` or `SourceSummary`), so a benchmark run can
   distinguish dependencies that should be emitted from dependencies that only
   provided declarations and facts. Source-dependency aggregate summaries now
   use dedicated report structs with a required source-root mode instead of
   reusing external-pack summaries with an optional mode, so an unlabeled
   `SourceDependency` row cannot hide a co-translation/source-summary boundary
   decision. The first
   context-driven constructor slice closes the clear HTSJDK VCF reader/header
   tail by modeling `VCFFileReader(Path)`,
   `VCFSimpleHeaderLine(String,Map<String,String>)`,
   `VCFSimpleHeaderLine(String,String,String)`,
   `VCFFileReader.getFileHeader`, and `VCFFileReader.iterator` as VCF
   external-pack facts. This reduces external constructor guesses to 15 and
   project constructor guesses to 43 while preserving zero unexplained calls
   and constructors. It also locks in the all-or-nothing rule for selected
   external packs: adding a type to the pack boundary means adjacent used
   public APIs must be modeled or explicitly marked unsupported, not left to
   generic conservative fallbacks. The next measured pack slice applies the
   same rule to a narrow Apache Commons Math optimization boundary inspired by
   `OptimizationUtils`: `SearchInterval(double,double,double)` and
   `UnivariateObjectiveFunction(function::method)` are explicit
   `org.apache.commons.math3.optim.univariate` external fact-model constructors
   with exact owned type patterns. `BrentOptimizer(double,double)` and the
   adjacent `GoalType.MAXIMIZE/MINIMIZE` constants are also pack-owned so the
   optimizer boundary is not split between the external pack and generic
   stdlib enum fallbacks. This intentionally does not claim the whole optimizer
   package yet; `UnivariateOptimizer.optimize`, `UnivariatePointValuePair`, and
   remaining optimizer result surfaces should be promoted together only when
   their used public API surface is modeled or marked unsupported. The full
   GATK benchmark after this slice has 13 external
   constructor guesses, 43 project constructor guesses, zero unexplained calls,
   zero unexplained constructors, 273 external-pack constructor facts, and 486
   external-pack explained constructors. The next constructor-tail slice
   promotes Picard command-line program groups into a coherent external
   fact-model pack instead of extending the legacy stdlib Picard constructor
   helper. The selected boundary covers all eight
   `picard.cmdline.programgroups` classes used by `HelpConstants`, including
   constructors and typed `getName`/`getDescription` getters. This removes the
   final three Picard variant program-group constructor guesses and leaves 10
   external constructor guesses, 43 project constructor guesses, zero
   unexplained calls, zero unexplained constructors, 281 external-pack
   constructor facts, and 494 external-pack explained constructors. Remaining
   external constructor pressure is now BigQuery/Avro parser setup, doclet
   exceptions, GCS retry settings, Spark write options, SV concordance state
   tuples, tuple/runtime URL helpers, and Kryo collection serializer setup.
   The follow-up Avro slice promotes `Schema.Parser()` and
   `GenericDatumReader(Schema)` into an `org.apache.avro` external fact-model
   pack with `Parser.parse(String) -> Schema` as a typed getter. Because the
   current parser collapses `new org.apache.avro.Schema.Parser()` to simple
   `Parser`, the pack temporarily owns simple `Parser` as a compatibility
   alias; it deliberately does not own simple `Schema`, since GATK also uses
   `com.google.cloud.bigquery.Schema` and that would turn BigQuery
   `Schema.getFields()` into a false Avro pack gap. This removes the Avro
   parser constructor row and leaves 9 external constructor guesses, 43 project
   constructor guesses, zero unexplained calls, zero unexplained constructors,
   283 external-pack constructor facts, and 497 external-pack explained
   constructors. `GenericDatumReader(Schema)` constructor precision has also
   been removed from the legacy stdlib Avro fallback, so the Avro pack is the
   only source of precise constructor provenance for that boundary. The next
   slice adds an exact Barclay help boundary for
   `DocException(String, Throwable)` and unchecked exception metadata. It
   deliberately does not own the whole `org.broadinstitute.barclay.help`
   package because GATK uses many Barclay annotations and doclet model classes
   whose behavior is not yet modeled. This removes the two doclet exception
   wrapper constructor guesses and leaves 7 external constructor guesses, 43
   project constructor guesses, zero unexplained calls, zero unexplained
   constructors, 284 external-pack constructor facts, and 499 external-pack
   explained constructors. Barclay command-line support is now split into
   adjacent narrow packs rather than generic stdlib constructor shortcuts:
   `org.broadinstitute.barclay.argparser` owns `CommandLineException`,
   `BadArgumentValue`, `MissingArgument`, and `CommandLineArgumentParser`
   constructors, including the parser-observed nested aliases
   `CommandLineException.BadArgumentValue` and
   `CommandLineException.MissingArgument`, while
   `org.broadinstitute.barclay.utils` owns `ClassFinder()`. The same narrow
   Barclay boundary now owns the GATK-observed
   command-line/help static fields:
   `SpecialArgumentsCollection.SHOW_HIDDEN`,
   `TemplateProperties.ARGUMENTS_POSITIONAL`,
   `TemplateProperties.ARGUMENT_DEFAULT_VALUE`, and
   `TemplateProperties.ARGUMENT_TYPE`, keeping CLI metadata constants out of
   generic stdlib field precision. The follow-up Scala tuple slice adds a narrow
   `scala.Tuple2`/`Tuple3`/`Tuple4` external fact-model pack with arity-specific
   constructors and `_1`/`_2`/`_3`/`_4` read-only getter facts. This removes the
   DragSTR `Tuple4` constructor row and keeps Scala tuple dependency facts out
   of the generic stdlib tuple-like helper. Apache Commons Lang tuple helpers
   (`Pair`, `ImmutablePair`, `MutablePair`, `Triple`, `ImmutableTriple`, and
   `MutableTriple`) are likewise pack-owned constructor and accessor facts
   rather than generic stdlib tuple-like provenance. The Picard program-group
   constructors have also
   been removed from the legacy stdlib Picard constructor fallback: without
   the `picard.cmdline.programgroups` pack they are conservative, and with the
   pack their constructor provenance comes from `ExternalLibraryPack` facts.
   This keeps the narrow program-group boundary in the pack manifest rather
   than in generic Picard shortcuts. The next measured constructor-tail slice classifies
   `java.net.URL(String)` as a JDK builtin utility constructor rather than as a
   GATK dependency-pack fact. This keeps standard-library modeling in
   `stdlib.rs`, removes the `URL` row from the external dependency tail, and
   leaves 5 external constructor guesses, 43 project constructor guesses, zero
   unexplained calls, zero unexplained constructors, 285 external-pack
   constructor facts, and 500 external-pack explained constructors. The next
   pack-manifest migration promotes two remaining legacy builtin dependency
   constructors into explicit external fact-model packs:
   `de.javakaffee.kryoserializers.UnmodifiableCollectionsSerializer()` and
   `org.disq_bio.disq.TempPartsDirectoryWriteOption(String)`. This keeps Kryo
   serializer registration and DISQ Spark write options out of stdlib
   catch-alls, removes both rows from the external constructor tail, and
   leaves 3 external constructor guesses, 43 project constructor guesses, zero
   unexplained calls, zero unexplained constructors, 287 external-pack
   constructor facts, and 503 external-pack explained constructors. The
   follow-up Guava immutable collection-builder slice adds an exact
   `com.google.common.collect.ImmutableMap.Builder` pack boundary, while
   keeping the parser-collapsed simple `Builder()` constructor as a
   constructor-only compatibility fact. This deliberately does not make every
   receiver named `Builder` Guava-owned, avoiding false pack diagnostics for
   protobuf, GCS, and other builder APIs. The measured GATK benchmark removes
   the `Builder 0` constructor row and leaves 2 external constructor guesses,
   43 project constructor guesses, zero unexplained calls, zero unexplained
   constructors, 288 external-pack constructor facts, and 504 external-pack
   explained constructors. `ThreadFactoryBuilder()` is now a separate narrow
   `com.google.common.util.concurrent` pack constructor rather than a generic
   Guava stdlib shortcut, keeping Guava concurrent utilities separate from the
   immutable-collection builder boundary. The Broad HTTP retry-settings slice then adds a
   narrow `org.broadinstitute.http.nio` fact-model pack for
   `HttpFileSystemProviderSettings`, nested `RetrySettings`, the
   `DEFAULT_SETTINGS` field, and the `redirect()` getter used by
   `BucketUtils`. `DEFAULT_SETTINGS` is now also typed by the Broad HTTP pack
   instead of the stdlib field fallback. The same Broad HTTP pack now owns
   `RetryHandler.DEFAULT_RETRYABLE_HTTP_CODES`,
   `RetryHandler.DEFAULT_RETRYABLE_EXCEPTIONS`, and
   `RetryHandler.DEFALT_RETRYABLE_MESSAGES`, keeping retry policy constants
   out of generic stdlib field precision. `HttpFileSystemProvider.SCHEME` and
   `HttpsFileSystemProvider.SCHEME` are likewise Broad HTTP pack-owned fields
   rather than generic string constants. This removes the `RetrySettings 5` constructor row and
   leaves 1 external constructor guess, 43 project constructor guesses, zero
   unexplained calls, zero unexplained constructors, 290 external-pack
   constructor facts, and 506 external-pack explained constructors. The Broad
   HTTP settings constructor has also been removed from the legacy stdlib
   constructor fallback, so `HttpFileSystemProviderSettings(...)` is
   conservative without the Broad HTTP pack and `ExternalLibraryPack`-owned
   with it. The Picard
   VCF concordance-state slice covers
   `GenotypeConcordanceStates.TruthAndCallStates`, its nested truth/call state
   enum values, constructor, and public tuple fields as a `picard.vcf`
   external fact-model pack. The `truthState` and `callState` fields are now
   precise only through the pack rather than the generic stdlib public-data
   field fallback. `GenotypeConcordanceStates.CallState` and
   `GenotypeConcordanceStates.TruthState` are also typed by the Picard VCF
   pack rather than generic stdlib nested-enum shortcuts.
   `GenotypeConcordance.CONTINGENCY_STATE_TAG` is now also a Picard VCF
   pack-owned string field rather than a generic stdlib constant. The GATK-observed
   state constants `CallState.HOM_REF`, `CallState.HET_REF_VAR1`,
   `TruthState.HOM_REF`, and `TruthState.NO_CALL` are now Picard-VCF-pack-owned
   fields rather than shared enum constants. Picard `DuplicationMetrics`
   public metric fields are now owned by a narrow `picard.analysis.metrics`
   external fact-model pack, with exact string, long, and double field types
   rather than generic stdlib public-data shortcuts. This
   same rule now covers `VariantContext.Type`: the nested type qualifier is a
   `variantcontext` pack field fact, and the GATK-observed `Type.INDEL`,
   `Type.NO_VARIATION`, `Type.SNP`, `Type.MNP`, and `Type.SYMBOLIC` constants plus
   `Genotype.UNPHASED_ALLELE_SEPARATOR`, `CommonInfo.NO_LOG10_PERROR`, and
   `VariantContext.NO_LOG10_PERROR` are variantcontext-pack-owned fields
   rather than shared enum or generic stdlib constants.
   `GenotypeLikelihoodsAllelePair.alleleIndex1/alleleIndex2` are also
   variantcontext-pack-owned public fields rather than generic stdlib
   public-data shortcuts. `JexlVCMatchExp.name/exp` follow the same rule:
   they are HTSJDK variantcontext public-data facts, not generic stdlib
   shortcuts. `Allele.create` is also modeled as a variantcontext-pack-owned
   typed factory rather than a stdlib method shortcut; `Allele.getBaseString`,
   `Allele.getBases`, and `Allele.length` are likewise receiver-specific
   non-null typed VariantContext pack effects, while generic return-type
   inference can still recognize reusable getter spellings without claiming
   API-effect provenance when an exact pack fact exists. The same
   variantcontext boundary now owns `Allele` static object/string constants
   such as `NO_CALL`, `SV_SIMPLE_DEL`, `NON_REF_STRING`, and
   `UNSPECIFIED_ALTERNATE_ALLELE_STRING`, plus
   `GenotypeLikelihoods.MAX_DIPLOID_ALT_ALLELES_THAT_CAN_BE_GENOTYPED`,
   instead of relying on generic stdlib enum/string constant shortcuts. The
   same boundary cleanup now applies to the VCF header pack:
   `VCFConstants` typed fields such as `PASSES_FILTERS_v4`,
   `GENOTYPE_ALLELE_DEPTHS`, `INFO_FIELD_ARRAY_SEPARATOR_CHAR`,
   `INFO_FIELD_ARRAY_SEPARATOR`, `DEPTH_KEY`, `ALLELE_NUMBER_KEY`,
   `ALLELE_COUNT_KEY`, `ALLELE_FREQUENCY_KEY`, `MAX_GENOTYPE_QUAL`,
   `NULL_ALLELE`, and related constants are now VCF pack-owned facts rather
   than generic stdlib typed constants.
   `VCFCompoundHeaderLine.SupportedHeaderLineType` is typed by the HTSJDK VCF
   pack instead of the stdlib nested-enum shortcut, and
   `SupportedHeaderLineType.FORMAT/INFO` are now VCF-pack-owned fields rather
   than shared enum constants.
   `VariantContextWriterBuilder.OutputType` is also a VCF pack field fact
   rather than a generic nested-enum shortcut. `DEFAULT_OPTIONS` and selected
   `Options` enum values are now VCF pack field facts instead of separate
   stdlib field-policy items. The same VCF pack owns the GATK-shaped
   `VariantContextWriterBuilder` lifecycle (`setOutputPath`,
   `setOutputFileType`, `setReferenceDictionary`, `setOption`, `setCreateMD5`,
   `determineOutputTypeFromFile`, and `build`) and the `VariantContextWriter`
   write lifecycle (`setHeader`, `writeHeader`, `add`, `checkError`, and
   `close`).
   `SamPairUtil.PairOrientation` and
   `BlockCompressedInputStream.FileTermination` follow the same rule in the
   samtools-util pack. `PairOrientation.FR/RF/TANDEM` and
   `FileTermination.DEFECTIVE`/`HAS_TERMINATOR_BLOCK` are now
   samtools-util-pack-owned fields rather than shared enum constants.
   SAM record public tag-value tuple fields, `SAMTagAndValue.tag/value` and
   `SAMBinaryTagAndValue.tag/value`, are now SAM-record-pack-owned fields
   rather than generic stdlib public-data shortcuts.
   `IndexFactory.IndexType` and
   `IndexFactory.IndexBalanceApproach` now follow the same rule in a narrow
   `htsjdk.tribble.index` pack. `IndexType.TABIX` and
   `IndexBalanceApproach.FOR_SEEK_TIME`, plus the tabix coordinate constants
   `TabixFormat.GENERIC_FLAGS` and `TabixFormat.ZERO_BASED`, are now
   tribble-index-pack-owned fields rather than shared enum or generic stdlib
   constants. `SamReaderFactory.Option` now
   follows the same rule in a narrow
   `htsjdk.samtools.reader` pack; that pack intentionally owns qualified
   `SamReaderFactory.Option` forms but not the ambiguous bare `Option` simple
   name. Qualified constants such as
   `SamReaderFactory.Option.CACHE_FILE_BASED_INDEXES` and
   `SamReaderFactory.Option.INCLUDE_SOURCE_IN_RECORDS` are SAM-reader-pack-owned
   fields, while ambiguous bare constants such as
   `Option.CACHE_FILE_BASED_INDEXES` remain shared enum-constant policy.
   `Percentile.EstimationType` and the `Percentile`
   zero/one-argument constructors now belong to a narrow
   `org.apache.commons.math3.stat.descriptive.rank` pack rather than the
   generic Apache Commons constructor fallback; `EstimationType.R_1` is now
   stats-pack-owned rather than shared enum-constant policy.
   `FieldValue.Attribute` and `QueryJobConfiguration.Priority` now follow the
   same rule in a narrow `com.google.cloud.bigquery` pack; it intentionally
   owns the qualified nested enum forms but not the ambiguous bare `Attribute`
   or `Priority` simple names. Qualified constants such as
   `FieldValue.Attribute.PRIMITIVE`, `QueryJobConfiguration.Priority.BATCH`,
   and `QueryJobConfiguration.Priority.INTERACTIVE` are BigQuery-pack-owned,
   while ambiguous bare constants such as `Attribute.PRIMITIVE` and
   `Priority.BATCH` remain shared enum-constant policy.
   `Storage.SignUrlOption` now follows the same rule in a narrow
   `com.google.cloud.storage` pack. Adjacent Google Cloud NIO fields
   `CloudStorageConfiguration.DEFAULT` and `CloudStorageFileSystem.URI_SCHEME`
   are now modeled together in a narrow
   `com.google.cloud.storage.contrib.nio` pack rather than through stdlib field
   shortcuts.
   `Log.LogLevel` now follows the same rule inside the existing
   `htsjdk.samtools.util` pack rather than through a generic stdlib nested-enum
   shortcut; constants such as `LogLevel.ERROR`, `LogLevel.WARNING`,
   `LogLevel.INFO`, and `LogLevel.DEBUG` are samtools-util-pack-owned fields
   rather than shared enum constants.
   The remaining GenomicsDB/protobuf builder qualifiers now belong to a
   coherent `com.intel.genomicsdb` pack together with the existing
   `GenomicsDBFeatureReader`, `GenomicsDBImporter`, and `ImportConfig`
   constructor facts. This moves `Coordinates.GenomicsDBColumn`,
   `Coordinates.ContigPosition`,
   `GenomicsDBExportConfiguration.ExportConfiguration`,
   `GenomicsDBImportConfiguration.ImportConfiguration`,
   `GenomicsDBImportConfiguration.Partition`,
   `GenomicsDBVidMapProto.VidMappingPB`,
   `GenomicsDBVidMapProto.FieldLengthDescriptorComponentPB`, and
   `ReadSession.TableReadOptions` out of the generic stdlib nested-enum table.
   The focused GenomicsDB slice now also models the protobuf/importer
   boundary methods that were repeatedly observed in the GATK package:
   `GenomicsDBColumn.newBuilder`, `ContigPosition.newBuilder`,
   `ExportConfiguration.newBuilder`, `ImportConfiguration.newBuilder`,
   `Partition.newBuilder`, `VidMappingPB.newBuilder`, `VidMappingPB.toBuilder`,
   `VidMappingPB.getContigsList`, `VidMappingPB.getFields`,
   `VidMappingPB.getFieldsCount`, `FieldInfo.getName`,
   `GenomicsDBImporter.getProtobufVidMapping`,
   `GenomicsDBImporter.updateProtobufVidMapping`,
   `GenomicsDBImporter.coalesceContigsIntoNumPartitions`,
   `GenomicsDBImporter.executeImport`, and the GATK-observed `ImportConfig`
   output/callback mutators. `ThreadFactoryBuilder.setNameFormat`,
   `ThreadFactoryBuilder.setDaemon`, and `ThreadFactoryBuilder.build` are
   modeled in the adjacent Guava concurrent pack because the GenomicsDB slice
   uses that builder at the same external boundary. Neutral tests cover both
   direct pack facts and small GATK-shaped semantic snippets.
   The focused benchmark for
   `org/broadinstitute/hellbender/tools/genomicsdb` now reports
   `semantic_diagnostics 0`,
   `external_api_missing_method_calls 0`,
   `external_api_missing_constructor_calls 0`, and
   `external_api_missing_field_accesses 0`. The last false diagnostic in this
   slice was `Partition.split`, which was not a GenomicsDB API gap: the source
   expression was a stream lambda parameter named `partition` whose element
   type is `String`. That is now handled by resolver/semantic responsibility
   separation: typed lambda calls are supplied through resolver unresolved
   target facts, while the semantic external-effect expression walker skips
   lambda bodies instead of guessing receiver types from lambda parameter
   names. This avoids adding `split` to the GenomicsDB pack.
   `MarkDuplicates.DuplicateTaggingPolicy` is now modeled by a
   narrow `picard.sam.markduplicates` pack rather than by the generic stdlib
   nested-enum table or the unrelated Picard program-groups pack. Its
   `DontTag`, `OpticalOnly`, and `All` constants are now markduplicates-pack
   fields rather than shared enum constants. `OpticalDuplicateFinder`
   constructors are now owned by the same markduplicates pack, and
   `ReadEnds` orientation byte constants (`FF`, `FR`, `RF`, `RR`, `F`, `R`)
   are modeled there instead of by generic stdlib byte-constant policy, while
   `GenotypeConcordanceCounts` and `GenotypeConcordanceSummaryMetrics`
   constructors are owned by the `picard.vcf` pack rather than generic stdlib
   Picard constructor precision. The summary metric label constants, including
   `GENOTYPE_CONCORDANCE`, `HET_PPV`, and `VAR_SPECIFICITY`, are also
   `picard.vcf` modeled fields rather than generic stdlib string constants.
   This
   removes the final `TruthAndCallStates 2`
   row: the latest GATK benchmark has zero external constructor guesses, 43
   project constructor guesses, zero unexplained calls, zero unexplained
   constructors, 291 external-pack constructor facts, and 507 external-pack
   explained constructors. The remaining semantic diagnostics are the two
   deliberate `VariantContext.fullyDecode` unsupported-boundary diagnostics.
   The focused `tools/walkers/variantutils` package also demonstrates the
   source-boundary side of the plan. When benchmarked alone, its remaining
   constructor and field pressure includes GATK-owned filter helpers, interval
   helpers, annotation/genotyping engines, and argument-collection fields, not
   external libraries. Adding source-summary roots for `engine`, `utils`,
   `cmdline/argumentcollections`, `tools/walkers/annotator`,
   `tools/walkers/haplotypecaller`, `tools/walkers/genotyper`, `exceptions`,
   and `tools/genomicsdb` moves those rows into
   `source_dependency_resolved_*`: external constructor guesses, project
   constructor guesses, source constructor guesses, resolver diagnostics, and
   all field-guess buckets are zero, while
   `GenomicsDBOptions`, `UserException.BadInput`,
   `UserException.ValidationFailure`, `UserException.MissingReference`, and
   other nested project exception constructors remain auditable as source
   dependency constructor-context rows rather than as external packs. The
   nested-constructor part is resolved by source-symbol aliases for
   outer-qualified names, not by a GATK exception special case, so source
   summaries can cover both imported simple nested names and qualified
   `Outer.Inner` constructor calls. Likewise,
   `DbsnpArgumentCollection.dbsnp`,
   `HaplotypeCallerArgumentCollection.standardArgs`,
   `StandardCallerArgumentCollection.genotypeArgs`, and
   `GenotypeCalculationArgumentCollection.standardConfidenceForCalling` remain
   auditable in source-dependency field-context rows. The neutral regression
   `source_summary_roots_resolve_nested_argument_collection_field_chains`
   locks in nested argument-collection fields, and
   `source_summary_roots_resolve_project_helper_fields_and_nested_exception_constructors`
   locks in qualified nested source-summary constructors without depending on
   GATK names.
   The same focused benchmark then exposed an object-model issue rather than a
   dependency issue: interface constants, static interface methods, and nested
   helper declarations were being counted as instance trait object-safety
   violations. Object-safety fact generation now excludes those static/nested
   members from the enclosing interface decision, while still rejecting real
   self-referential instance methods. Source-summary interface constants are
   now parsed as implicit public static final fields and benchmarked through
   the same source-dependency field context path as class fields, including
   snake-case aliases used by type-qualified static field receivers. A focused
   regression proves a source-summary interface can expose a constant, a static
   factory, and an instance predicate without creating object-safety
   diagnostics or source field guesses. A follow-up object-safety refinement
   treats Java-style combinators named `and`, `or`, `negate`, `andThen`, or
   `compose` that return the same interface as dyn-trait candidates. Direct
   self-typed parameters are also object-safe at the fact layer because they
   can be represented as trait-object parameters; nested generic self
   parameters remain conservative until their slot-shape lowering is proven.
   A source-summary regression now locks that distinction at the benchmark
   boundary: `DirectSelf.accept(DirectSelf)` remains dyn-trait viable, while
   `NestedSelf.acceptAll(List<NestedSelf>)` remains visible erased-object
   pressure instead of being silently accepted.
   The source-boundary object pass now also treats source-summary superclasses
   used by emitted roots as erased-object framework boundaries, because their
   bodies are intentionally not emitted. Co-translated superclasses still remain
   emitted inheritance pressure. On the expanded `variantutils` source-summary
   run these general rules reduce `object_representation_diagnostics` from 85 to
   0 while preserving source-dependency method, constructor, and field
   attribution.
   Apache Commons Collections helper constructors are now modeled by a narrow
   `org.apache.commons.collections4` external fact-model pack rather than by
   the generic stdlib constructor fallback. The pack covers the GATK-observed
   `DefaultedMap(defaultValue)`, `FilterIterator(iterator,predicate)`,
   `SingletonIterator(value)`, and `MultiMap()` constructors. These are
   dependency-boundary helper types, not Java platform collection primitives,
   so keeping them in a pack preserves the all-or-nothing dependency boundary
   rule while still giving semantic analysis constructor facts for
   fixture-sized GATK-inspired snippets.
   Jama and Hipparchus constructor precision now follows the same rule:
   `Jama.Matrix` construction and `EmpiricalDistribution()` are modeled by
   narrow `Jama` and `org.hipparchus.random` external fact-model packs instead
   of by generic stdlib constructor shortcuts. These packs deliberately model
   only the GATK-observed constructor surfaces for now; richer matrix or
   distribution lowering remains a future facade/slot-shape decision. The
   `utils/spark` benchmark then promoted the clear Jama `Matrix` read surface
   into the same pack: `numRows`, `numCols`, and `toArray` are now zero-arity
   pack-owned getter facts with typed non-null returns. This keeps Spark
   converter matrix dimensions and array conversion out of generic method
   guessing while leaving matrix lowering policy for a later facade decision.
   Apache HTTP client construction is now also pack-owned:
   `HttpGet(URI)` and `PoolingHttpClientConnectionManager()` are modeled by a
   narrow `org.apache.http` external fact-model pack instead of by stdlib
   constructor shortcuts. The same pack now models the GATK `htsgetreader`
   request-mutation surface `HttpGet.addHeader`, removing the missing-pack
   diagnostic from that narrow real-corpus benchmark. This keeps Apache HTTP
   separate from the Broad HTTP NIO settings pack and preserves dependency
   attribution for future facade mapping decisions.
   Apache Commons Compress and Commons Lang helper construction now follows
   the same boundary rule. `GzipCompressorInputStream`,
   `GzipCompressorOutputStream`, null streams, tar archive streams/entries,
   `HashCodeBuilder`, mutable number wrappers, and
   `NotImplementedException` constructors are modeled by narrow
   `org.apache.commons.compress` and `org.apache.commons.lang3` packs rather
   than stdlib constructor shortcuts. Commons Compress static fields observed
   in GATK-shaped snippets are also pack-owned now:
   `TarArchiveOutputStream.BIGNUMBER_STAR`,
   `TarArchiveOutputStream.LONGFILE_GNU`,
   `NullOutputStream.INSTANCE`, and
   `NullOutputStream.NULL_OUTPUT_STREAM` are typed by the Commons Compress
   pack rather than by stdlib field shortcuts. `NullWriter.INSTANCE` follows
   the same Commons IO singleton pattern and is also pack-owned now.
   Commons Lang static platform constants now follow the same manifest-owned
   rule: `SystemUtils.OS_ARCH`, `SystemUtils.OS_NAME`,
   `SystemUtils.IS_OS_MAC`, `SystemUtils.IS_OS_LINUX`, and
   `SystemUtils.IS_OS_WINDOWS` are modeled fields of the
   `org.apache.commons.lang3` pack. This keeps dependency platform facts out of
   generic stdlib typing and leaves future lowering free to map them to a
   Commons facade or direct Rust environment probes.
   Apache Commons Codec digest constants now use the same pack-owned path:
   `MessageDigestAlgorithms.SHA_256` is a modeled field of the narrow
   `org.apache.commons.codec` pack rather than a generic stdlib field shortcut.
   Apache Commons Math constructor precision has also been drained out of
   stdlib. The remaining GATK-shaped math constructors for linear algebra,
   distributions, descriptive statistics, random generators, and exceptions are
   now owned by a narrow `org.apache.commons.math3` fact-model pack; the
   existing optim and stats packs keep `BrentOptimizer` and `Percentile`
   surfaces, and `MutableDouble` is correctly owned by the Commons Lang pack.
   This keeps math dependency attribution in pack manifests instead of a broad
   builtin constructor table.
   The core Kryo serializer runtime constructors are now also modeled by a
   narrow `com.esotericsoftware.kryo` external fact-model pack: `Kryo()`,
   `Input(InputStream)`, `Output(OutputStream)`,
   `FieldSerializer(Kryo,Class)`, `ImmutableMapSerializer()`, and the
   mutating `Kryo.register` overloads used by serializer setup. The same
   pack now owns the GATK-observed primitive IO surface on Kryo `Input` and
   `Output`: boolean, byte, short, int, long, float, byte-array, and string
   reads/writes carry receiver mutation, return type, and nullability facts in
   the external pack layer. The byte/long/float/byte-array refresh is driven
   by a neutral serializer workflow and by the focused GATK evidence package,
   where pack-owned Kryo missing API diagnostics dropped from 54 to 22 without
   widening the primitive-IO slice to object graph serialization APIs. The
   follow-up object-serialization/lifecycle slice then models
   `Kryo.readClassAndObject`, `Kryo.writeClassAndObject`, and `Output.close`
   through a separate neutral fixture, reducing the same focused evidence
   package to 17 pack-owned missing API calls. This keeps primitive stream IO
   and object graph serialization adjacent but independently auditable inside
   the Kryo pack.
   It also owns the qualified erased superclass boundary
   `com.esotericsoftware.kryo.Serializer` without claiming the ambiguous bare
   simple name `Serializer`; hierarchy fact collection must preserve the
   qualified declared superclass when source subclasses extend that runtime
   type.
   The
   `de.javakaffee.kryoserializers` pack remains a separate helper-serializer
   boundary for `UnmodifiableCollectionsSerializer()`. The generic stdlib
   constructor fallback no longer claims these core Kryo constructors, so
   provenance is pack-owned when the pack is loaded and conservative
   otherwise.
   OpenCSV reader/writer construction and the common GATK-shaped CSV IO
   surface are now modeled by a narrow `com.opencsv` external fact-model pack:
   `CSVReader(Reader)`, `CSVReader(Reader,char,char,char)`,
   `CSVWriter(Writer,char)`, `CSVWriter(Writer,char,char,char)`,
   `CSVReader.readNext/readAll/close`, and
   `CSVWriter.writeNext/writeAll/flush/close`. `readNext` is explicitly
   nullable at EOF and reader/writer calls mutate receiver state. The generic
   stdlib constructor fallback no longer claims `CSVReader` or `CSVWriter`, so
   these constructors are pack-owned when the pack is loaded and conservative
   otherwise.
   Hadoop `Configuration` is now modeled by a narrow
   `org.apache.hadoop.conf` external fact-model pack rather than a generic
   stdlib constructor shortcut. The pack covers `Configuration()`, common
   `get`/`set` string accessors, boolean and integer accessors, and their
   receiver mutability/nullability facts. This keeps Hadoop as a runtime
   boundary while still giving semantic analysis enough facts for
   GATK-shaped configuration code.
   Apache Commons Configuration is now modeled by a narrow
   `org.apache.commons.configuration2` external fact-model pack rather than a
   generic stdlib constructor shortcut. The pack covers `Configurations()`,
   file-backed `properties`/`xml`/`ini` loading, and common typed
   `getString`/`getBoolean`/`getInt`/`setProperty` facts on the specific
   returned configuration classes. It intentionally does not own the ambiguous
   bare `Configuration` name, which remains available for Hadoop and other
   package-specific boundaries.
   Spark runtime construction is now modeled by a narrow `org.apache.spark`
   external fact-model pack rather than a generic stdlib constructor shortcut.
   The pack covers `SparkConf`, `HashPartitioner`, `JavaSparkContext`,
   `JavaRDD`, `JavaPairRDD`, `PartitionGroup`, and `RowMatrix` constructors
   plus common `SparkConf`, `JavaSparkContext`, `JavaRDD`, `JavaPairRDD`,
   `Broadcast`, and `RowMatrix` API effects used by GATK-shaped Spark code.
   The current `utils/spark` benchmark slice has zero semantic diagnostics
   after modeling runtime context/configuration access, partition
   transformations, pair-RDD sorting/key extraction, raw RDD bridging, and
   broadcast teardown as Spark pack facts. The same focused package now also
   has zero object-representation diagnostics: anonymous implementations of
   explicitly erased external boundary types, such as Guava
   `AbstractIterator`, are represented as erased external values instead of as
   unsupported anonymous source classes. Spark stays a runtime boundary;
   generic element propagation for calls such as
   `JavaSparkContext.parallelize` remains in the existing argument-sensitive
   type inference path until the semantic fact layer can represent dependent
   return types directly. The remaining two comparator constructor guesses in
   the primary-only `utils/spark` run are source-dependency pressure, not
   external-pack pressure: adding `utils/read` as a co-translated source root
   resolves `ReadCoordinateComparator(1)` and `ReadQueryNameComparator(0)` as
   source constructors, and resolves four source-dependency method calls on
   `GATKRead.get_name` and `ReadUtils.is_read_name_grouped_bam`. That makes the
   next design decision whether `utils/read` should be emitted, summarized, or
   excluded for the Spark adapter benchmark. The co-translated run also
   exposed a reusable resolver rule rather than a Spark or GATK pack rule:
   type-qualified enum constants from an imported but absent enum, such as
   `Tail.LEFT`/`Tail.RIGHT`, should conservatively type as the receiver enum
   type after source fields, source summaries, user facts, and pack fields have
   had precedence. This lets source overload selection use the same field/type
   fact layer without adding one-off `countClippedBases` mappings.
   A later ownership-accounting slice applies the same principle to conservative
   field fallback reports: if an omitted project/source utility is imported as
   `BaseUtils` but appears in resolver facts as `base_utils`, explicit import
   ownership is compared through the same snake-case aliases used elsewhere.
   Ownership also propagates through type-like static-field chains such as
   `BaseUtils.Base.N.base`, so the focused `utils/read` benchmark now reports
   `external_field_guesses = 0` and keeps those BaseUtils enum/value accesses
   under project field pressure. This is classification only; it does not add
   BaseUtils lowering or a BaseUtils external pack.
   Constructor fallback attribution now follows the same responsibility split
   for nested type constructors. The parser preserves type-like dotted
   constructor names in `new Outer.Inner(...)`, and resolver-benchmark
   ownership checks the outer type before classifying the conservative
   constructor fact. In the focused `utils/read` benchmark, nested
   `GATKException.ReadAttributeTypeMismatch`,
   `UserException.CouldNotReadInputFile`, and
   `UserException.MissingReference` constructors now report as project
   pressure because their outers are explicitly imported from the project
   package, while the true HTSJDK reference boundary,
   `ReferenceSequenceFileWalker`, is now owned by the SAM reference pack
   together with its `getSequenceDictionary` and indexed `get` accessors.
   The focused `utils/read` benchmark now has zero external constructor
   guesses and zero semantic diagnostics. Generic dotted builders such as
   `CollectionUtil.MultiMap<>` continue through existing external-pack
   matching instead of being reclassified by the nested-constructor rule. The
   nested-exception attribution remains classification only; exception lowering
   still belongs in the exception hierarchy/object-model plan, not in a
   GATK-specific pack rule.
   The same focused Spark/read boundary is now covered in `source_summary`
   mode as well: summary declarations resolve the `GATKRead.get_name`,
   `ReadUtils.is_read_name_grouped_bam`, `ReadCoordinateComparator(1)`, and
   `ReadQueryNameComparator(0)` dependency edges without benchmarking or
   emitting `utils/read` method bodies. Resolver benchmark object-shape counts
   still consume source-summary declaration facts, but object-lowering
   diagnostics are collected only from emitted roots, so anonymous classes and
   inheritance inside summary-only dependency files do not appear as
   translation pressure for the Spark adapter.
   The focused Spark/SV evidence package later exposed the remaining Spark
   runtime tail: broadcast value accessors, `JavaRDD.mapPartitions` and
   `mapPartitionsToPair` overloads with partition-preservation flags,
   `saveAsTextFile`, `unpersist`, and adjacent `JavaPairRDD`
   `reduceByKey`, `partitionBy`, `mapPartitions`, `filter`, `flatMap`, `map`,
   and `combineByKey` overloads. These are now modeled by the same Spark pack
   with neutral Spark workflow tests. In that package, pack-owned missing API
   calls dropped from 17 to 5, and Spark no longer appears in the missing-pack
   rows; the remaining rows are HTSJDK CIGAR and BWA native-wrapper surfaces.
   The follow-up tail slice models `Cigar.getPaddedReferenceLength` in the
   HTSJDK SAM-record pack and completes the evidence-package BWA surface in the
   BWA pack: `BwaMemAligner.alignSeqs`, `setIntraCtgOptions`,
   `setZDropOption`, the `BwaMemAlignment` 16-argument serializer
   reconstruction constructor, and the observed alignment value getters. This
   is the dependency-pack policy in practice: once a selected corpus forces an
   external value type to become pack-owned, the constructor/getter surface used
   by that corpus must be covered by the same pack rather than left as generic
   fallback. The focused Spark/SV evidence benchmark now has
   `semantic_diagnostics = 0`,
   `external_api_missing_method_calls = 0`, and
   `external_api_missing_constructor_calls = 0`; remaining benchmark pressure is
   in generic guesses and object-lowering diagnostics, not external-pack
   coverage. The next object-model validation slice keeps that distinction
   explicit: primary-source object diagnostics must be planned with the same
   package-level source context used by resolver symbols, not with isolated
   per-file programs. This matters for user-injected hierarchy facts on
   GATK-shaped source hierarchies such as `BreakpointEvidence`: without a
   package context, an override can be falsely rejected as referencing unknown
   variants in sibling files. The benchmark path now has a regression proving
   a primary-source `BreakpointEvidence`-style hierarchy stays diagnostic by
   default, but a user `ClosedEnum` override makes its represented `extends`
   edges non-diagnostic and contributes a closed-enum representation count.
   The same escape hatch is now covered through the line-oriented external fact
   syntax (`closed-enum BreakpointEvidence BreakpointEvidenceValue ...`), so
   users can supply hierarchy strategy evidence without writing Rust-side test
   facts or lowering special cases. That is the intended path for public,
   non-final Java hierarchies where the corpus suggests an enum mapping but
   the source alone does not prove a closed world. The override validation now
   accepts terminal descendants, not only direct subclasses, and represented
   ancestor hierarchies cover intermediate `extends` edges. A real
   `BreakpointEvidence` override fact for the focused Spark/SV evidence
   hierarchy drives `object_representation_diagnostics` from 12 to 0 while
   leaving external-pack diagnostics at zero.
   Top object-representation
   diagnostics now also carry the emitted source-root mode (`PrimarySource` or
   `CoTranslateSource`), so object-model pressure from co-translated
   dependencies can be separated from primary-source pressure while
   source-summary-only bodies remain excluded from emitted object diagnostics.
   The same focused evidence package now has an explicit source-summary
   boundary check for project helpers and exceptions. With only the evidence
   root, the current benchmark reports 17 external constructor guesses, 51
   project constructor guesses, two external field guesses, and 47 project
   field guesses. The field split is intentional: public argument-collection
   fields and sibling GATK helper fields are omitted-project-source pressure,
   not external-library pressure. Adding `exceptions` and `tools/spark/sv` as
   `source-summary` roots drives external/project/source field-guess counters
   to zero and reports 58 source-dependency constructors, 83 source-dependency
   method calls, and 71 source-dependency field accesses instead. Overlapping
   roots are now assigned by most-specific path, so the broad `tools/spark/sv`
   source-summary root does not also parse the primary
   `tools/spark/sv/evidence` files as summaries; the focused run reports 28
   primary files, three exception summary files, and 64 sibling Spark/SV
   summary files. Neutral regressions mirror both shapes: source-summary
   public helper fields and nested exception constructors are tracked as a
   source-boundary decision rather than as an external-pack expansion,
   project-owned field fallback is reported separately from external-library
   fallback, and broader source-summary parents cannot duplicate
   more-specific primary roots.
   BWA native wrappers are now modeled by a narrow
   `org.broadinstitute.hellbender.utils.bwa` external fact-model pack rather
   than generic stdlib shortcuts. The pack covers `BwaMemIndex(path)`,
   `BwaMemAligner(index)`, the `BwaMemAligner.MEM_F_ALL` flag field, aligner
   configuration/alignment calls, and the serializer-observed
   `BwaMemAlignment` value surface, so native-resource ownership and constants
   stay together at the dependency boundary. DISQ write options now follow the
   same boundary rule:
   `ReadsFormatWriteOption.BAM` and
   `FileCardinalityWriteOption.MULTIPLE/SINGLE` are DISQ-pack-owned fields,
   not generic stdlib typed constants, so option constructors and option enum
   values stay in one external fact-model pack. The same `org.disq_bio.disq`
   pack now owns the GATK-observed runtime constructors
   `HtsjdkReadsRdd`, `HtsjdkVariantsRdd`, and
   `HtsjdkReadsTraversalParameters`, so DISQ RDD/traversal setup no longer
   depends on generic stdlib constructor precision. Intel GKL native wrappers now
   have the same treatment through a narrow `com.intel.gkl` pack:
   `IntelDeflaterFactory`, `IntelInflaterFactory`, `IntelPDHMM`,
   `IntelPairHmm`, and `IntelPairHmmOMP` constructors plus
   `PDHMMNativeArguments`/`PairHMMNativeArguments`,
   `ReadDataHolder`/`HaplotypeDataHolder`,
   `BatchCompletionCallbackFunctionArgument`, and
   `SWNativeAlignerResult` public fields are pack-owned facts rather than
   generic stdlib constructor or field shortcuts. A focused semantic fixture
   now proves both native-wrapper constructors and typed public field reads are
   admitted as `ExternalLibraryPack` facts without relying on unresolved-field
   fallback.
7. Add facade mapping facts for HDF5 through `hdf5-pure-rust` and evaluate
   SQLite through the `sqlite` crate. Status: implemented as crate-mapping
   external packs targeting `hdf5_pure_rust` and `sqlite`, with constructor,
   method, mutability, nullability, typed return facts for selected calls,
   external error facts for selected HDF5 runtime operations, checked
   `SQLException` facts for selected SQLite connection and statement
   operations, crate/facade receiver-call and constructor planning for HDF5 and
   SQLite, lowering-level facade rendering checks for both crates, and
   benchmark attribution tests. The latest HDF5 string-array read refresh adds
   `HDF5File.readStringArray` as a mapped facade getter returning `String[]`
   and throwing `HDF5Error`. The SQLite facade refresh adds checked
   `SQLException` metadata for `createConnection`, statement preparation,
   statement execution, statement lifecycle through `clearParameters` and
   `close`, transaction configuration through `setAutoCommit`, result
   conversion through `getLong` and `getBoolean`, transaction side effects, and
   close operations while keeping local config conversion such as
   `toProperties` non-throwing. SQLite now has the same
   all-or-nothing crate-boundary diagnostics as HDF5:
   unmodeled methods, constructors, and fields on pack-owned SQLite facade
   types report pack-owned missing-member diagnostics instead of falling
   through to generic external guesses. Current
   crate-only GATK benchmark evidence shows HDF5/SQLite Rust crate-mapping
   packs explain 61 method calls and 15 constructors, with zero unexplained
   method calls or constructors in that controlled mode. HDF5 and SQLite
   constructor precision has also been removed from the legacy stdlib
   constructor fallback: without the crate-mapping packs, `HDF5File(...)` and
   `SQLiteConfig()` are conservative external constructors; with the packs,
   their constructor provenance comes from `ExternalLibraryPack` facts. HDF5
   `OpenMode` fields (`HDF5File.OpenMode`, `READ_ONLY`, and `CREATE`) are now
   typed by the HDF5 crate-mapping pack rather than generic stdlib enum
   shortcuts. This
   keeps runtime facade ownership in the external pack layer instead of in
   standard-library shortcuts. The focused HDF5 workflow fixture now exercises
   open/check/write/flush/close plus scalar, array, and matrix reads/builders,
   plus `HDF5Library.load`, so resource lifecycle and dataset shape facts are
   covered together at the crate-mapping boundary. Call-plan coverage now also
   verifies that `HDF5Library.load` selects and renders the
   `hdf5_pure_rust::load` facade target through crate-mapping facts, matching
   the existing HDF5 file constructor and receiver-call facade checks.
8. Update `docs/gatk-resolver-benchmark-report.md` with results from GATK
   alone versus GATK plus selected dependency facts. Status: implemented and
   refreshed after manifest diagnostics, mode attribution, user-authored
   manifest inputs, explicit field manifest coverage, provider fact-count
   attribution, and `IntervalList` constructor coverage. The report includes
   baseline history and current dependency-pack/crate-mapping counters from the
   full GATK benchmark.
