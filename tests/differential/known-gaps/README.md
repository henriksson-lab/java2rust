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

## ErasedCarrierIdentity.java

Java casts from `Object` back to a source superclass preserve object identity.
The current source-hierarchy carrier can recover the dynamic subtype for
dispatch, but recovered carriers clone the payload and cannot yet answer Java
reference identity or inherited `Object.equals` correctly.

```
java: true,false,true,false
translation: compile_error!("unsupported Java source hierarchy carrier reference identity comparison requires identity-preserving carrier representation")
diagnostic: java_reference_identity_equality
```

Move this fixture to `tests/differential/` when the source hierarchy carrier
preserves identity across erased-object recovery.

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
