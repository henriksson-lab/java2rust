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

`MapFieldEntryMutation.java` was promoted after native-map fields whose entries
escape gained shared tracked-map storage. Ordinary field map calls borrow that
same handle, while the entry retains its stable node ID. The promoted oracle
covers HashMap, LinkedHashMap, and TreeMap; Java and Rust both print
`1:7:7,2:8:8,4:9:9:a`.

`NullableMapFieldEntryMutation.java` was promoted after literal-null key/value
evidence began shaping the live field carrier's generic arguments. Its runtime
entry adapter still exposes `Map.Entry<String, Integer>` while the tracked map
stores optional keys/values. Java and Rust print `null:7,null:8,null:9:a`.

`StaticMapFieldEntryMutation.java` was promoted after live native-map handles
and entry observers gained synchronized `Arc/RwLock/Weak` storage. The escaped
entry and process-wide static field now share the same tracked node; Java and
Rust agree across HashMap, LinkedHashMap, and TreeMap. The adjacent nullable
static-field oracle is promoted too.

`TreeMapEntryParameterMutation.java` was promoted after a source method that
obtains a live entry from a `TreeMap` parameter gained a shared tracked-map
argument carrier. Java and Rust both print `1:7:a`.

`NullableMapEntryParameterMutation.java` was promoted after live native-map
parameters became generic over their Rust key and value storage. The same Java
helper can now receive a tracked map storing either `String` or
`Option<String>` keys and either `i32` or `Option<i32>` values while its
`Map.Entry<String, Integer>` contract remains nullable. Java and Rust both
print `null:7,null:8,null:9:t` across HashMap, LinkedHashMap, and TreeMap.

`GenericErasedDisplay.java` was promoted to the main differential suite after
concrete generic source instantiations, including declared slots with diamond
constructors, gained erased-display downcast arms.

`GenericNullableFieldDisplay.java` was promoted after single-use generic
constructor transfer and null-aware generic field display were implemented.
Java and Rust both print `Box(a),Box(null),Box(2),Box(payload)`.

`NullableObjectsEquals.java` was promoted after declared `String` locals kept
concrete dispatch at an erased `Objects.equals` call boundary. It prints
`false` without making `String.equals` on a null receiver null-safe.
