# External Facts File

The translator accepts line-oriented external fact files through `--facts` and
line-oriented external pack manifests through `--external-pack-manifest`. The
same options are accepted by `resolver-bench`, where each input is registered
as a dependency fact provider for benchmark attribution. `resolver-bench` also
accepts repeatable dependency source roots with `--source-root MODE:PATH`,
where `MODE` is `primary`, `co-translate`, or `source-summary`. These inputs
are parsed into `FactSet` before representation, signature, shape, and
coercion planning run. The intent is to inject corpus analysis, user
knowledge, and library-pack boundaries without adding local lowering special
cases.

Blank lines are ignored. `#` starts a comment.

## Hierarchy Closure

```text
closed Expr
closed Expr true
closed Expr false
```

`closed Type` is shorthand for `closed Type true`.

## Representation Overrides

```text
concrete User
closed-enum Expr ExprValue Add Num
dyn-trait Sink SinkTrait
boxed-dyn-trait Sink SinkTrait
erased-object ExternalNode
dispatch-requirement ExternalNode needs-subtype-dispatch
dispatch-requirement ExternalNode needs-downcast
```

These select the object representation strategy for a Java hierarchy root.
`dispatch-requirement TYPE REQUIREMENT` records representation pressure found
by source summaries or user analysis without choosing the final Rust shape in
the fact parser. Supported requirements are:

- `needs-subtype-dispatch`
- `needs-closed-runtime-case-split`
- `needs-instanceof`
- `needs-downcast`
- `needs-owned-polymorphic-storage`
- `needs-borrowed-trait-view`
- `needs-public-extensible-api`
- `reject-trait-object`
- `needs-unknown-runtime-object`

The object model and signature projection consume those requirements. Lowering
should not rediscover `instanceof`, casts, extensible APIs, or boxed storage by
library-specific Java spelling.

## Type Capabilities

```text
copy ExternalId
copy ExternalId true
copy ExternalId false
clone ExternalHandle false
string-like ExternalLabel true
sequence-like ExternalIds ExternalId
extends Dog Animal
implements Dog Pet Named
final Dog
final Dog true
object-safe-trait Pet false
object-safety-violation Pet static-method of
object-safety-violation Pet self-return next
object-safety-violation Pet nested-self-return nextAll
object-safety-violation Pet self-param accept other
object-safety-violation Pet nested-self-param acceptAll others
object-safety-violation Pet field VERSION
object-safety-violation Pet constructor
object-safety-violation Pet unsupported-member
```

Capability facts feed the same `FactSet` queries used by signature solving,
shape projection, representation planning, and coercion. For example,
`sequence-like ExternalIds ExternalId` lets stream and indexed projection treat
`ExternalIds` as a sequence of `ExternalId`, while `copy ExternalId` lets the
element shape become a copied value instead of a borrowed view.

The parser currently expects Java type names without spaces inside a fact line.
Generic type spellings that the Java type parser accepts as one token can be
used as capability values.

`object-safe-trait Type false` only records the boolean capability. The
`object-safety-violation` forms additionally record why a dyn/boxed dyn trait
mapping must fall back to erased object representation. These reasons are
consumed by representation policy diagnostics; lowering and coercion should not
inspect interface syntax to rediscover them.

## External API Effects

```text
external-constructor SummaryBuilder String int
external-api getValue * getter read-only
external-api setValue ExternalCounter setter mutates-receiver
external-api get ExternalList indexed-getter read-only
external-api put ExternalMap indexed-setter mutates-receiver
external-api read NativeHandle getter read-only maybe-null args=1 return=Vec<u8> throws=NativeError,IOException
external-argument read NativeHandle 0 definitely-non-null read-only borrowed type=String
external-crate-api NativeHandle read 1 read_record
external-crate-constructor NativeHandle 1 NativeHandleFacade open
unsupported-api native NativeHandle rawRead requires runtime codec
unsupported-constructor native NativeHandle 2 requires native open flags
unsupported-field native NativeHandle RAW_HANDLE exposes native pointer
```

`external-constructor TYPE [ARG_TYPE...]` records a source-summary or user
constructor fact for an untranslated type. Argument types are optional and are
parsed as Java types. Use this for declaration-style constructor availability.

`external-crate-api RECEIVER METHOD ARG_COUNT RUST_NAME` records a facade
symbol override for a mapped receiver method. `ARG_COUNT` is a non-negative
integer or `*` for arity-agnostic matching.

`external-crate-constructor TYPE ARG_COUNT RUST_TYPE RUST_NAME` records a
facade constructor symbol override. Use `*`, `-`, or `none` for `RUST_TYPE` to
keep the default Rust type name. These crate mapping facts select Rust symbols;
the owning crate target and dependency mode still come from a Rust-crate-mapping
pack boundary.

`unsupported-api PACK RECEIVER METHOD REASON...`,
`unsupported-constructor PACK TYPE ARG_COUNT REASON...`, and
`unsupported-field PACK TYPE FIELD REASON...` record explicit unsupported
external boundaries from user-authored facts or source summaries. `PACK` is an
attribution label for diagnostics and provider benchmarks; these rows do not
declare pack ownership and are not substitutes for external-pack manifest
coverage rows.

`external-api METHOD RECEIVER ROLE MUTABILITY` records a user override for an
untranslated or unresolved library call. `RECEIVER` is a Java type name, or `*`
for a global method-name fact. Receiver-specific facts are preferred over global
facts for matching receiver types.

An optional fifth positional value records return nullability:
`unknown`, `definitely-non-null`, `maybe-null`, or `definitely-null`. Additional
metadata tokens may follow it:

- `args=N` records an exact argument count for overload-specific API facts.
  Omit it for arity-agnostic facts.
- `return=TYPE` records the Java return type used by type and shape inference.
- `throws=TYPE_A,TYPE_B` records exception or error types raised by the API.

The parser expects these type names as single tokens, so generic spellings must
not contain spaces.

Supported roles are `getter`, `boolean-getter`, `setter`, `mutator`,
`fluent-mutator`, `side-effect`, `indexed-getter`, `indexed-setter`, and
`unknown`.

Supported receiver mutability values are `read-only`, `mutates-receiver`, and
`unknown`.

`external-argument METHOD RECEIVER INDEX NULLABILITY MUTABILITY ALIAS_POLICY
[type=TYPE]` records a user/source-summary argument boundary fact for an
untranslated or unresolved library call. `RECEIVER` follows the same spelling
as `external-api`, including `*` for a global method-name fact. `INDEX` is
zero-based. `ALIAS_POLICY` is `unknown`, `no-alias`, `borrowed`, or `escapes`.
The optional `type=TYPE` metadata records the Java parameter type used by slot
and conversion planning.

These facts feed semantic analysis and signature planning. Downstream lowering
must consume the facts and plans rather than adding new name heuristics for
external getters, setters, or mutators.

## Provider Analysis Facts

```text
method-effect dep.AnalysisTarget summarize read-only definitely-non-null throws=dep.DecodeError,IOException
capture method dep.AnalysisTarget summarize 1 lambda capture Runnable capturedName local String true false
trivial-getter dep.AnalysisTarget getValue value policy=inline-candidate
trivial-setter dep.AnalysisTarget setValue value value fluent=true policy=preserve-api
recursion dep.AnalysisTarget summarize dep.AnalysisTarget summarize direct
control-flow dep.AnalysisTarget summarize tail-recursive-self-call
dispatch-requirement dep.AnalysisTarget needs-subtype-dispatch
catch dep.AnalysisTarget summarize 0 dep.DecodeError,IOException assignable=dep.SpecificDecodeError
```

These rows let source summaries and user-authored analysis providers inject
facts that would otherwise require scanning method bodies. They are ordinary
`FactSet` entries with user-override provenance, so dependency-mode benchmarks
can attribute them to the configured provider.

`method-effect TYPE METHOD MUTABILITY NULLABILITY [throws=TYPE_A,TYPE_B]`
records receiver mutation, return nullability, and declared thrown types for a
method. Mutability uses `read-only`, `mutates-receiver`, or `unknown`.
Nullability uses `unknown`, `definitely-non-null`, `maybe-null`, or
`definitely-null`.

`capture method TYPE METHOD OWNER_ID OWNER_KIND OWNER_NAME TARGET_TYPE
CAPTURE_NAME SOURCE CAPTURE_TYPE EFFECTIVELY_FINAL MUTABLE_AFTER_CAPTURE`
records one captured binding for a lambda, local class, anonymous class, or
inner class inside a method. `capture constructor TYPE ...` and
`capture type TYPE ...` use the same owner and captured-binding fields for
constructor and type origins. `OWNER_KIND` is `lambda`, `local-class`,
`anonymous-class`, or `inner-class`. `SOURCE` is `local`, `parameter`,
`this-field`, or `outer-this`. Use `*`, `-`, or `none` for absent owner name or
target type. Boolean values use the same accepted forms as `closed`.

`trivial-getter TYPE METHOD FIELD [policy=POLICY]` and
`trivial-setter TYPE METHOD FIELD PARAM [fluent=BOOL] [policy=POLICY]` record
source-summary accessor facts for method calls that should be planned as
backing-field reads or writes. `POLICY` is `inline-candidate`, `preserve-api`,
or `analyze-only`; getter and setter rows default to `inline-candidate`.
Setters default to `fluent=false`.

`recursion TYPE METHOD CALLS_TYPE CALLS_METHOD KIND` records a direct or mutual
recursion edge. `KIND` is `direct` or `mutual`.

`control-flow TYPE METHOD KIND` records control-flow rewrite pressure.
Supported values are `assignment-in-condition` and
`tail-recursive-self-call`.

`catch TYPE METHOD CLAUSE_INDEX TYPES [assignable=TYPE_A,TYPE_B]` records
catch-clause types and optional exception-hierarchy assignability facts. Type
lists are comma-separated and use the same single-token Java type spelling
rules as external API metadata.

These facts are intentionally not lowering instructions. Signature planning,
control-flow rewrite planning, object strategy, and exception/result planning
consume them through the same fact queries used for facts discovered from
co-translated source.

## External Pack Manifests

External pack manifests use the same line-oriented style, but they define a
coherent library boundary instead of isolated user overrides:

```text
external-pack dep.variant dep.variant rust-crate-mapping mapped target=crate::external::dep_variant
external-pack-api dep.variant dep.variant.Record getContig semantic getter read-only definitely-non-null return=String
external-pack-api dep.variant dep.variant.Builder setContig semantic fluent-mutator mutates-receiver definitely-non-null return=dep.variant.Builder
external-pack-api dep.variant dep.variant.Reader read mapped getter read-only maybe-null args=1 return=Record throws=DepIoError rust=read_record
external-pack-constructor dep.variant dep.variant.Builder 0 mapped rust-type=DepVariantBuilder rust=new_builder
external-pack-constructor dep.variant dep.variant.LegacyReader 1 unsupported
external-pack-field dep.variant dep.variant.Record TYPE semantic
external-pack-field dep.variant dep.variant.Record RAW_HANDLE unsupported
external-pack-field-nullability dep.variant dep.variant.Record TYPE definitely-non-null
external-pack-object dep.variant dep.variant.Record erased-object
external-pack-object dep.variant dep.variant.Builder dyn-trait DepVariantBuilder
external-pack-object dep.variant dep.variant.Kind closed-enum DepVariantKind SNV,INDEL
external-pack-closed dep.variant dep.variant.Kind closed
external-pack-capability dep.variant dep.variant.Record clone=true final=true
external-pack-capability dep.variant dep.variant.Collection sequence=Record final=true
external-pack-capability dep.variant dep.variant.NamedRecord implements=Record,Named
external-pack-object-safety dep.variant dep.variant.GenericFactory nested-self-return copyAll
external-pack-argument dep.variant dep.variant.Builder setContig 0 definitely-non-null read-only borrowed type=String
external-pack-exception dep.variant dep.variant.DecodeError checked
external-pack-unsupported dep.variant dep.variant.Reader rawDecode requires runtime codec state
```

`external-pack PACK TYPE_PREFIX MODE COVERAGE [target=RUST_PATH]` records
library ownership and the default coverage state for types under that prefix.
Supported modes are `external-fact-model`, `rust-crate-mapping`,
`ffi-boundary`, and `conservative-boundary`.

`external-pack-api PACK RECEIVER METHOD COVERAGE ROLE MUTABILITY [NULLABILITY]
[args=N] [return=TYPE] [throws=TYPE_A,TYPE_B] [rust=RUST_FN]` records a
pack-owned modeled API entry as an external API effect fact with
external-library-pack provenance. The coverage value must be one of
`declared`, `conservative`, `semantic`, `mapped`, or `unsupported`.
Unsupported entries become explicit unsupported facts instead of falling
through to conservative guessing. Inline `unsupported` API rows use the
generic reason `declared unsupported by external pack manifest`; use
`external-pack-unsupported` when the manifest should carry a specific reason.
The role and mutability fields are still validated for inline unsupported rows
so malformed manifests fail early, but they do not emit an API effect fact.
Inline unsupported API rows do not accept `args=`, `return=`, `throws=`, or
`rust=` metadata because no call effect or facade mapping is emitted.
For `rust-crate-mapping` packs, mapped API rows also produce facade-symbol
mapping facts. `rust=RUST_FN` overrides the default Java-to-snake-case facade
function name. The manifest audit treats these mapping facts as first-class
pack output: a crate API mapping fact must come from a `rust-crate-mapping`
manifest row with matching receiver, method, arity, and Rust symbol.

`external-pack-constructor PACK TYPE ARG_COUNT COVERAGE [rust-type=RUST_TYPE]
[rust=RUST_FN]` records a modeled constructor entry. Constructor argument types
are intentionally unknown at the manifest layer; resolution and call planning
match pack constructors by arity for external-library-pack facts. For
`rust-crate-mapping` packs, mapped constructor rows also produce constructor
facade mapping facts. `rust-type=RUST_TYPE` and `rust=RUST_FN` override the
default Rust type name and constructor function name. The manifest audit checks
constructor mapping facts against these declared Rust type/function symbols.
`unsupported`
constructor rows produce structured unsupported-constructor facts instead of
falling through to generic missing-constructor diagnostics. Unsupported
constructor rows do not accept `rust-type=` or `rust=` metadata because no
constructor facade mapping is emitted.

`external-pack-field PACK TYPE FIELD COVERAGE` records a modeled field, nested
type, enum constant, or static constant entry. Like methods and constructors,
field entries carry external-library-pack provenance so missing fields inside
owned pack types can stay diagnostic-driven rather than becoming generic
fallbacks. `unsupported` field rows produce structured unsupported-field facts.

`external-pack-field-nullability PACK TYPE FIELD NULLABILITY` records
pack-owned field nullability for signature and slot-shape planning. Supported
states are `definitely-non-null`, `maybe-null`, `definitely-null`, and
`unknown`, with the same aliases accepted by `field-nullability`. User
`field-nullability` overrides remain higher precedence than pack-owned field
nullability facts.

`external-pack-object PACK TYPE STRATEGY [ARGS...]` records a pack-owned object
representation strategy as a hierarchy override fact. Supported strategies are
`erased-object`, `concrete`, `dyn-trait TRAIT_NAME`, and
`closed-enum ENUM_NAME VARIANT_A,VARIANT_B`. These facts let external packs
state facade/object fallback policy without lowering package-name checks.

`external-pack-closed PACK TYPE [VALUE]` records pack-owned closed-hierarchy
facts for dependency types. `VALUE` defaults to `true`; accepted values are
`true`, `false`, `closed`, and `open`. Use this when a dependency pack can
assert closed-world hierarchy knowledge separately from a representation
override.

`external-pack-capability PACK TYPE KEY=VALUE...` records pack-owned type
capabilities. Supported keys are `copy`, `clone`, `string-like`, `final`, and
`object-safe` with `true` or `false` values, plus `sequence=ELEMENT_TYPE`,
`implements=TYPE_A,TYPE_B`, and `extends=TYPE`. These facts carry collection
element shapes, clone/final proofs, interface links, and object-safety
assumptions through the same effective-facts path used by source and override
facts.

`external-pack-object-safety PACK TYPE VIOLATION_KIND [ARGS...]` records
pack-owned detailed object-safety facts for external interfaces. Supported
violation forms mirror `object-safety-violation`: `static-method METHOD`,
`self-return METHOD`, `nested-self-return METHOD`, `self-param METHOD PARAM`,
`nested-self-param METHOD PARAM`, `field FIELD`, `constructor`, and
`unsupported-member`. These rows also emit `object-safe=false` capability
facts with external-library-pack provenance, so dyn-trait planning can fall
back to erased objects without inspecting external Java source or lowering
package names.

`external-pack-argument PACK RECEIVER METHOD INDEX NULLABILITY MUTABILITY
ALIAS_POLICY [type=TYPE]` records pack-owned parameter boundary facts for a
modeled method argument. Nullability uses the same values as API return
nullability. Mutability uses `read-only`, `mutates-receiver`, or `unknown` for
the argument value. Alias policy is one of `unknown`, `no-alias`, `borrowed`,
or `escapes`. These facts are intentionally separate from receiver mutation:
signature, ownership, lifetime, and coercion planning can ask one fact query
about a parameter without rediscovering method-specific library rules.

`external-pack-exception PACK TYPE KIND` records pack-owned exception or error
classification. `KIND` is `checked`, `unchecked`, or `unknown`. These facts are
used with API `throws=` metadata so exception/result planning can distinguish
declared checked errors from unchecked runtime failures without lowering
knowing the dependency package.

`external-pack-unsupported PACK RECEIVER METHOD REASON...` records a structured
unsupported API diagnostic owned by the pack. It is equivalent to an
`external-pack-api ... unsupported ...` row for coverage purposes, but keeps
the supplied reason text. Manifest coverage audits compare emitted unsupported
API, constructor, and field facts against their manifest rows, including the
structured reason for unsupported APIs and the generated reason for inline
unsupported constructors and fields.

Parsed pack manifests can be passed to the CLI with
`--external-pack-manifest path/to/pack.txt`; the option may be repeated for
both translation and `resolver-bench`. Manifests produce ordinary `FactSet`
entries. Resolver, semantic analysis, signature solving, shape/conversion
planning, and lowering consume those facts through the same paths used by
built-in packs. For benchmark baselines, `resolver-bench` also accepts
`--no-builtin-external-packs`; this disables built-in library packs so the
effect of explicit `--facts` or `--external-pack-manifest` providers can be
measured separately. Selected built-in packs can then be re-added as ordinary
benchmark fact providers with `--builtin-external-pack PACK_NAME`; useful pack
names include `htsjdk.variant.variantcontext`, `htsjdk.variant.vcf`,
`htsjdk.samtools.header`, `htsjdk.tribble.feature`,
`htsjdk.samtools.util`, `hdf5`, and `sqlite`.

Additional source roots can be benchmarked with
`--source-root co-translate:path/to/java` or
`--source-root source-summary:path/to/java`. Co-translated roots participate in
resolution and benchmarked source behavior. Source-summary roots feed
declarations and declaration facts into downstream analysis without being
benchmarked as emitted source.

Missing or unsupported calls, constructors, and fields inside a configured
pack-owned type produce pack-owned diagnostics before generic conservative
external guesses are considered. If imported source uses simple type names,
the manifest should include an owned simple-name type pattern as well as the
qualified package prefix, matching the built-in pack convention.
Resolver benchmark output keeps these boundary failures separate from generic
unexplained resolver gaps: unsupported and missing pack-owned members each have
dedicated method, constructor, and field counters plus top-summary sections.

Parsed manifests can be registered as dependency fact providers for all
external-library modes. The provider mode is inferred from declared
`external-pack` rows: `external-fact-model`, `rust-crate-mapping`,
`ffi-boundary`, and `conservative-boundary` each keep separate benchmark
attribution for explained calls and constructors. Provider facts are reported
under `fact_providers`; they are not counted again as generated per-file
builtin pack facts. A manifest provider must contain at least one
`external-pack` boundary row, and all boundary rows in that provider must use
the same external-library mode. Split mixed-mode dependencies into separate
manifest/provider inputs so reporting, mode-specific fallback behavior, and
future lowering stay aligned at the provider boundary.

Manifest rows are also audited against declared ownership boundaries. An
`external-pack-api`, `external-pack-argument`, `external-pack-constructor`,
`external-pack-field`, `external-pack-field-nullability`,
`external-pack-object`, `external-pack-closed`, `external-pack-capability`,
`external-pack-object-safety`, `external-pack-exception`, or
`external-pack-unsupported` row whose receiver/type is not owned by a
matching `external-pack` row produces a structured manifest diagnostic such as
`external_pack_manifest_unowned_api_fact`,
`external_pack_manifest_unowned_argument_effect_fact`,
`external_pack_manifest_unowned_constructor_fact`, or
`external_pack_manifest_unowned_field_fact`,
`external_pack_manifest_unowned_field_nullability_fact`,
`external_pack_manifest_unowned_object_strategy_fact`,
`external_pack_manifest_unowned_closed_hierarchy_fact`,
`external_pack_manifest_unowned_type_capability_fact`,
`external_pack_manifest_unowned_object_safety_fact`, or
`external_pack_manifest_unowned_exception_fact`. Unsupported rows are audited
with `external_pack_manifest_unowned_unsupported_api_fact`,
`external_pack_manifest_unowned_unsupported_constructor_fact`, and
`external_pack_manifest_unowned_unsupported_field_fact`. Crate facade mappings
emitted by `rust-crate-mapping` packs are audited the same way with
`external_pack_manifest_unowned_crate_api_mapping_fact` and
`external_pack_manifest_unowned_crate_constructor_mapping_fact`. Add an
explicit package/type boundary row for every qualified or parser-collapsed
simple-name surface that the manifest models; do not use a one-off member row
as an implicit boundary.

Manifest coverage audits also check hierarchy rows beyond representation
strategy. A pack-owned closed-hierarchy fact without a corresponding
`external-pack-closed` row reports
`external_pack_manifest_missing_closed_hierarchy_coverage`; a stale open/closed
value reports `external_pack_manifest_closed_hierarchy_mismatch`. A pack-owned
object-safety fact without a corresponding `external-pack-object-safety` row
reports `external_pack_manifest_missing_object_safety_coverage`; a stale or
different violation set reports
`external_pack_manifest_object_safety_mismatch`.

## Field Nullability

```text
field-nullability User name definitely-non-null
field-nullability Cache.value maybe-null
field-nullability Node.parent unknown
```

`field-nullability TYPE FIELD STATE` and `field-nullability TYPE.FIELD STATE`
record user overrides for Java field storage nullability. Supported states are
`definitely-non-null`, `maybe-null`, `definitely-null`, and `unknown`.
Aliases `non-null`, `nullable`, and `null` are accepted.

External field-nullability facts override constructor and method proofs. This
allows corpus-level or user-provided knowledge to force Rust storage to direct
`T`, `Option<T>`, or an unknown conservative shape through the same fact query
used by signature planning.
