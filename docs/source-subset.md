# Source Subset

## Initial Target

The first supported subset is Java 8-ish source with ordinary classes, methods, primitive expressions, strings, arrays, collections, and basic control flow. The design should not depend on Java 8 specifically, but lambdas, streams, and default interface methods are important enough to model early.

## Supported In First Design Pass

- Packages and imports.
- Top-level classes and interfaces.
- Fields, static fields, constructors, instance methods, static methods.
- Primitive types, `String`, arrays, simple generic class and method uses.
- `if`, `while`, `do`, classic `for`, enhanced `for`, `break`, `continue`, `return`, and `switch`.
- Expressions including assignment, arithmetic, comparisons, boolean logic, casts, ternary, method calls, field access, object creation, and array access.
- `null` as an explicit nullable signal.
- Basic exceptions as a design surface.
- Common `java.lang`, `java.util`, and `java.io`/`java.nio` surfaces by exact rule or explicit defer.

## Deferred Or Report-Only

- Reflection and dynamic class loading.
- Concurrency primitives beyond explicit runtime shims.
- Native methods.
- Serialization semantics.
- Annotation processing.
- Complex wildcard capture and recursive generic bounds.
- Full checked-exception compatibility when the Rust API should instead become idiomatic `Result`.
- Anonymous/local classes and captured inner classes beyond documented expected lowering.

## Unsupported Means Diagnostic

Unsupported constructs must produce a structured diagnostic with source span and reason. They must not fall through to misleading Rust. A diagnostic can still allow best-effort output when the generated Rust is clearly marked as incomplete.

