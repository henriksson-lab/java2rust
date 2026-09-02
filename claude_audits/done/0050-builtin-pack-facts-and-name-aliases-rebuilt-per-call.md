# 0050 — The built-in pack fact set and the method-name alias list are rebuilt on every call from the lowering hot path

Status: **fixed** (profiled, call-counted with a breakpoint, list-diffed)

Codex follow-up 2026-08-21: substantially fixed. `builtin_external_library_packs()`
now folds `builtin_external_pack_fact_sets()` through a `OnceLock<FactSet>` and
returns a clone of that cached aggregate, preserving the old API while removing
the per-call 62-pack rebuild. `facts::external_method_name_aliases` now has the
same thread-local memo shape as `external_type_name_aliases`. Guard coverage was
added in `rendered_string_heuristics_have_centralization_guardrails`, and
`builtin_external_member_lookup_uses_cached_pack_facts` still passes.
`--no-builtin-external-packs` is now wired through translate/batch semantic
analysis and through the direct support-generation/member-lookup pack fallbacks;
`disabling_builtin_external_packs_suppresses_builtin_support_surfaces` guards the
previous no-op. Still open: the return type is not yet a shared `&'static
FactSet`/handle, and the alias memos still clone `Vec<String>` on hits.

Codex follow-up 2026-08-22: the alias memos now return shared `Rc<[String]>`
handles on hits, call-plan arity naming uses indexed built-in facts,
inherited-member lookup borrows built-in facts directly, member-access and
constructor-support helpers avoid the aggregate clone, and semantic analysis
now borrows cached pack fact sets when collecting representation/exception
facts. Remaining aggregate clones are the three larger support-generation
effective-facts merges in `stdlib`.

Codex follow-up 2026-08-22, field-surface support slice: external field support
generation no longer clones `builtin_external_library_packs()`. It now builds a
smaller support fact set from the caller's facts plus the borrowed cached built-in
pack fact sets, copying only the representation/object-safety/type-capability,
library-pack and crate type-mapping categories needed by field return rendering
and trait-impl surface discovery. Remaining aggregate clones are the two larger
`stdlib` support-generation merges for static API surfaces and dyn-trait
surfaces.

Codex follow-up 2026-08-22, API-argument iterator cleanup: the exported
`builtin_external_api_argument_effect_facts()` helper no longer obtains argument
facts by cloning the aggregate `builtin_external_library_packs()` result. It now
borrows the cached pack fact sets directly and yields references; semantic
collection clones only the individual facts it actually inserts. A guard now
checks the helper itself, not just the semantic collector. Remaining aggregate
clones are still the two larger `stdlib` support-generation merges for static
API surfaces and dyn-trait surfaces.

Codex follow-up 2026-08-22, API-surface support slice: the two remaining
`stdlib` support-generation merges now share
`ExternalFacadeSupport::external_api_surface_support_facts`, which starts from
the caller's facts and extends only the support categories needed by static API
surface and dyn-trait generation from `builtin_external_pack_fact_sets_iter`.
`add_static_external_api_facts_for_surface_types` and
`add_external_dyn_trait_surfaces` no longer call the aggregate
`builtin_external_library_packs()` helper. The remaining larger design cleanup is
returning a shared fact-set handle/reference for callers that still need a whole
aggregate, rather than cloning a cached `FactSet`.

Codex follow-up 2026-08-22, borrowed aggregate closure: the shared aggregate now
has a borrowed accessor, `builtin_external_library_pack_facts() -> &'static
FactSet`. The old `builtin_external_library_packs()` remains only as an owned
compatibility wrapper that clones the borrowed aggregate. The remaining in-repo
aggregate users were tests and now call the borrowed accessor; source guardrails
assert that production code uses the borrowed aggregate or narrower built-in fact
iterators instead of the clone-returning wrapper. The 0050 performance/design
scope is closed.

Severity: **high for performance** (57% of runtime on the measured input),
**low-medium for correctness** (a gate that six sites bypass; currently
unobservable through the CLI because the CLI flag is not wired to the gate)
Kind: one fact computed in many places — the direct sequel to
[0040](done/0040-external-type-name-matching-recomputes-on-the-hot-path.md),
which fixed one of *two* identical siblings and left the other alone.

Measured on `6bcaa7a`, `target/release/new-java2rust`,
Intel Xeon Gold 6138 @ 2.00 GHz.

## Symptom

A **122-line** Java file (`Ext.java`: 40 one-line methods, each making three
method calls on one `htsjdk` type) takes **15.0 s** to translate. The same file
with the external type replaced by a local nested class (`Loc.java`, identical
statement shapes) takes **4.5 s**.

A **4-line** file with one external call takes **12.3 s**.

`perf record -g --call-graph=dwarf` on `Ext.java`, cumulative ("children"):

| symbol | children | self |
|---|---|---|
| `facts::external_method_name_matches` | **51.10%** | 1.66% |
| `java_naming::java_method_name_aliases` | **44.84%** | 7.60% |
| `parser::to_snake_case` | 19.42% | 7.21% |
| `external_packs::builtin_external_library_packs` | **11.69%** | 0.04% |
| `facts::external_type_name_matches` | 9.28% | 0.51% |

On a different input with no external library types at all (`Big.java`,
722 lines, 96 methods over `java.util` only, 13.9 s),
`facts::external_type_name_matches` is 45.62% and
`builtin_external_library_packs` is still reached 1449 times.

Two independent recomputations account for essentially all of it.

## Finding A — `builtin_external_library_packs()` rebuilds 62 pack fact sets on every call

`src/external_packs.rs:29013 external_packs::builtin_external_library_packs`

```rust
pub fn builtin_external_library_packs() -> FactSet {
    let mut facts = FactSet::default();
    facts.extend_from(java_platform_field_fact_set());
    facts.extend_from(htsjdk_variantcontext_pack_facts());
    …                                     // 62 lines total
    facts
}
```

It takes **no arguments**, reads **no** program, facts, or configuration — it is
a constant. It allocates and merges 62 `FactSet`s each time it is called.

### Call count, measured

`gdb` breakpoint on the symbol, `ignore 1 100000000`, then `info breakpoints`:

| input | lines | calls to `builtin_external_library_packs` | wall |
|---|---|---|---|
| `Filt.java` | 8 | 19 | 1.5 s |
| `Ext2.java` | 4 (1 external call) | 32 | 12.3 s |
| `Big.java` | 722 (no external libs) | 1449 | 13.9 s |
| `Ext.java` | 122 (120 external calls) | **1544** | 15.0 s |

`Ext2` → `Ext` adds 1512 rebuilds and 2.7 s of wall time: **~1.8 ms per call**,
matching the 11.69% profile share (1.75 s of 15.0 s).

### The callers are lowering-inner-loop code, not setup

- `src/member_access_plan.rs:307 fact_set_for_builtin_pack_lookup` — called from
  the static-constant owner resolution at `member_access_plan.rs:245`.
- `src/call_plan.rs:4181 external_method_needs_arity_name` — decides whether an
  emitted external method name gets an arity suffix; runs per call site.
- `src/call_plan.rs:4251 external_constructor_needs_arity_name` — same, per
  `new`.
- `src/inherited_members.rs:197 external_inherited_instance_fields_for_type` —
  `let mut effective_facts = facts.clone();` (the *whole* `FactSet`) then
  `effective_facts.extend_from(builtin_external_library_packs());`, per type.
  Reached from `item_assembly.rs:1015`, `lower.rs:1019`, `signature.rs:6081`.
- `src/inherited_members.rs:244`, `:280` — same shape for static fields and
  methods.

### The memoized version already exists, 280 lines away

`src/external_packs.rs:29294 builtin_external_pack_fact_sets`:

```rust
fn builtin_external_pack_fact_sets() -> &'static [(String, FactSet)] {
    static FACT_SETS: OnceLock<Vec<(String, FactSet)>> = OnceLock::new();
    FACT_SETS.get_or_init(|| vec![
        ("java-platform-fields".to_string(), java_platform_field_fact_set()),
        (VARIANTCONTEXT_PACK.to_string(), htsjdk_variantcontext_pack_facts()),
        …
    ])
}
```

I extracted both enumerations mechanically and diffed them: **62 entries each,
same functions, same order, zero difference.** They are two hand-maintained
transcriptions of one list. Today they agree; nothing makes them agree. Adding
pack number 63 to one and not the other produces a translator where
`external_field_for(…)` (index path) knows a type that
`external_inherited_instance_fields_for_type` (rebuild path) does not — a
divergence with no test that could see it, since neither list is asserted
against the other.

### Correctness corollary: six sites bypass the enable gate

`semantic_analysis.rs` treats "are the built-in packs on?" as a decision:

- `src/semantic_analysis.rs:155 collect_builtin_external_representation_facts(facts, enabled)` — `if !enabled { return; }`
- `src/semantic_analysis.rs:403 collect_builtin_external_exception_facts(facts, enabled)` — `if !enabled { return; }`

The six call sites listed above call `builtin_external_library_packs()`
unconditionally. They do not take the flag and cannot see it. So in the one
pipeline that *does* set `SemanticAnalysisConfig::enable_builtin_external_packs
= false` (`src/resolver_benchmark.rs:1997`), the packs still arrive through
`inherited_members` and `call_plan`.

Separately — and this is why the bypass is not observable from the CLI today —
there are **two** fields named `enable_builtin_external_packs` and they are
never connected:

- `dependency_mode.rs:170 DependencyModeConfig::enable_builtin_external_packs`,
  set from `--no-builtin-external-packs` at `src/main.rs:41`, consumed **only**
  in `resolver_benchmark.rs`.
- `semantic_analysis.rs:63 SemanticAnalysisConfig::enable_builtin_external_packs`,
  which is what the gates above read. The whole `lib.rs` translate pipeline
  reaches semantic analysis through `semantic_analysis::analyze_program`
  (`src/semantic_analysis.rs:68`), which hardcodes
  `SemanticAnalysisConfig::default()` — always `true`.

Verified end to end: a class extending `java.io.FilterInputStream` gets the
modelled `in` field either way.

```java
public class Filt extends FilterInputStream {
    private int counter;
    public Filt(InputStream src) { super(src); }
    public int bump() { counter = counter + 1; return counter; }
}
```

```
$ ./target/release/new-java2rust Filt.java > with.rs
$ ./target/release/new-java2rust --no-builtin-external-packs Filt.java > without.rs
$ diff with.rs without.rs        # no output — byte identical
```

At the time of the audit, both emitted

```rust
pub struct Filt {
    counter: i32,
    r#in: Option<JavaIoInputStream>,   // from java_platform_field_fact_set()
}
```

The `r#in` field comes from `inherited_members.rs:197`, which merges the packs
regardless.

Codex follow-up 2026-08-21: fixed for the translate path. The CLI-loaded
`FactSet` now carries `disable_builtin_external_packs`, `lib.rs` turns that into
`SemanticAnalysisConfig::enable_builtin_external_packs = false`, and direct
pack fallbacks in inherited-member lookup, call planning, member access, and
stdlib support generation honor the same bit. The regression uses a
`SAMRecord.getReadName()` probe because the old behavior emitted identical
builtin SAMRecord support with and without the flag.

Codex follow-up 2026-08-22: builtin unsupported-API lookup now indexes and
queries method-name aliases, matching the other builtin member indexes.
`builtin_external_member_lookup_uses_cached_pack_facts` now asserts that
`VariantContext.fully_decode` resolves the existing `fullyDecode` unsupported
fact. The performance portion remains open: hot-path callers still clone the
cached aggregate `FactSet`, and call-plan arity scans still have their own
method-name matching path.

Codex follow-up 2026-08-22, second pass: call-plan arity naming no longer
clones `builtin_external_library_packs()`. The existing built-in lookup index
now carries `api_effect_arities` and `constructor_arities`; `call_plan` uses
`external_api_arities_for_receiver` and `external_constructor_arities_for` for
built-in facts while keeping user `FactSet` scans unchanged. Behavior coverage
checks `ParsingUtils.split`, snake-case API aliases, and `VariantContextBuilder`
constructor arities; source guards assert the arity-name helpers do not call the
aggregate clone path. Remaining 0050 work is the other aggregate-clone callers
and the clone-on-hit alias memo shape.

Codex follow-up 2026-08-22, third pass: inherited-member lookup no longer
clones the cached built-in aggregate. `external_packs` exposes borrowed
iterators for built-in field/API facts plus a borrowed built-in field
nullability lookup; `inherited_members` now walks user facts and borrowed
built-ins in separate passes when the pack gate is enabled. The centralization
guard now asserts `inherited_members.rs` does not call
`builtin_external_library_packs()`.
Remaining aggregate clone sites are support-generation/member-access paths and
the alias memos still clone on hits.

Codex follow-up 2026-08-22, fourth pass: alias memo hits no longer clone the
whole alias vector. `facts::external_method_name_aliases` and
`facts::external_type_name_aliases` now cache `Rc<[String]>`, preserving the
existing iterator callers while making cache hits a handle clone. Remaining
0050 work is the support-generation/member-access aggregate clone sites.

Codex follow-up 2026-08-22, fifth pass: member-access static constant owner
resolution no longer clones the built-in aggregate. It now asks the existing
indexed `external_packs::external_library_pack_for_type` path for built-in
Rust-crate pack ownership, while preserving the `disable_builtin_external_packs`
gate. The source guard in `type_coercion` asserts `member_access_plan.rs` does
not reintroduce `builtin_external_library_packs()`. Remaining 0050 work is the
support-generation aggregate clone sites in `stdlib`.

Codex follow-up 2026-08-22, sixth pass: constructor support helpers no longer
clone the aggregate just to count constructor arities or infer a unique
constructor argument type. `external_packs` now exposes borrowed built-in
constructor facts; `stdlib` uses that iterator in
`external_constructor_fact_arity_count` and
`unique_external_constructor_fact_arg_types`, and `add_external_field_surfaces`
uses the existing borrowed built-in field iterator for its filtered built-in
field pass. Remaining aggregate clones are the three larger effective-facts
merges in static API, field surface, and dyn-trait surface generation.

## Finding B — `java_method_name_aliases` is 0040's untouched twin

`src/facts.rs:3436 facts::external_method_name_matches`:

```rust
fn external_method_name_matches(fact_method_name: &str, query_method_name: &str) -> bool {
    let fact_aliases = external_method_name_aliases(fact_method_name);
    external_method_name_aliases(query_method_name)
        .iter()
        .any(|query_alias| fact_aliases.iter().any(|fact_alias| fact_alias == query_alias))
}
```

`src/facts.rs:657 external_method_name_aliases` →
`src/java_naming.rs:9 java_method_name_aliases`, which allocates a
`Vec<String>`, calls `to_snake_case`, maybe `snake_method_name_to_java_spelling`,
then the caller sorts and dedups. Both names, once per fact, inside a linear
scan of the fact vectors.

This is *character for character* the shape 0040 diagnosed and fixed for
`external_type_name_aliases` — whose memo now sits at `src/facts.rs:3474`, **17
lines below** `external_method_name_matches`, carrying a doc comment that
explains exactly why:

> Memoized because it is a **pure function of its argument** that sits on the
> hottest path in the translator […] it recomputed the alias list […] for
> *both* names on every comparison.

`java_method_name_aliases` satisfies the same premise word for word: no facts,
no program, no configuration. It was not memoized. On `Ext.java` it is 44.84%
of runtime, four times what the already-fixed sibling costs.

The `to_snake_case` self-time (7.21%) is the same leader 0040 profiled — the fix
moved it off the type-name path and left it on the method-name path.

Note also that 0040's memo *clones* its `Vec<String>` on every hit
(`std::thread::local::LocalKey<T>::with` → `Vec::clone` → `String::clone` shows
under `external_type_name_aliases` at 7.56% on `Ext.java`, and under
`external_type_name_matches` at 45.62% on `Big.java`). Returning
`Rc<[String]>`, or an interned id, would remove the remaining cost; a memo that
hands back a fresh allocation is only half a memo.

## Why it went unnoticed

- Both facts are *constants of the binary*. Nothing in the type system, and no
  test, distinguishes "computed once" from "computed 1544 times" — the answers
  are identical, so only a clock or a profiler can see it, and the test suite
  measures compile errors in the generated crate
  ([0018](0018-the-compile-error-metric-cannot-see-silent-failures.md)).
- 0040 was found by asking why a *specific* three-line input was slow, and the
  profile at that moment was dominated by the type-name path. Fixing it made the
  method-name path the top of the profile, but nobody re-profiled.
- The two pack enumerations are 280 lines apart in a **32 415-line** file. Nobody
  reading either one can see the other.
- The gate bypass is invisible because the CLI flag that would expose it never
  reaches the gate. `--no-builtin-external-packs` "works" in the sense that
  `parse_args` accepts it and `main.rs:41` stores it; it simply has no consumer
  on the translate path, so a user testing the bypass sees identical output and
  concludes the flag is honoured.

## Direction that removes the possibility

1. **Delete `builtin_external_library_packs`'s body and make it a view.**
   Add one `OnceLock<FactSet>` that folds `builtin_external_pack_fact_sets()`
   in order, and return `&'static FactSet`. Both enumerations collapse into
   one; the 62-line list stops being duplicated; the 1544 rebuilds become one.
   The signature change (`FactSet` → `&'static FactSet`) is what makes the
   duplication *impossible to reintroduce*: a caller can no longer mutate the
   returned set, which is what `inherited_members.rs:197` currently does via
   `facts.clone().extend_from(…)`.
2. **Memoize `java_method_name_aliases`** the same way
   `external_type_name_aliases` already is, and while there, return a shared
   handle rather than a cloned `Vec` from both memos.
3. **Fold the two `enable_builtin_external_packs` fields into one**, thread it
   into the six bypassing sites (or, better, make the six sites read the packs
   out of the `&FactSet` they are already given, so there is nothing left to
   gate). Until then, at minimum, `analyze_program` should take the flag rather
   than hardcoding `SemanticAnalysisConfig::default()`.

Step 1 alone is a mechanical change with no semantic risk: the folded set is
byte-identical to what `builtin_external_library_packs()` builds today (same
functions, same order, same `extend_from`), verified by diffing the two lists.

## Reproduction

```
# call count
cat > /tmp/cnt.gdb <<'EOF'
set pagination off
break new_java2rust::external_packs::builtin_external_library_packs
ignore 1 100000000
run
info breakpoints
quit
EOF
gdb -q -batch -x /tmp/cnt.gdb --args ./target/release/new-java2rust Ext.java | grep "already hit"

# profile
perf record -g --call-graph=dwarf -F 199 ./target/release/new-java2rust Ext.java
perf report --children --percent-limit 2 --stdio -g none
```

`Ext.java` is 40 copies of
`public String mN(VariantContext vc) { return vc.getContig() + vc.getStart() + vc.getEnd(); }`
with `import htsjdk.variant.variantcontext.VariantContext;`.

Follow-up 2026-08-30: the shared external type alias cache now also carries the
nested-type suffix aliases that were previously only used by exception lookup.
This closed the Avro `Schema.Parser` semantic-pack lookup without adding a
second constructor-owner path, and keeps the new alias spellings on the same
memoized path as package/simple/canonical spellings.
