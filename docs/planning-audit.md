# Planning Audit

This audit checks the repository against `PLANNING.md`.

## Completion Criteria

| Criterion | Evidence | Status |
|---|---|---|
| Previous attempt inventoried | `docs/old-attempt-inventory.md` summarizes the old repo structure, corpuses, history signals, architecture lessons, and failure classes. | Complete |
| Focused Java example suite exists | `examples/java/` contains 36 narrow Java fixtures grouped by feature area. | Complete |
| Transformation rules documented | Rule docs exist for source subset, pipeline, type mapping, ownership/mutability, object model, control flow, errors/exceptions, standard library mapping, and Rust emission style. | Complete |
| Every example evaluated | `docs/example-evaluation.md` lists every Java fixture and marks it Covered or Partial with the relevant rule outcome. | Complete |
| Known gaps explicit | `docs/example-evaluation.md` has a Known Design Gaps section. Partial examples keep the unresolved rule visible. | Complete |
| Crate/module layout documented | `docs/crate-layout.md` defines proposed modules and ownership boundaries. | Complete |
| First implementation slice selected | `docs/first-implementation-slice.md` defines included/excluded features, candidate examples, and acceptance criteria. | Complete |

## Artifact Inventory

Design docs:

- `docs/source-subset.md`
- `docs/pipeline.md`
- `docs/methods-and-overloads.md`
- `docs/type-mapping.md`
- `docs/ownership-and-mutability.md`
- `docs/object-model.md`
- `docs/control-flow.md`
- `docs/errors-and-exceptions.md`
- `docs/standard-library.md`
- `docs/rust-emission-style.md`
- `docs/crate-layout.md`
- `docs/first-implementation-slice.md`

Planning and evaluation docs:

- `PLANNING.md`
- `docs/old-attempt-inventory.md`
- `docs/example-suite.md`
- `docs/example-evaluation.md`
- `docs/planning-audit.md`

Java fixtures:

- `examples/java/01-expressions/ExpressionPrecedence.java`
- `examples/java/01-expressions/NumericPromotion.java`
- `examples/java/01-expressions/StringConcat.java`
- `examples/java/02-control-flow/LoopsAndBranches.java`
- `examples/java/02-control-flow/SwitchFallthrough.java`
- `examples/java/02-control-flow/LabeledContinue.java`
- `examples/java/02-control-flow/LabeledContinueForUpdate.java`
- `examples/java/03-methods/MethodsAndOverloads.java`
- `examples/java/03-methods/Varargs.java`
- `examples/java/03-methods/OverloadSameArity.java`
- `examples/java/03-methods/ConstructorOverloads.java`
- `examples/java/03-methods/NullOverloadAmbiguity.java`
- `examples/java/03-methods/VarargsCalls.java`
- `examples/java/04-objects/SimpleClass.java`
- `examples/java/04-objects/InheritanceAndSuper.java`
- `examples/java/04-objects/InterfaceDefault.java`
- `examples/java/04-objects/InnerCapture.java`
- `examples/java/04-objects/AnonymousInterface.java`
- `examples/java/04-objects/LocalClassCapture.java`
- `examples/java/04-objects/AliasingSharedMutation.java`
- `examples/java/05-types/GenericsAndWildcards.java`
- `examples/java/05-types/Nullability.java`
- `examples/java/05-types/ArraysAndVarMutation.java`
- `examples/java/05-types/WildcardBounds.java`
- `examples/java/05-types/RawTypes.java`
- `examples/java/06-stdlib/CollectionsUse.java`
- `examples/java/06-stdlib/OptionalAndObjects.java`
- `examples/java/06-stdlib/StreamsUse.java`
- `examples/java/06-stdlib/CheckedException.java`
- `examples/java/06-stdlib/StreamsCollect.java`
- `examples/java/06-stdlib/StreamsFindMatch.java`
- `examples/java/06-stdlib/CollectorsToMap.java`
- `examples/java/06-stdlib/TryCatchFinally.java`
- `examples/java/06-stdlib/MultiCatch.java`
- `examples/java/06-stdlib/CheckedPropagation.java`
- `examples/java/06-stdlib/StaticOwnerShadowing.java`

## Non-Goals Observed

- No transpiler implementation code was added.
- No Rust crate skeleton was created.
- No old-repo generated scratch artifacts were copied.
