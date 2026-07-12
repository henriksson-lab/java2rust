# Standard Library Mapping

## Rule Shape

Stdlib mappings must be exact-owner or positive-carrier gated. Do not rewrite a method just because a simple name matches a JDK class.

## Direct Idiomatic Mappings

- `String.length()` -> `.chars().count()` when Java UTF-16 length is not required; otherwise a runtime UTF-16 helper.
- `String.isEmpty()` -> `.is_empty()`.
- `StringBuilder` -> `String` plus `push_str`/`push`, or a runtime wrapper when chaining requires Java shape.
- `List.add` -> `Vec::push`.
- `List.get` -> indexing or `.get(...).expect(...)` according to bounds policy.
- `Map.get` -> `HashMap::get`, with `Option` handling.
- `Optional<T>` -> `Option<T>`.
- `Objects.equals(a,b)` -> null-aware equality helper or direct equality when both slots are non-null.

## Runtime/Shim Mappings

Use runtime shims for Java semantics that do not match Rust directly:

- Java UTF-16 string APIs.
- Java iterator cursor APIs with `hasNext`/`next`.
- Java file/path APIs where exact behavior matters.
- `BigDecimal`, `BigInteger`, date/time, regex matcher state, and formatters.
- Erased `Object` and `Class` carriers.

## Deferred APIs

Streams, collectors, reflection, concurrency, serialization, NIO edge cases, and third-party libraries should be explicit design surfaces. Initial support may handle common exact cases and diagnose the rest.

## Shadowing Rule

If a project class or explicit import named `Collections`, `Objects`, `Integer`, `Paths`, or similar shadows the JDK owner, the JDK rewrite must fail closed.

## Streams And Collectors

Lower Java streams to Rust iterator chains only when all of these are true:

- The receiver carrier is a proven Rust collection or iterator.
- Every intermediate operation has a supported lambda or method-reference shape.
- The terminal operation has a known Rust equivalent.
- No Java stream identity, parallelism, ordering side effect, or checked exception behavior is required.

Supported initial shapes:

- `stream().filter(v -> predicate)` -> `.iter().filter(|v| predicate)`, with borrowing/coercion from the element slot.
- `map(v -> expr)` -> `.map(|v| expr)`.
- `mapToInt`/`mapToLong`/`mapToDouble` -> `.map(...)` plus terminal numeric type.
- `sum()` -> `.sum::<T>()` from the mapped primitive stream type.
- `findFirst()` -> `.find(...)` or `.next()` after filters, returning `Option<T>`.
- `anyMatch(pred)` -> `.any(...)`.
- `collect(Collectors.toList())` -> `.collect::<Vec<_>>()`.
- `collect(Collectors.toSet())` -> `.collect::<HashSet<_>>()`.
- `collect(Collectors.toMap(k, v))` -> `.map(|item| (k(item), v(item))).collect::<HashMap<_, _>>()` only when duplicate-key policy is either Java-compatible for the source or explicitly diagnosed.

Unsupported stream features emit a diagnostic and may use a Java stream runtime shim rather than pretending to be a Rust iterator.

## First Stream Lowering Slice

The first implemented stream slice is exact-shape only:

```java
values.stream()
    .filter(v -> v > 0)
    .mapToInt(v -> v)
    .sum()
```

Requirements:

- The source carrier lowers to `Vec<i32>` from `List<Integer>`.
- The chain must start with `.stream()`.
- Supported intermediate operations are `filter(lambda)` and `mapToInt(lambda)`.
- The terminal operation must be `sum()`.
- Lambda bodies are lowered only for simple expression lambdas over the stream item.

Lowering target:

```rust
values.iter()
    .filter(|v| **v > 0)
    .map(|v| *v)
    .sum::<i32>()
```

This recognizer lives in `lower` as a stdlib-specific expression lowering rule. It must not change `parser` output, and broader stream facts should move into `stdlib`/`resolve` once more stream shapes are supported.
