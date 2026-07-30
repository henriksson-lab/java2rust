# 0027 — Compound assignment omits Java's implicit narrowing cast

Status: **validated** (rustc-confirmed)
Severity: medium — always a compile error on narrow types
Kind: small lowering gap, exact fix

## Symptom

```java
public int narrowCompound(int x) { byte b = 10; b += 300; return b; }
```

Emitted:

```rust
let mut b: i8 = 10;
b = (b as i32) + 300;      // i32 assigned to i8
return b as i32;
```

```
error[E0308]: mismatched types
1684 |         b = (b as i32) + 300;
     |             ^^^^^^^^^^^^^^^^ expected `i8`, found `i32`
```

## Root cause

The lowering correctly applies Java's *binary numeric promotion* to the operands
(`b as i32`), but omits the **implicit narrowing cast back to the target type**
that Java's compound assignment operators include.

JLS 15.26.2: `E1 op= E2` is equivalent to
`E1 = (T)((E1) op (E2))` where `T` is the type of `E1`. The cast is part of the
definition — this is why `byte b = 10; b += 300;` compiles in Java (and yields
`54`, since `310` truncated to a byte is `54`), while `b = b + 300;` does **not**
compile.

So one half of the rule is implemented and the other half is missing.

## Fix

Emit the narrowing cast on the result whenever the target type is narrower than
the promoted arithmetic type:

```rust
b = ((b as i32) + 300) as i8;
```

Applies to `byte`, `short`, and `char` targets for all compound operators
(`+= -= *= /= %= &= |= ^= <<= >>= >>>=`). Interacts with:

- [0003](0003-java-integer-arithmetic-semantics-not-preserved.md) — the
  arithmetic itself should be wrapping, so the full shape is
  `((b as i32).wrapping_add(300)) as i8`;
- [0005](0005-unsigned-right-shift-collapsed-to-arithmetic-shift.md) — `>>>=`
  needs the unsigned variant *and* the narrowing cast;
- `char` targets need the `char ↔ u32` round trip already used elsewhere
  (`char::from_u32(… & 0xffff)`).

## Note

Unlike most findings here this one fails **loudly**, so it is low-risk to fix and
cannot be hiding behind a clean corpus run. It is included because it is small,
exact, and sits in the same code path as 0003 and 0005 — worth doing in the same
pass rather than separately.

## Regression tests worth adding

```java
byte  b = 10;  b += 300;   // 54
short s = 1;   s *= 100000; // -31072
char  c = 'a'; c += 1;      // 'b'
byte  n = -128; n -= 1;     // 127   (wrapping, with 0003)
```
