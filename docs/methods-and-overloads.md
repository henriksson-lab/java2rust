# Methods, Overloads, And Varargs

## Method Names

Java allows overloads by parameter types; Rust does not. The Rust item name must be stable and predictable from the Java signature.

Rules:

- If a Java method name is unique within its declaring type, emit the normal snake-case name.
- If overloaded methods share the same Java name, emit a signature-suffixed Rust name for every overload in that overload set.
- The suffix is derived from erased Java parameter types, lower snake case, joined by `_`.
- Primitive suffixes use Java names: `int`, `long`, `double`, `boolean`.
- Reference suffixes use simple Java type names after resolution: `String` -> `string`, `StringBuilder` -> `string_builder`.
- Array suffixes append `_array`; varargs append `_varargs`.

Example:

- `choose(int)` -> `choose_int`
- `choose(long)` -> `choose_long`
- `choose(String)` -> `choose_string`

Call sites must resolve the Java overload first, then call the suffixed Rust method. Do not select overloads by arity alone.

## Ambiguous Overloads

If Java overload resolution is ambiguous, emit a structured diagnostic and no arbitrary call target. `null` is ambiguous when multiple unrelated nullable reference overloads are applicable and no most-specific overload exists.

## Constructors

Constructors lower to associated functions:

- The zero-argument constructor becomes `new`.
- A single unambiguous constructor may become `new` even with arguments.
- Overloaded constructors use `new_<signature_suffix>`, following method suffix rules.
- Constructor delegation through `this(...)` calls the resolved associated constructor.
- `super(...)` is represented in Java IR and lowers through the object-model rule for base composition.

Example:

- `ConstructorOverloads()` -> `new`
- `ConstructorOverloads(String)` -> `new_string`
- `ConstructorOverloads(String, int)` -> `new_string_int`

## Static Methods

Static methods lower to associated functions on the Rust type. Static overloads use the same suffix rule. Static imports affect name resolution only; they do not change the emitted owner.

## Varargs

The canonical Rust declaration for Java varargs is a slice parameter:

```rust
fn sum(&self, label: String, values: &[i32]) -> i32
```

Call lowering:

- Zero varargs passes `&[]`.
- Inline varargs create an array temporary or slice literal, such as `&[1, 2, 3]`.
- Passing an existing Java array passes `values.as_slice()` for `Vec<T>` storage or `values` when already a slice.
- A varargs parameter used as a Java array inside the method lowers as a slice.

Use `Vec<T>` only when the method stores the varargs array or returns it as owned data.

