# Current GATK Idiom Audit

Date: 2026-07-13

Corpus sweep:
`/tmp/new_java2rust_gatk_sweep_current`

Parallel audit helper:
`/tmp/new_java2rust_idiom_audit.py`

The audit scanned all current generated Rust files with eight workers and
counted regex-defined idiom signals. These are not correctness proofs; they are
triage signals for fixture selection.

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
