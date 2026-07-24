# Standard Library Mapping

## Rule Shape

Stdlib mappings must be exact-owner or positive-carrier gated. Do not rewrite a method just because a simple name matches a JDK class.

Rust stdlib/core/alloc are too large to mirror with tailored rules. For
mechanical Rust ABI choices such as receiver borrowing, argument borrowing,
`&mut`, slices, `usize` indices, owned versus borrowed returns, `Option`, and
`Result`, prefer a Rust signature/ABI fact whenever it gives the same emitted
Rust shape as a hand-written rule. Keep tailored rules for Java semantics that a
Rust signature cannot encode, such as null behavior, Java string/runtime
semantics, overload selection, class tokens, synthetic array length, and
control-flow rewrites.

Each stdlib mapping is a transformation rule and must follow
`docs/transformation-rule-contract.md`: document the **Ideal Rust Target**
before the **Implementation Rule**. For example, a stream, collection, string,
or path mapping should first show the idiomatic Rust expression and carrier
shape, then describe the owner/type/capability facts needed to select it.

The short mapping bullets below are an inventory, not implementation-ready
rules by themselves. Before implementing or materially revising any listed
mapping, expand it into a rule entry with **Ideal Rust Target** immediately
before **Implementation Rule**, followed by required facts, owning layer,
fallback shape, and evidence.

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

### Ideal Rust Target

For a Java stream over a proven `List<Integer>`/`Vec<i32>` where the pipeline is
pure local filtering, primitive mapping, and summation, the target Rust should
be an ordinary iterator chain over borrowed elements:

```rust
values
    .iter()
    .filter(|v| **v > 0)
    .map(|v| *v)
    .sum::<i32>()
```

The target is borrowed iteration, not a Java stream wrapper, because the stream
does not need Java cursor identity, parallel stream behavior, checked exception
propagation, mutation during iteration, or collector-specific duplicate-key
semantics. The primitive terminal result is an owned `i32`.

### Implementation Rule

Requirements:

- The source carrier lowers to `Vec<i32>` from `List<Integer>`.
- The chain must start with `.stream()`.
- Supported intermediate operations are `filter(lambda)` and `mapToInt(lambda)`.
- The terminal operation must be `sum()`.
- Lambda bodies are lowered only for simple expression lambdas over the stream item.

This recognizer lives in `lower` as a stdlib-specific expression lowering rule. It must not change `parser` output, and broader stream facts should move into `stdlib`/`resolve` once more stream shapes are supported.
