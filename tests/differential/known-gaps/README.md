# Known-gap differential fixtures

Fixtures here reproduce a confirmed defect. They are **not** run by the main
Java-vs-Rust output comparison in `tests/differential.rs` (which only reads
`*.java` in the parent directory), so the harness stays usable as a gate while
the gap is open.

They are still checked by
`known_gap_fixtures_are_valid_java_and_match_expected_failure_mode`: every
checked-in `*.java` file here must have a Java oracle and an expected current
failure mode.

Some rows also forbid stale generated Rust fragments. That is intentional:
the fixture should not pass merely because it failed loudly for a different
reason after reintroducing an old wrong lowering shape.

Move one back up a level as part of the change that fixes it — that is the
regression test.

## Aliasing.java

Java reference assignment aliases; the translator still lacks mutable shared
object storage.

```
java:        5|7|3|9|11|13|17|19
translation: diagnostic error `observable_alias`
```

See `claude_audits/0002-object-aliasing-has-no-representation.md`. The
translator now reports the provable mutable-alias divergence as an
`observable_alias` error (`src/aliasing.rs`), so the known-wrong Rust is no
longer a successful translation. The fixture covers local aliases, receiver
mutation through an alias, array-element storage, holder-field storage, and
this-field storage. Closing this still needs `SharedLocal`/`Rc<RefCell<T>>`
storage plus receiver/field lowering for mutable shared source objects.

## InheritanceUpcast.java

Java dispatch through a supertype-typed variable keeps the subtype object and
override. The current translation refuses this with:

```
unsupported Java class upcast: Savings to Account requires class hierarchy lowering
```

The harness also requires `unsupported_inheritance`.

Move this fixture to `tests/differential/` when class hierarchy lowering can
preserve dynamic dispatch for source classes.

## ExplicitInheritanceUpcast.java

The same source-class upcast gap appears through an explicit cast expression.
Java keeps the dynamic `Savings` object and dispatches to the override:

```
java: savings
translation: compile_error!("unsupported Java class upcast: Savings to Account requires class hierarchy lowering")
```

The harness also requires `unsupported_inheritance`.

Move this fixture to `tests/differential/` with `InheritanceUpcast.java` when
class hierarchy lowering preserves dynamic dispatch for source classes.

## SuperMutatingCall.java

Java `super.bump()` inside an override mutates the base subobject of the same
dynamic receiver. The current translation refuses the mutating super call
because there is no hierarchy carrier to preserve that receiver shape:

```
java: 11
translation: compile_error!("unsupported Java mutating super call: Base.bump requires class hierarchy lowering")
```

The harness also requires `unsupported_inheritance`.

Move this fixture to `tests/differential/` when source-class hierarchy lowering
can route mutating super calls through the carrier.

## NestedInheritanceUpcast.java

The same source-class upcast gap also exists for nested classes. Java keeps the
dynamic nested subtype and dispatches to `Dog.speak()` through an
`Animal`-typed slot:

```
java: woof,woofwoof
translation: compile_error!("unsupported Java class upcast: NestedInheritanceUpcast.Dog to NestedInheritanceUpcast.Animal requires class hierarchy lowering")
```

The harness also requires `unsupported_inheritance`.

Move this fixture to `tests/differential/` with `InheritanceUpcast.java` when
class hierarchy lowering preserves dynamic dispatch for source classes.

## PolymorphicListUpcast.java

The same source-class upcast gap appears when a subtype value is stored through
a collection whose element type is the superclass. Java keeps the dynamic
`Savings` object and dispatches through the `Account`-typed element:

```
java: savings,105
translation: compile_error!("unsupported Java class upcast: Savings to Account requires class hierarchy lowering")
```

The harness also requires `unsupported_inheritance`.

Move this fixture to `tests/differential/` with `InheritanceUpcast.java` when
class hierarchy lowering preserves dynamic dispatch for source classes.

## InnerCapture.java

Java non-static inner-class construction captures the enclosing instance by
reference. The current translation emits the captured `__outer` field and
non-panicking `_with_outer` companions, but ordinary `new Inner()` from an outer
instance method still has no shared outer receiver handle, so the missing
constructor path is reported as a runtime-loud unsupported feature while the
translator emits the `inner_class_construction_panics` diagnostic.

```
java: 21
translation: runtime panic containing "unsupported Java inner-class outer capture construction"
diagnostic: inner_class_construction_panics
```

Move this fixture to `tests/differential/` when ordinary inner construction can
route through the shared outer handle path.

## QualifiedInnerCapture.java

Qualified non-static inner construction has the same outer-handle requirement
as ordinary `new Inner()`. The parser/capture path now recognizes
`other.new Inner()` and reports the same loud unsupported-construction
diagnostic, but lowering still cannot produce the shared outer receiver handle.

```
java: 21
translation: runtime panic containing "unsupported Java inner-class outer capture construction"
diagnostic: inner_class_construction_panics
```

Move this fixture to `tests/differential/` with `InnerCapture.java` when
ordinary and qualified inner construction can route through the shared outer
handle path.

## InstanceofSupertype.java

Java `instanceof` is a subtype test. The current exact erased-object runtime
test cannot answer `new B() instanceof A`, so the translation refuses this with:

```
unsupported Java instanceof check: target type A has source subtypes and requires class hierarchy lowering
```

The harness also requires
`unsupported_instanceof_source_hierarchy_runtime_carrier`.

Move this fixture to `tests/differential/` when subtype-aware runtime tests are
implemented.

## ErasedObjectSupertypeCast.java

Java casts from `Object` to a source superclass are subtype-aware. The current
erased-object cast cannot recover that hierarchy, so the translation refuses
the cast:

```
java: B
translation: compile_error!("unsupported Java cast: target type A has source subtypes and requires class hierarchy lowering")
```

The harness also requires `unsupported_cast_source_hierarchy_runtime_carrier`.

Move this fixture to `tests/differential/` when subtype-aware erased-object
casts are implemented.

## PatternInstanceofSupertype.java

Pattern `instanceof` has the same subtype-aware requirement as plain
`instanceof`, plus binding the narrowed value. The current translation refuses
the carrier-dependent test:

```
java: A,A,other
translation: compile_error!("unsupported Java instanceof check: target type A has source subtypes and requires class hierarchy lowering")
```

The harness also requires
`unsupported_instanceof_source_hierarchy_runtime_carrier`.

Move this fixture to `tests/differential/` when subtype-aware runtime tests and
pattern bindings are implemented.

## PatternInstanceofSupertypeTernary.java

The same pattern `instanceof` source-supertype gap also appears in ternary
conditions:

```
java: A,A,other
translation: compile_error!("unsupported Java instanceof check: target type A has source subtypes and requires class hierarchy lowering")
```

The harness also requires
`unsupported_instanceof_source_hierarchy_runtime_carrier`.

Move this fixture to `tests/differential/` with the statement-form pattern
fixture when subtype-aware runtime tests and pattern bindings are implemented.

## StringEqualsVsEq.java

Java `String ==` is reference equality. The current translation refuses this
case with a loud `compile_error!` and reports `java_string_identity`, because
strings do not yet have an identity-preserving representation.

```
java:        true,false,true,true,false
translation: compile_error!("unsupported Java String reference identity comparison requires string identity representation")
```

Move this fixture to `tests/differential/` only if Java string identity gets a
real representation. This is the documented policy boundary from 0028, not an
exception-lowering gap.

## PriorityQueueComparatorConstructor.java

Java `PriorityQueue(Comparator)` must preserve comparator-defined ordering. The
current translation refuses comparator-backed priority queues instead of
silently using the natural ordering carrier.

The checked-in fixture uses an anonymous `Comparator`, so the current translation
also reports `unsupported_anonymous_class`. Promoting it requires both anonymous
comparator object lowering and a comparator-backed priority-queue carrier. The
harness also requires `unsupported_priority_queue_comparator_constructor`.

## PriorityQueueNamedComparatorConstructor.java

This is the same `PriorityQueue(Comparator)` ordering gap, but with a named
`Comparator` implementation. It isolates the queue-carrier problem from
anonymous-class lowering: promoting this fixture should require only a
comparator-backed priority-queue carrier. The harness also requires
`unsupported_priority_queue_comparator_constructor`.
