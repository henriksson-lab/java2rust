# Example Suite

The example suite is intentionally narrow. Each file exists to force one or two translation decisions, not to model a realistic application.

## Inventory

| Example | Primary pressure | Expected Rust shape |
|---|---|---|
| `01-expressions/ExpressionPrecedence.java` | Operator precedence, ternary, shifts | Direct expressions, explicit parentheses only where needed, ternary as Rust `if` expression |
| `01-expressions/NumericPromotion.java` | Java numeric promotion | Explicit widening casts before mixed arithmetic |
| `01-expressions/MixedNumericArithmetic.java` | Mixed integer/floating arithmetic and comparison | Integer operands coerced before float operations/comparisons |
| `01-expressions/StringConcat.java` | String concatenation | `format!` or `String` builder, no numeric `+` confusion |
| `02-control-flow/LoopsAndBranches.java` | Classic `for`, array indexing, break/continue | `while`/`loop` preserving update after `continue`, indices via shared Java-index coercion |
| `02-control-flow/SwitchFallthrough.java` | Grouped cases, string switch | `match grade.as_str()` with grouped `"B" | "C"` arm |
| `02-control-flow/LabeledContinue.java` | Java loop labels | Rust loop labels; classic-for update semantics preserved |
| `02-control-flow/LabeledContinueForUpdate.java` | Labeled continue in classic `for` | Structural loop lowering that executes the outer update before continuing |
| `02-control-flow/ForOmittedParts.java` | `for` loops with omitted init/condition/update | Structural loop lowering that preserves break/update behavior |
| `02-control-flow/LabeledBreak.java` | Java labeled break | Rust loop labels or equivalent structured break, no dropped body |
| `03-methods/MethodsAndOverloads.java` | Static methods and overload resolution | Static associated function, overloaded methods disambiguated by signature-derived Rust names |
| `03-methods/Varargs.java` | Varargs and enhanced for | Varargs as slice parameter; enhanced for over values |
| `03-methods/OverloadSameArity.java` | Same-arity overloads | Signature-aware dispatch; distinct Rust names or generated dispatcher |
| `03-methods/ConstructorOverloads.java` | Constructor overloads and delegation | Distinct constructor functions plus exact `this(...)` delegation |
| `03-methods/NullOverloadAmbiguity.java` | `null` overload ambiguity | Structured ambiguity diagnostic instead of arbitrary overload choice |
| `03-methods/VarargsCalls.java` | Zero/many/array varargs calls | Slice parameter with `&[]`, slice literals, or array/slice pass-through |
| `03-methods/NamingPolicy.java` | Java identifier casing in fields, methods, and calls | Consistent snake-case names with call sites updated from resolved symbols |
| `04-objects/SimpleClass.java` | Fields, constructor, `this`, mutation | Struct with `name: String`, `count: i32`, `new`, `increment(&mut self)`, `label(&self)` |
| `04-objects/InheritanceAndSuper.java` | Base state and `super` | Composition with `base: BaseCounter`; direct base method call |
| `04-objects/InterfaceDefault.java` | Interface default method | Trait with required `name` and provided `display` |
| `04-objects/InnerCapture.java` | Captured outer `this` | Inner struct stores explicit outer reference/handle and offset |
| `04-objects/AnonymousInterface.java` | Anonymous interface implementation | Generated concrete type or closure-backed trait object with provenance |
| `04-objects/LocalClassCapture.java` | Method-local class capture | Hoisted generated type with explicit captured locals |
| `04-objects/AliasingSharedMutation.java` | Java reference aliasing | Shared mutable representation or rewrite-needed diagnostic; no silent deep clone |
| `04-objects/ConstructorExpressions.java` | Constructor calls as expressions | `new`/constructor functions usable in method-call receiver positions |
| `04-objects/EnumConstantsWithFields.java` | Enum constants with constructors and fields | Rust enum plus associated payload/facts, or structured unsupported diagnostic |
| `04-objects/FieldWriteReceiverMutation.java` | Field writes inside methods | Receiver shape becomes mutable when method body mutates instance state |
| `04-objects/StaticFinalFields.java` | Static final constants versus instance fields | Associated constants/statics are not emitted as ordinary struct fields |
| `05-types/GenericsAndWildcards.java` | Generic owner substitution and wildcard | `GenericsAndWildcards<T>` with list storage; wildcard diagnosed or fresh generic for `size_of` |
| `05-types/Nullability.java` | Nullable field and null check | `cached: Option<String>`, `if let`/`match`, assignment wraps value |
| `05-types/ArraysAndVarMutation.java` | Mutable array parameter, local mutability | `&mut [i32]` or `&mut Vec<i32>`, `let mut current` |
| `05-types/ArrayInitializers.java` | One- and two-dimensional array assignment | Array writes lower to indexed vector/slice updates with numeric coercion |
| `05-types/WildcardBounds.java` | `? extends` and `? super` | Read-only covariant and write-only contravariant rules, or explicit defer |
| `05-types/RawTypes.java` | Raw collection erasure | Erased object carrier, not `_` in Rust fields/signatures |
| `06-stdlib/CollectionsUse.java` | Map/list carrier rules, boxed nullable `Integer` | `HashMap<String, i32>` with `get` returning `Option<&i32>` |
| `06-stdlib/OptionalAndObjects.java` | Exact JDK `Objects` and `Optional` | Null-aware equality helper or direct `Option` equality; `Option::unwrap_or`/`unwrap_or_else` shape |
| `06-stdlib/StreamsUse.java` | Stream chain | Iterator chain when receiver is proven `List<Integer>` |
| `06-stdlib/CheckedException.java` | Checked exception | `Result<String, JavaIoError>` or generated error enum |
| `06-stdlib/StreamsCollect.java` | Stream `filter`/`map`/`collect(toList)` | Proven stream lowers to iterator chain plus `collect::<Vec<_>>()` |
| `06-stdlib/StreamsFindMatch.java` | Stream `findFirst` and `anyMatch` | `find`/`any` iterator lowering with `Option<T>` result |
| `06-stdlib/CollectorsToMap.java` | `Collectors.toMap` | `collect::<HashMap<K,V>>()` only with proven key/value lambda facts |
| `06-stdlib/TryCatchFinally.java` | Local catch plus finally | `Result` match plus explicit cleanup on all exit paths or rewrite-needed |
| `06-stdlib/MultiCatch.java` | Multi-catch | Error enum/category matching preserving catch order |
| `06-stdlib/CheckedPropagation.java` | Checked exception propagation | `Result` return and `?`-style propagation |
| `06-stdlib/StaticOwnerShadowing.java` | JDK owner shadowing | Project nested owners must not trigger JDK `Objects`/`Collections`/`Integer` rewrites |
| `06-stdlib/StringFormatAndThrow.java` | `String.format` and exception construction | Formatting and thrown exceptions route through owned stdlib/exception policies |
| `06-stdlib/SynchronizedBlock.java` | Java `synchronized` block | Explicit synchronization lowering or structured unsupported diagnostic; body is not dropped |

## Fixture Policy

Each example should stay small enough that a failed rule can be identified by reading one file. When a real corpus exposes a new class of issue, add a minimized example here before changing implementation rules.
