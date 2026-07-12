# Example Evaluation

This document evaluates the current rule docs against every Java fixture. Status values:

- Covered: the current rules are concrete enough to predict the Rust shape.
- Partial: the broad strategy is documented, but a later design detail remains.
- Blocked: the rules intentionally defer the construct.

| Example | Status | Rule evaluation |
|---|---|---|
| `01-expressions/ExpressionPrecedence.java` | Covered | `docs/type-mapping.md` covers primitives and numeric widths; `docs/control-flow.md` covers ternary as `if` expression. Shift uses explicit integer type. |
| `01-expressions/NumericPromotion.java` | Covered | Numeric promotion must insert casts before Rust arithmetic. The rules still need implementation tables, but the design decision is explicit. |
| `01-expressions/StringConcat.java` | Covered | `docs/rust-emission-style.md` and `docs/standard-library.md` allow `format!` for mixed string/numeric concatenation. |
| `02-control-flow/LoopsAndBranches.java` | Covered | Classic `for` lowers through `docs/control-flow.md`; array/list index conversion is a shared helper per type/control-flow docs. |
| `02-control-flow/SwitchFallthrough.java` | Covered | Non-fallthrough grouped cases lower to `match`; string switch uses string matching. |
| `02-control-flow/LabeledContinue.java` | Covered | `docs/control-flow.md` now specifies Rust loop labels and generated update paths for classic `for` continues. |
| `02-control-flow/LabeledContinueForUpdate.java` | Covered | Concrete update-on-continue rewrite is specified in `docs/control-flow.md`; the outer labeled continue must branch through the outer update path. |
| `03-methods/MethodsAndOverloads.java` | Covered | `docs/methods-and-overloads.md` defines signature-suffixed names and resolved-call facts. |
| `03-methods/Varargs.java` | Covered | Varargs lower to slice parameters; enhanced for over the varargs parameter iterates the slice. |
| `03-methods/OverloadSameArity.java` | Covered | Same-arity overloads resolve by Java argument type and emit signature suffixes such as `choose_int`, `choose_long`, and `choose_string`. |
| `03-methods/ConstructorOverloads.java` | Covered | Constructor naming and `this(...)` delegation are defined in `docs/methods-and-overloads.md`. |
| `03-methods/NullOverloadAmbiguity.java` | Covered | `null` cannot select between unrelated nullable reference overloads; expected behavior is a structured ambiguity diagnostic. |
| `03-methods/VarargsCalls.java` | Covered | Zero varargs pass `&[]`, inline varargs pass a slice literal, and array pass-through uses the array/slice carrier rule. |
| `04-objects/SimpleClass.java` | Covered | Struct/impl/constructor/receiver mutability rules predict the Rust shape. |
| `04-objects/InheritanceAndSuper.java` | Partial | Composition plus direct base calls are documented; hierarchy-wide dispatch strategy must be selected when polymorphic references appear. |
| `04-objects/InterfaceDefault.java` | Covered | Interface-to-trait and default method rules predict the Rust shape. |
| `04-objects/InnerCapture.java` | Partial | Capture representation is now defined, but final `&Outer` versus `Rc<RefCell<Outer>>` requires escape facts for this value. |
| `04-objects/AnonymousInterface.java` | Covered | `docs/object-model.md` defines deterministic generated names and anonymous-interface lowering to generated trait implementors. |
| `04-objects/LocalClassCapture.java` | Covered | Local class hoisting, generated naming, and explicit capture fields are now specified. |
| `04-objects/AliasingSharedMutation.java` | Covered | Ownership rules prohibit silent clone; expected result is shared mutable carrier or rewrite-needed diagnostic. |
| `05-types/GenericsAndWildcards.java` | Covered | Generic owner substitution and `List<?>` fresh/erased wildcard policy are specified in `docs/type-mapping.md`. |
| `05-types/Nullability.java` | Covered | Field becomes `Option<String>`; null check lowers to `match`/`if let`; setter wraps non-null value. |
| `05-types/ArraysAndVarMutation.java` | Covered | Mutable array parameter and mutable local rules predict `&mut [i32]` plus `let mut current`. |
| `05-types/WildcardBounds.java` | Covered | `? extends Number` uses a fresh bounded parameter for reads; `? super Integer` uses an erased supertype carrier for writes. |
| `05-types/RawTypes.java` | Covered | Raw collections lower to erased `JavaObject` collection carriers with explicit cast/downcast rules. |
| `06-stdlib/CollectionsUse.java` | Covered | Positive collection/map carrier rules and boxed nullable `Integer` behavior predict `HashMap::get` as `Option<&i32>`. |
| `06-stdlib/OptionalAndObjects.java` | Covered | Exact owner-gated `Objects.equals` and `Optional<T>` -> `Option<T>` are documented. |
| `06-stdlib/StreamsUse.java` | Covered | Supported stream `filter`/`mapToInt`/`sum` lowering is specified in `docs/standard-library.md`. |
| `06-stdlib/CheckedException.java` | Covered | Checked `IOException` maps to `Result`; thrown exception becomes `Err`. |
| `06-stdlib/StreamsCollect.java` | Covered | `collect(Collectors.toList())` lowers to `collect::<Vec<_>>()` under proven stream and exact collector-owner facts. |
| `06-stdlib/StreamsFindMatch.java` | Covered | `findFirst` and `anyMatch` have positive-carrier iterator lowerings. |
| `06-stdlib/CollectorsToMap.java` | Covered | `toMap` lowering is defined when key/value lambda facts and duplicate-key policy are known; otherwise the rule requires a diagnostic. |
| `06-stdlib/TryCatchFinally.java` | Covered | `try` lowers to a `Result` block, catches to enum matches, and simple non-throwing `finally` is duplicated or represented by a scope guard. |
| `06-stdlib/MultiCatch.java` | Covered | Multi-catch lowers to error-enum or-patterns or adjacent equivalent arms while preserving catch order. |
| `06-stdlib/CheckedPropagation.java` | Covered | Checked exception propagation maps to `Result` return and propagation at call site. |
| `06-stdlib/StaticOwnerShadowing.java` | Covered | Exact-owner/shadowing rule says nested project owners must fail closed and avoid JDK rewrites. |

## Known Design Gaps

- Inheritance remains partial for polymorphic hierarchy-wide strategy beyond simple `super` composition.
- Inner-class capture remains partial where escape analysis must choose `&Outer` versus `Rc<RefCell<Outer>>`.
- Stream `toMap` has a concrete rule only when duplicate-key policy is known; otherwise the required behavior is diagnostic.
- Complex `finally` blocks that throw, return, break, continue, or cross borrowed state remain structural-rewrite-needed.
