# Current GATK Idiom Audit

Date: 2026-07-19

Corpus sweep:
`/tmp/new_java2rust_gatk_sweep_current`

Tracked audit helper:
`scripts/sweep-generated-idioms.py`

The audit scans generated Rust files and counts regex-defined idiom signals.
These are not correctness proofs; they are triage signals for fixture
selection. The helper is wired into `scripts/translate-focused-package.sh`, so
focused package gates now leave `idiom-sweep.md` and `idiom-sweep.tsv` beside
`compile.log`, `invalid-patterns.tsv`, and `top-compile-errors.tsv`.

Re-run directly on an existing generated crate:

```sh
python3 scripts/sweep-generated-idioms.py \
  --generated-root /tmp/new_java2rust_gatk_sweep_current/generated-crate/src/generated \
  --status /tmp/new_java2rust_gatk_sweep_current/status.tsv \
  --out-md /tmp/new_java2rust_gatk_sweep_current/idiom-sweep.md \
  --out-tsv /tmp/new_java2rust_gatk_sweep_current/idiom-sweep.tsv
```

## Top Signals

| Occurrences | Files | Signal | Example source |
|---:|---:|---|---|
| 15163 | 994 | JavaBean-style `get_`/`set_` calls | `htsjdk/samtools/SAMRecordSparkCodec.java` |
| 7790 | 1050 | `downcast::<T>().expect("invalid Java cast")` | `htsjdk/samtools/SAMRecordSparkCodec.java` |
| 6851 | 635 | acronym splitting | `org/broadinstitute/hellbender/Main.java` |
| 4266 | 777 | unsupported dispatch coercion compile errors | `htsjdk/samtools/SAMRecordSparkCodec.java` |
| 2086 | 490 | lambda compile errors | `org/broadinstitute/hellbender/Main.java` |
| 1854 | 75 | switch state machine lowering | `org/broadinstitute/hellbender/engine/AlignmentContext.java` |
| 1773 | 427 | Java collection `.size()` spelling | `org/broadinstitute/hellbender/Main.java` |
| 859 | 299 | Java `.equals(...)` spelling | `org/broadinstitute/hellbender/Main.java` |
| 854 | 298 | method-reference compile errors | `GATKAnnotationPluginDescriptor.java` |
| 700 | 244 | `string.format(...)` spelling | `org/broadinstitute/hellbender/Main.java` |
| 582 | 283 | try/catch/finally lowering placeholder | `htsjdk/samtools/SAMRecordSparkCodec.java` |
| 510 | 188 | Java array creation placeholder | `htsjdk/samtools/SAMRecordSparkCodec.java` |
| 365 | 200 | class literal compile errors | `org/broadinstitute/hellbender/Main.java` |
| 358 | 161 | Java collection `.add_all(...)` spelling | `org/broadinstitute/hellbender/Main.java` |
| 285 | 218 | `super(...)` constructor placeholder | `FeatureInput.java` |
| 186 | 89 | unsupported `instanceof` shape | `ModeArgumentUtils.java` |
| 29 | 22 | unresolved constructor delegation | `FeatureContext.java` |
| 11 | 8 | `this(...)` followed by additional statements | `PSPathogenReferenceTaxonProperties.java` |

## Fixture Additions From This Audit

Added under `examples/idiom-gaps` and asserted in `tests/idiom_gaps.rs`:

- `PrimitiveGetterDispatchCoercion.java`
- `JavaBeanGetterSetterNames.java`
- `ConstructorDelegationWithExtraWork.java`
- `UnresolvedConstructorDelegation.java`
- `BooleanGetterDispatchCoercion.java`
- `ByteArrayGetterDispatchCoercion.java`
- `CollectionRemoveIfLambda.java`
- `ForEachLambdaSideEffect.java`
- `MethodReferenceFlatMap.java`
- `ClassLiteralRegistration.java`
- `StringArrayLiteralArgument.java`
- `MapPutGetJavaApi.java`
- `EnumNameAndValueOf.java`
- `OptionalGetIsPresent.java`

These complement the earlier GATK-derived idiom fixtures for lambdas, method
references, class literals, arrays, downcasts, strings, collection APIs, switch
lowering, `super(...)` state, and try-with-resources.

`tests/idiom_gaps.rs` now contains 30 current-output regression tests.

## Triage Ownership Map

The tracked sweep keeps the audit tied to generalized fixtures rather than
single corpus files:

| Signal | Owning layer | Generalized fixture coverage |
|---|---|---|
| Runtime `downcast::<T>().expect("invalid Java cast")` | `COERCE-1` / representation dispatch | `TypedGetterAvoidsDowncast.java`, `PrimitiveGetterDispatchCoercion.java` |
| `None` likely flowing into concrete storage | `NULL-2` / slot optionality | `NullableParameterAndLocal.java`, `NullableLocalReturn.java` |
| Java collection method spelling | `CALL-1` / native collection call planning | `CollectionAddAllAndSize.java`, `CollectionsUse.java`, `MapPutGetJavaApi.java` |
| Stream/lambda/method-reference placeholders | `STREAM-2` / stream planning | `MethodReferenceFlatMap.java`, `StreamFlatMapLambda.java`, `ForEachLambdaSideEffect.java` |
| Try/catch/finally placeholders | `EXC-2` / `EXC-3` exception planning | `SimpleTryCatchReturn.java`, `SimpleTryFinallyCleanup.java`, `TryWithResourcesCopy.java` |
| Java array creation placeholders | `ARRAY-1` / array shape planning | `PrimitiveArrayWithComputedLength.java`, `StringArrayLiteralArgument.java` |
| Synthetic switch state machines | `CTRL-2` / switch planning | `SimpleReturnSwitches.java`, `SwitchReturnsEnumMatch.java`, `SwitchFallthrough.java` |
| `super(...)` constructor placeholders | `INIT-1` / object representation and inherited initialization | `SuperConstructorState.java`, `InheritanceAndSuper.java` |
| Unsupported constructor delegation | `INIT-1` / constructor initialization planning | `ConstructorDelegationWithExtraWork.java`, `UnresolvedConstructorDelegation.java` |
| Acronym-split identifiers | identifier hygiene | `AcronymIdentifierNames.java` |
| Class literal placeholders | `CALL-1` / class literal token planning | `ClassLiteralToken.java`, `ClassLiteralRegistration.java` |
| Unsupported `instanceof` / cast shape | representation dispatch and narrowing | `InstanceofGuardNarrowsCast.java` |

Use the counts to pick the next generalized fixture or implementation slice.
Do not treat a lower count as proof that the owning layer is correct; correctness
still comes from focused fixtures, generated-crate gates, and corpus compile
evidence.
