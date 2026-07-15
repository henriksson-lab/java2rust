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
- Item 3 is covered by configured `SourceSummary` fact providers and by a real
  `SourceSummary` Java source root in `tests/dependency_mode.rs`; summary-root
  declarations participate in the resolver source index while summary files
  are not benchmarked as emitted source.
- Items 4 and 5 are covered by `tests/external_packs.rs` through pack-owned
  method/constructor/field facts and unsupported/missing API diagnostics.
- Item 6 is covered for the active external fact paths: user overrides beat
  external pack facts, Rust crate-mapping facts, and built-in external facts.
  `EffectiveFacts` also covers user override precedence over parsed source
  hierarchy/object facts.
- Item 7 is covered by a neutral dependency-mode fixture in
  `tests/dependency_mode.rs`: a primary app root plus a co-translated
  dependency root resolves the dependency source and lets the object model
  choose a closed enum for a sealed dependency interface.
- Item 8 is covered at the conversion-planner boundary in
  `tests/type_coercion.rs`: a Rust crate-mapped HDF5 handle and an equivalent
  co-translated helper type both use the same proven clone-recovery slot
  conversion policy. External API effect facts now also carry optional return
  types, and `tests/type_coercion.rs` verifies that HDF5 and VariantContext
  pack return-type facts feed ordinary unresolved-call type inference. A later
  lowering-level test should still check rendered crate-facade calls once
  facade call rendering exists.

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

## Benchmark Plan

Run at least three benchmark modes:

1. GATK alone, with current external facts.
2. GATK plus HTSJDK external fact packs for variant, VCF, util, tribble,
   header, and sequence dictionary APIs.
3. GATK plus crate/facade mappings for selected external libraries such as
   HDF5 and SQLite.

Compare:

- unresolved methods, constructors, and fields
- external API guesses by provenance
- fact counts and saturation time
- object strategy fallbacks caused by unknown hierarchy closure
- nullability and builder facts for HTSJDK-shaped objects
- Rust output idiom issues caused by dependency boundaries

If adding external facts reduces guesses but creates poor Rust output, refine
the facts or facade mappings. Do not partially co-translate external libraries
as a workaround unless the dependency has been promoted to a full translation
target.

## Near-Term Work

1. Add a dependency-mode configuration structure for source roots and fact
   providers. Status: implemented as `DependencyModeConfig` with explicit
   source-root modes, fact-provider modes, benchmark integration, and
   GATK-shaped tests proving that adding a dependency root changes ownership
   from external boundary to cross-root source resolution.
2. Add an external pack manifest format with package ownership, coverage
   states, provenance, unsupported entries, and crate/facade mapping hooks.
   Status: implemented as an in-Rust `ExternalPackManifest` declaration with
   owned type patterns, default coverage, mode, Rust target/facade hook, and
   unsupported API entries. Existing built-in packs now generate boundary and
   unsupported facts from those manifests. A future step can add a serialized
   manifest parser if we want user-authored pack files outside the binary.
3. Teach the resolver benchmark to report fact provenance by source root,
   external pack, crate facade, or conservative guess. Status: implemented for
   configured source roots, configured fact providers/source summaries,
   external fact-model packs, Rust crate-mapping packs, FFI/conservative pack
   buckets, and conservative guesses. Source-summary roots are now parsed into
   the source index for declaration resolution without being benchmarked as
   emitted files.
4. Implement package-owned diagnostics for calls inside an external pack that
   are declared unsupported or missing from the manifest. Status: implemented
   for unsupported methods, missing methods, missing constructors, and missing
   fields. Pack-owned missing entries now produce structured diagnostics and
   do not silently fall through to conservative external guesses.
5. Build the `htsjdk.variant.variantcontext` pilot pack and its GATK-inspired
   fixture tests. Status: implemented. The pilot pack has manifest ownership,
   builder/getter/mutator/nullability facts, typed return facts for central
   builder and getter APIs, unsupported API diagnostics, user-override
   precedence tests, semantic-analysis tests, resolver benchmark attribution
   tests, and GATK-shaped neutral fixtures.
6. Add HTSJDK external fact packs for `variant.vcf`, `samtools.util`,
   `tribble.Feature`, and selected header/dictionary models only after the
   `variantcontext` pilot proves the mechanism. Status: implemented for
   `htsjdk.variant.vcf`, `htsjdk.samtools.util`, `htsjdk.tribble.Feature` and
   related simple feature values, plus selected SAM header/sequence dictionary
   models. Current GATK benchmark evidence shows these packs explain 4,217
   method calls and 463 constructors as external fact-model coverage.
7. Add facade mapping facts for HDF5 through `hdf5-pure-rust` and evaluate
   SQLite through the `sqlite` crate. Status: implemented as crate-mapping
   external packs targeting `hdf5_pure_rust` and `sqlite`, with constructor,
   method, mutability, nullability, typed return facts for selected calls, and
   benchmark attribution tests. Current GATK benchmark evidence shows Rust
   crate-mapping packs explain 17 method calls and 15 constructors.
8. Update `docs/gatk-resolver-benchmark-report.md` with results from GATK
   alone versus GATK plus selected dependency facts. Status: implemented and
   refreshed after manifest diagnostics, mode attribution, and `IntervalList`
   constructor coverage. The report includes baseline history and current
   dependency-pack/crate-mapping counters from the full GATK benchmark.
