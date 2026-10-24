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

External argument nullability is positive evidence, not a missing-data default.
If no external argument fact, source annotation, provider summary, or call-site
evidence says a reference parameter accepts `null`, slot planning should treat
the external boundary as definitely non-null. Use `maybe-null` or
`definitely-null` only when the library contract or observed source behavior
actually proves the optional shape.

This also applies to callback parameters and constructor arguments. For an
unknown external callback slot, assume the callback itself and its parameters
are required values unless nullable evidence exists. For example, a constructor
parameter that is only known to be a callback taking
`(Map<String, URI>, int, int)` and returning a reader map is modeled as
non-null by default; a future injected fact can widen that exact slot to
nullable without changing lowering rules.

This is a general boundary rule, not a library shim rule. The only place that
may turn unknown external parameter nullability into the default non-null shape
is the semantic fact or slot-planning policy that consumes `FactSet`. Lowering,
call rendering, external crate mappings, and per-library manifests must consume
the normalized parameter slot shape. They must not add their own method-name,
type-name, crate-name, or corpus-specific nullable guesses. If a future corpus
shows a real nullable parameter, add or inject a fact for that exact callable
slot and let the same rule widen it.

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
external-pack dep.variant dep.variant rust-crate-mapping mapped target=crate::external::dep_variant package=dep-variant path=../vendor/dep-variant
external-pack dep.variant dep.variant.NativeCursor rust-crate-mapping mapped target=crate::external::dep_variant rust-type=CursorHandle
external-pack-api dep.variant dep.variant.Record getContig semantic getter read-only definitely-non-null return=String
external-pack-api dep.variant dep.variant.Builder setContig semantic fluent-mutator mutates-receiver definitely-non-null return=dep.variant.Builder
external-pack-api dep.variant dep.variant.Reader read mapped getter read-only maybe-null args=1 return=Record throws=DepIoError rust=read_record
external-pack-constructor dep.variant dep.variant.Builder 0 mapped rust-type=DepVariantBuilder rust=new_builder
external-pack-constructor-argument dep.variant dep.variant.Builder 2 1 definitely-non-null read-only borrowed type=dep.variant.Options
external-pack-conversion dep.variant dep.variant.Leaf dep.variant.Root into
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

`external-pack PACK TYPE_PREFIX MODE COVERAGE [target=RUST_PATH]
[rust-type=RUST_TYPE] [reference-identity=RUST_METHOD] [package=CARGO_PACKAGE] [path=DEPENDENCY_PATH]
[string-abi=utf8|utf16]` records
library ownership and the default coverage state for types under that prefix.
Supported modes are `external-fact-model`, `rust-crate-mapping`,
`ffi-boundary`, and `conservative-boundary`. `target=` is semantic input: it
names the Rust module or crate path used by lowering. `rust-type=` records the
Rust facade type name for this owned Java type or prefix and is the preferred
place to model stable Java-type to Rust-type name differences such as acronym
or facade handle spelling. Type-level `rust-type=` facts are consumed by all
crate-mapped type rendering, including method returns, fields, locals,
constructor results, and facade support. Constructor-level `rust-type=`
metadata remains available for constructor-specific facade mappings, but it is
not the canonical place to name a type. `package=` and `path=` are
dependency-assembly metadata consumed by generated-crate tooling; semantic
analysis accepts them but does not use them for type, ownership, nullability,
or lowering decisions.

For an immutable mapped reference, `reference-identity=RUST_METHOD` requires
an explicit `rust-type=` and an inherent `fn(&self, &Self) -> bool` method.
The provider must implement Clone by retaining the same Java allocation;
fresh constructor results retain distinct identities even when their values
are equal. The comparison method is pure and infallible. This is separate
from Rust PartialEq and Java equals, which still compare numeric/value
semantics. Binary Java reference comparisons retain both carrier snapshots
in Java evaluation order, including nullable borrowed getter results, before
calling this method. A user override of the type mapping replaces its identity
contract too; an omitted method does not inherit stale pack metadata. The
java-math provider reuses the shared immutable-reference allocation kernel
also used by Java String carriers.

For Rust-crate mappings, `string-abi=utf16` opts Java String values into a
unit-preserving native text ABI: constructor/receiver arguments use owned
`Vec<u16>`, optional parameters use `Option<Vec<u16>>`, and String results use
the same carriers inside their existing nullable/Result wrappers. Native
getMessage adapters declared for exceptions must return `Option<Vec<u16>>`
too. Java argument evaluation precedes native null checks and encoding.
Omitted metadata retains the existing `utf8` ABI. This is a text-value bridge,
not an alias-preserving foreign Java String reference protocol; native APIs
that return or retain reference identity need an explicit identity contract.
Custom `rust-return=` declarations retain their authority. This option uses
the existing mapped constructor/receiver paths; it does not add support for
otherwise-unmodeled APIs or change source-owned signatures.

`external-pack-api PACK RECEIVER METHOD COVERAGE ROLE MUTABILITY [NULLABILITY]
[args=N] [return=TYPE] [throws=TYPE_A,TYPE_B] [rust=RUST_FN]
[rust-result=true|false] [rust-return=RUST_TYPE]` records a pack-owned modeled
API entry as an external API effect fact with
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
function name. `rust-result=true|false` records whether the Rust facade returns
`Result` independently of the Java `throws=` model. `rust-return=RUST_TYPE`
records the actual Rust facade return type when it differs from the Java
semantic `return=TYPE`; slot conversion then owns any cast or adapter, such as
Rust `usize` returned by a facade for a Java `int` method. Borrowed ABI forms
such as `rust-return=&Schema` and `rust-return=Option<&FieldVector>` mean the
facade returns a borrow while Java semantics still expose a value; conversion
planning must recover owned values before erasing or storing them. The manifest
audit treats these mapping facts as first-class pack output: a crate API
mapping fact must come from a `rust-crate-mapping` manifest row with matching
receiver, method, arity, and Rust symbol.

External method and constructor parameter nullability is non-null by default.
`unknown` parameter nullability, missing parameter metadata, or an unmodeled
neighboring parameter means the Rust-facing slot is required unless positive
nullable evidence exists. Positive evidence includes an explicit
`external-pack-argument` or user fact, observed `null` at a resolved call site,
nullable source-summary/provider facts, or declared library annotations. This
default applies only to parameters; return and field nullability continue to
come from their own facts. Future user-injected facts should override this
default in the fact layer, then flow through slot shape and conversion planning
without adding nullable special cases in lowering.

`external-pack-constructor PACK TYPE ARG_COUNT COVERAGE [rust-type=RUST_TYPE]
[rust=RUST_FN]` records a modeled constructor entry. Constructor argument types
default to unknown at the manifest layer; resolution and call planning match
pack constructors by arity for external-library-pack facts. For
`rust-crate-mapping` packs, mapped constructor rows also produce constructor
facade mapping facts. `rust-type=RUST_TYPE` and `rust=RUST_FN` override the
default Rust type name and constructor function name. The manifest audit checks
constructor mapping facts against these declared Rust type/function symbols.
`unsupported`
constructor rows produce structured unsupported-constructor facts instead of
falling through to generic missing-constructor diagnostics. Unsupported
constructor rows do not accept `rust-type=` or `rust=` metadata because no
constructor facade mapping is emitted.

`external-pack-constructor-argument PACK TYPE ARG_COUNT INDEX NULLABILITY
MUTABILITY ALIAS_POLICY [type=TYPE]` records a constructor parameter boundary
fact. It uses the same nullability, mutability, aliasing, and `type=TYPE`
metadata meanings as `external-pack-argument`, but is keyed by constructor
owner type and Java arity instead of method name. Use it for crate/facade
constructors whose Rust parameter shape differs from the Java value shape, such
as borrowed allocator handles, borrowed path strings, borrowed slices, optional
callbacks, and owned escaping resources.

External method and constructor parameters default to non-null. A missing
argument fact, or an argument fact whose nullability is `unknown`, must not
introduce `Option<T>` by itself. Use `maybe-null` or `definitely-null` only
when a manifest, source summary, observed `null` call site, or future injected
override proves that the boundary accepts null.

`external-pack-conversion PACK FROM_TYPE TO_TYPE into` records a Rust facade
adapter for a Java reference conversion where both sides are external-pack
types. The current adapter kind, `into`, means Rust has a standard
`From<FROM> for TO` or equivalent `Into<TO>` implementation, so coercion may
render `expr.into()` when an expression of `FROM_TYPE` is passed to a slot of
`TO_TYPE`. This is intended for facade subtype wrappers such as concrete vector
handles into a facade enum. Lowering must not special-case the source or target
library name.

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
ALIAS_POLICY [type=TYPE] [args=COUNT] [null-message=RUST_FUNCTION]` records pack-owned parameter boundary facts for a
modeled method argument. Nullability uses the same values as API return
nullability. Mutability uses `read-only`, `mutates-receiver`, or `unknown` for
the argument value. Alias policy is one of `unknown`, `no-alias`, `borrowed`,
or `escapes`. These facts are intentionally separate from receiver mutation:
signature, ownership, lifetime, and coercion planning can ask one fact query
about a parameter without rediscovering method-specific library rules.
Optional `args=COUNT` restricts a fact to that method arity. It uses the existing
arity-aware parameter index: e.g. index 1 may be an enum for a two-argument
overload and an integer for a three-argument overload. Without `args=`, the
fact remains a fallback across arities. An exact-arity fact takes precedence;
invalid arities, duplicate `args=` and indices outside the stated arity are
rejected. Same-arity overloads still need compatible parameter boundary facts;
this option is not a new type-based overload dispatch mechanism.

Optional `null-message=crate::function` supplies a zero-argument runtime function
returning the exception message when a required ABI argument is null. It requires
`definitely-non-null`; duplicate or empty function paths are rejected before
publishing the argument contract. The function is called only on null failure.
The diagnostic belongs to the selected argument-effect fact, including its owner,
arity, parameter index and override precedence; replacing a contract without this
option removes its diagnostic. Lowering replaces the required value's null check,
preserving exceptions raised while evaluating the argument itself. For example,
the java-math pack uses this for BigDecimal's dereference of a null rounding mode.

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
value reports `external_pack_manifest_closed_hierarchy_mismatch`. External-pack
object-safety rows are parsed directly into pack-owned `ObjectSafetyFact` values;
boundary audits still report object-safety facts whose type is not owned by any
declared pack.

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
