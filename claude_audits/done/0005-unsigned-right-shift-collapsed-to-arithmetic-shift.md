# 0005 — Java `>>>` is collapsed into `>>` at parse time

Status: **validated** (runtime-confirmed wrong output; compiles cleanly)
Severity: high — silent wrong answer on every negative operand
Kind: missing IR node — information destroyed in the parser, unrecoverable downstream

## Symptom

Java's unsigned (logical) right shift `>>>` is translated to Rust's `>>`, which
on a signed integer type is an **arithmetic** shift. Every negative operand
gives the wrong result. The output compiles and runs.

## Minimal repro

```java
public class Bits {
    public int ushift(int a) { return a >>> 8; }
}
```

| | `ushift(-1)` |
|---|---|
| Java | `16777215` |
| emitted Rust | `-1` |

Verified by running the emitted file:

```
ushift(-1)  = -1   (Java: 16777215)
```

## Emitted Rust

```rust
pub fn ushift(&self, a: i32) -> i32 {
    return a >> 8;      // arithmetic shift: sign-fills
}
```

## Root cause

`BinaryOp` in `src/java_ir.rs` has no unsigned-shift variant:

```rust
pub enum BinaryOp {
    Assign, Add, Sub, Mul, Div, Rem,
    BitAnd, BitOr, BitXor,
    Shl, Shr,            // <-- no UShr
    Eq, Ne, Lt, Le, Gt, Ge, And, Or,
}
```

and the parser maps both spellings onto the one variant:

- `src/parser.rs:2564` — `">>" | ">>>" => BinaryOp::Shr,`
- `src/parser.rs:2543` — `Some(">>=") | Some(">>>=") => BinaryOp::Shr,`

The distinction is destroyed before any analysis or lowering runs. No downstream
fact, plan, or rule can recover it — there is nothing left to inspect. This is
why it is worth treating as its own class of defect: unlike a lowering bug, no
amount of work in `src/lower.rs` can fix it.

## Fix

1. Add `BinaryOp::UShr` (and handle `>>>=` in the compound-assignment path).
2. Parse `">>>" => BinaryOp::UShr` and `">>>=" => BinaryOp::UShr`.
3. Lower `UShr` through the unsigned type of the operand's width:
   - `int`:  `((a as u32) >> n) as i32`
   - `long`: `((a as u64) >> n) as i64`
4. Add the new variant to the operator tables in `src/operator_lowering.rs`
   (`:71` spelling map, `:99` and `:117` operator-class checks). The compiler
   will point at each non-exhaustive match, so the change is mechanical once the
   variant exists.

## Related: shift distance is not masked

The same probe showed a second, independent shift defect. Java masks the shift
distance to the low 5 bits for `int` (6 for `long`), so `1 << 33 == 2`. Rust
panics in debug on an over-wide shift:

```
shift(1, 33)  ->  thread 'main' panicked: attempt to shift left with overflow
```

Java gives `2`. This one is a policy question rather than a missing node, and
belongs with the arithmetic-semantics work in
[0003](0003-java-integer-arithmetic-semantics-not-preserved.md) — the fix is
the same mechanism (`wrapping_shl` / explicit `& 31` masking).

## Regression tests worth adding

```java
-1  >>> 8      // 16777215
-1  >>  8      // -1          (must stay arithmetic)
-1L >>> 8      // 72057594037927935
1   <<  33     // 2           (masking, see 0003)
x >>>= 8       // compound form goes through the same path
```

Include the plain `>>` case in the same test — it pins that the fix did not
convert *both* operators to logical shifts.
