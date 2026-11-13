# 0095 — EnumSet.noneOf class-literal argument inference (enum constructor delegation panic)

## Symptom

Translating a large enum whose constants delegate through overloaded
constructors panicked:

```
thread 'main' panicked at src/enum_lowering.rs:350:22:
resolved enum delegation required
```

## Evidence

Minimal repro (enum with constants plus a two-hop `this(...)` delegation
chain through overloads distinguished only by a 5th `EnumSet<Flag>` vs
`byte[]` parameter):

```java
enum EC {
    A(null, "a", new String[0], "b");
    enum Flag { X }
    EC(Supplier<String> s, String d, String[] p, String t) {
        this(s, d, p, t, EnumSet.noneOf(Flag.class));
    }
    EC(Supplier<String> s, String d, String[] p, String t, byte[] uri) {
        this(s, d, p, t, EnumSet.noneOf(Flag.class), uri);
    }
    EC(Supplier<String> s, String d, String[] p, String t, EnumSet<Flag> flags) {
        this(s, d, p, t, flags, new byte[0]);
    }
    EC(Supplier<String> s, String d, String[] p, String t, EnumSet<Flag> flags, byte[] uri) {
        this.desc = d;
    }
    final String desc;
}
```

Instrumentation (since removed) showed `EnumSet.noneOf(Flag.class)` infers
`Unknown("noneOf")`, so the 5-argument delegation is ambiguous between the
`EnumSet<Flag>` and `byte[]` overloads; resolution records an
`resolve_constructor_ambiguous` error and leaves `resolved = None`, and enum
lowering then panics on the constant-invoked constructor. The sibling factory
table already maps `EnumSet.allOf(Class<E>)` to `EnumSet<E>` via
`class_token_element_type`; `noneOf` has the identical JDK signature
(`public static <E extends Enum<E>> EnumSet<E> noneOf(Class<E>)`) and was
missing.

## Classification

Local inference fix: add the missing factory rule. As the working rules
require, the crash boundary was diagnosed before any fix.

## Fix

`src/stdlib.rs`: extend the `EnumSet` class-token rule to cover
`noneOf`/`none_of` alongside `allOf`/`all_of`. The original file now translates
without panicking; remaining `resolve_constructor_ambiguous` diagnostics for
enum constants whose first argument is an unresolved method reference are a
distinct, pre-existing limitation that degrades gracefully.

## Follow-up observed

Batch runs abort on the first panicking file because the emission worker
propagates panics; graceful per-file failure would let the rest of the batch
complete. Not addressed here.
