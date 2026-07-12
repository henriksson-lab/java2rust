# Errors And Exceptions

## Mapping Policy

Use idiomatic Rust `Result<T, E>` for recoverable checked exception paths in translated public APIs. Use panic only for Java runtime exceptions whose Java behavior is also unchecked failure, such as bounds errors, null dereference, or illegal state where no local recovery exists.

## Checked Exceptions

Methods declaring checked exceptions should lower to `Result<Return, ErrorType>` unless the call site catches and fully handles the exception locally. Multiple checked exception types can initially share a generated enum.

The generated error enum is per Rust module unless a package-level exception hierarchy is available. Variant names follow resolved Java exception simple names, for example `JavaIoError::IOException`. Unknown checked exceptions use `JavaError::External { class_name, message }`.

## Runtime Exceptions

Runtime exceptions lower by category:

- Bounds/null failures: rely on Rust panic or explicit `expect` with provenance.
- `IllegalArgumentException`/`IllegalStateException`: generated error enum if used in recoverable API, otherwise panic.
- Unsupported external exceptions: diagnostic plus generated placeholder error type.

## Try/Catch/Finally

- `try/catch` lowers to `match` on `Result`.
- `finally` requires scope-exit behavior. Simple non-throwing cleanup can become explicit code on every exit path. Complex `finally` should be structural-rewrite-needed.
- Catch type ordering must be preserved.

Concrete lowering rules:

- A `try` body whose fallible operations return `Result<T, E>` is emitted as an inner closure or block returning `Result<T, E>`.
- Each `catch` arm matches variants of the generated error enum.
- Multi-catch expands to an or-pattern when all caught Java types map to the same Rust enum, otherwise to adjacent arms with identical bodies.
- A method with `throws E` and no local catch returns `Result<T, E>`, and calls to fallible operations use `?`.
- A local catch that fully handles the error returns the enclosing method's ordinary return type.
- A simple `finally` block that cannot throw is duplicated after normal and caught paths, or emitted through a small scope guard in Rust IR before final codegen.
- A `finally` block that can throw, return, break, continue, or mutate borrowed state across exits is structural-rewrite-needed.

## Throws Documentation

Every lowered `Result` should preserve Java `throws` provenance in docs until implementation proves the mapping is stable.

## First Exception Lowering Slice

The first implemented exception slice is intentionally narrow:

- `parser` records declared `throws` types on methods and constructors.
- `resolve` does not choose exception strategy; it only keeps expression and call facts available.
- `lower` maps any method with a non-empty `throws` list to `Result<T, JavaException>`.
- `lower` maps `throw new E(message)` to `return Err(JavaException::new("E", message));`.
- Normal `return expr;` inside a throwing method lowers to `return Ok(expr);`.
- `return;` inside a throwing `void` method lowers to `return Ok(());`.
- `try`/`catch`/`finally` remains structurally represented but unsupported until result-flow rewriting is implemented.

The first generated `JavaException` is a compatibility struct:

```rust
pub struct JavaException {
    pub class_name: &'static str,
    pub message: String,
}
```

This is deliberately simpler than the eventual typed exception enum. It gives fallible APIs a compiling Rust surface while preserving the Java exception class name for later refinement.
