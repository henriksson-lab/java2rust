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

There are currently no known-gap fixtures. `StringEqualsVsEq.java` was promoted
to the main differential suite after the generated `JavaString` carrier gained
literal interning, fresh construction, value equality, and reference identity.
