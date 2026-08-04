# Repro sources

Small Java files used to validate the findings in `claude_audits/`.
Each is deliberately minimal — one behaviour per file.

## Files that reproduce a confirmed defect

| file | issue | what goes wrong |
|---|---|---|
| `LoopMove.java` | [0001](../done/0001-loop-back-edge-invisible-to-move-analysis.md) | `E0382` — value moved in previous loop iteration |
| `Shared.java` | [0002](../0002-object-aliasing-has-no-representation.md) | `E0599` no `clone`; and alias→copy changes the result |
| `Overflow.java` | [0003](../done/0003-java-integer-arithmetic-semantics-not-preserved.md) | debug panic on overflow; `/ 0` panics instead of throwing |
| `StrOp.java` | [0004](../done/0004-string-concat-absorbs-nested-arithmetic.md) | `"total = " + (a+b)` → `"total = 12"`, want `"total = 3"` |
| `Bits.java` | [0005](../done/0005-unsigned-right-shift-collapsed-to-arithmetic-shift.md) | `>>>` behaves as `>>`; also unmasked shift distance |

## Second batch — silent failures (compile cleanly, behave wrongly)

These need to be **run**, not just compiled. Append a `fn main()` and diff
against Java.

| file | issue | what goes wrong |
|---|---|---|
| `LabeledContinue.java`, `L1`–`L4` | [0014](../done/0014-labelled-statements-are-silently-deleted.md) | any labelled loop is **deleted**; `L2` (label, no labelled jump) isolates the trigger, `L1` is the passing control |
| `StaticInit.java` | [0015](../done/0015-static-initializer-blocks-are-silently-dropped.md) | `static { counter = 5; }` dropped → `run()` is `0`, Java `10` |
| `Fallthrough.java` | [0016](../done/0016-core-jdk-facades-are-no-op-stubs.md) | returns `""` — **not** a switch bug; the `StringBuilder` facade is a no-op |
| `GenericBox.java` | [0017](../done/0017-generic-bounds-dropped-and-method-type-params-undeclared.md) | bound dropped (`E0599 compare_to`); `<U>` never declared |

## Negative probes — currently translate correctly

`Fall2.java` is the control for `Fallthrough.java`: identical switch, plain
string concatenation instead of `StringBuilder`. It returns the correct
`"onetwo"` / `"two"` / `"threeother"` / `"other"`, which is what proves the
fallthrough lowering is fine and the fault is in the facade. Keep both.

`FinallyReturn.java`: `try`/`finally` with `return` in the `try` is correct;
`return` inside `finally` is unsupported **and correctly diagnosed** — a good
example of the behaviour 0014 and 0015 should have.

`Boxing.java`: `Integer a = 128, b = 128; a == b` returns `true`; Java returns
`false` (outside the −128..127 cache, `==` is reference equality). Diverges from
Java but may be deliberate — confirm intent before filing, same as `StrEq.java`.


Kept because they pin behaviour that must not regress when the above are fixed.

| file | what it pins |
|---|---|
| `AliasArgs.java` | same value passed twice to one call — clone inserted correctly |
| `BranchMove.java` | move in one branch, use in another |
| `TryMove.java` | move in `try`, use in `catch` |
| `SelfAlias.java` | iterating a field while calling a `&self` method |
| `SelfRecurse.java` | `&mut self` recursion |
| `SelfRecurse.java`, `Overflow.java` | also cover `%` on negatives and narrowing casts, both already correct |

## Not yet conclusive

| file | note |
|---|---|
| `LabeledBreak.java` | needs the `ndarray` crate for `int[][]`, so it cannot be checked with plain `rustc`; verify inside a cargo project before drawing conclusions |
| `StrEq.java` | Java `==` on `String` is reference equality; emitted Rust uses value equality. Diverges from Java, but may be a deliberate choice — confirm intent before filing |

## Checking one

```sh
cargo build --release
./target/release/new-java2rust claude_audits/repro/StrOp.java > /tmp/out.rs
rustc --edition 2021 --crate-type lib --crate-name probe --emit=metadata --out-dir /tmp /tmp/out.rs
```

For runtime divergences, append a `fn main()` that calls the method and build
`/tmp/out.rs` as a binary. The emitted file is self-contained apart from the
`ndarray` case noted above.
