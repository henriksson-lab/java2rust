# Known-gap differential fixtures

Fixtures here reproduce a confirmed defect. They are **not** run by the main
Java-vs-Rust output comparison in `tests/differential.rs` (which only reads
`*.java` in the parent directory), so the harness stays usable as a gate while
the gap is open.

They are still checked by
`known_gap_fixtures_are_valid_java_and_match_expected_failure_mode`: every
checked-in `*.java` file here must have a Java oracle and an expected current
failure mode.

Move one back up a level as part of the change that fixes it — that is the
regression test.

## Aliasing.java

Java reference assignment aliases; the translation copies.

```
java:        5|7|3|9
translation: 0|0|0|0
```

See `claude_audits/0002-object-aliasing-has-no-representation.md`. The
translator now *reports* all four cases as `observable_alias` warnings
(`src/aliasing.rs`), so they are no longer silent — but reporting is not repair,
and the output still computes the wrong answer. Closing this needs a shared
representation in `ObjectRepresentation`.

## InheritanceUpcast.java

Java dispatch through a supertype-typed variable keeps the subtype object and
override. The current translation refuses this with:

```
unsupported Java class upcast: Savings to Account requires class hierarchy lowering
```

The harness also requires `unsupported_inheritance`.

Move this fixture to `tests/differential/` when class hierarchy lowering can
preserve dynamic dispatch for source classes.

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

## InnerCapture.java

Java non-static inner-class construction captures the enclosing instance by
reference. The current translation emits the captured `__outer` field and
non-panicking `_with_outer` companions, but ordinary `new Inner()` from an outer
instance method still has no shared outer receiver handle, so the missing
constructor path is reported as a runtime-loud unsupported feature while the
translator still emits the `inner_class_construction_panics` diagnostic.

```
java: 21
translation: runtime panic containing "unsupported Java inner-class outer capture construction"
diagnostic: inner_class_construction_panics
```

Move this fixture to `tests/differential/` when ordinary inner construction can
route through the shared outer handle path.

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
