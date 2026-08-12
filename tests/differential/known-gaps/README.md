# Known-gap differential fixtures

Fixtures here reproduce a confirmed defect. They are **not** run by
`tests/differential.rs` (which only reads `*.java` in the parent directory), so
the harness stays usable as a gate while the gap is open.

Move one back up a level as part of the change that fixes it — that is the
regression test.

## Generics.java

Generic values render as the empty string in string concatenation.

```
java:        5,true,false,11
translation: ,true,false,
```

The comparisons are right, so bounds projection and dispatch work. The values
vanish because a type-parameter-shaped operand takes the erased-display path and
`java_erased_object_to_string` is a no-op:

```rust
pub fn java_erased_object_to_string<T: ?Sized>(_value: &T) -> String {
    Default::default()
}
```

It has no `Display` bound, so it cannot format anything — the 0016 no-op-facade
class, in the support library rather than a JDK facade.

Fix direction: a type parameter used in a concatenation needs a display
capability, exactly as one that gets cloned needs `Clone`
(`callable_type_param_needs_clone_bound`). Infer a `ToString` bound for such a
parameter and emit `.to_string()` instead of routing it through the erased
helper. The erased helper should stay for genuinely erased values, and should
be made loud rather than returning `""`.

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
