# Type Mapping

## Primitive Types

| Java | Rust |
|---|---|
| `boolean` | `bool` |
| `byte` | `i8` for scalar semantics, `u8` only for byte-buffer APIs with explicit rule |
| `short` | `i16` |
| `char` | `char` for Unicode scalar intent, `u16` only under explicit UTF-16 rule |
| `int` | `i32` |
| `long` | `i64` |
| `float` | `f32` |
| `double` | `f64` |

Numeric promotion must be explicit. Do not rely on Rust inference to reproduce Java promotion.

## Reference Types

- Non-null project class values default to owned structs or references chosen by slot context.
- Nullable references are `Option<T>`.
- Java `String` maps to `String` for owned values and `&str` only when the Rust API is intentionally idiomatic and the value is borrowed.
- Boxed primitives map to `Option<primitive>` only when nullable; otherwise they should usually lower to primitives plus exact boxed-method rewrites.

## Arrays And Collections

- Java arrays map to `Vec<T>` unless fixed-size Rust arrays are explicitly proven useful.
- `List<T>` maps to `Vec<T>` as storage.
- `Set<T>` maps to `HashSet<T>` with `T: Eq + Hash`.
- `Map<K,V>` maps to `HashMap<K,V>` with `K: Eq + Hash`.
- Broad `Collection<T>` and `Iterable<T>` may use `Vec<T>` for storage but must not expose list-only operations such as indexed `get`.

## Generics

Generic owner parameters must be substituted at method call sites.

- Use fresh Rust generic parameters when the source position is statically typed and bounded.
- Use `JavaObject`/erased runtime carrier only when Java erasure is semantically required.
- Never emit `_` in struct fields.

### Wildcards

Use declaration-site helper parameters rather than Rust `_`.

- `List<?>` in a read-only parameter becomes `&[JavaObject]` only when element operations are erased; otherwise introduce a fresh type parameter such as `W`.
- `List<? extends Number>` in a read position becomes a fresh parameter with the required bound, for example `W: JavaNumber`, and a slice/reference to `W`.
- `List<? super Integer>` in a write position becomes an erased supertype carrier when the exact target cannot be represented. For the initial design, use `Vec<JavaObject>`/`&mut Vec<JavaObject>` and insert values through `Into<JavaObject>`.
- Wildcard fields are not emitted with `_`; they use a generated owner parameter or an erased carrier.

### Raw Types

Raw Java collections lower to erased carriers:

- Raw `List` -> `Vec<JavaObject>`.
- Raw `Map` -> `HashMap<JavaObject, JavaObject>`.
- Raw `Set` -> `HashSet<JavaObject>`.

Operations on erased carriers preserve Java runtime uncertainty. Reads return `JavaObject`; typed extraction requires explicit cast/downcast rules. Writes convert values into `JavaObject`.

## Nullability Overlay

Nullability is an overlay on the resolved underlying type. `null`, assignments of `null`, nullable returns, checked `x == null`, and nullable external annotations feed this overlay. Constructor definite-non-null facts may remove `Option` only when every constructor path proves initialization before escape.

For un-translated external Java API parameters, the default boundary assumption
is non-null unless the source program, a known external pack, a source summary,
or a future injected fact says otherwise. This keeps Rust signatures from
spreading `Option<T>` through ordinary callbacks, handles, readers, and helper
objects just because the callee is not translated. Evidence that widens the
slot includes observed `null` call sites, Java null checks around the value,
nullable annotations or library models, explicit nullable parameter facts, or
user-provided override facts. The rule is still fact-driven: injected facts
must take precedence over the default, and the default must be recorded as a
boundary assumption rather than rediscovered in lowering. Return and field
nullability remain separate external facts until we deliberately lock an
equivalent default for those boundaries.
