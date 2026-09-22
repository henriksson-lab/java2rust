# 0081 — Optional mutable arrays were not reborrowed consistently

Status: **fixed** — optional mutable array parameters use mutable slice slots
and can be reborrowed for writes, recursive calls, and readonly helper calls
without moving the option.

## Findings and repair

The lowering paths already emitted the required `as_deref_mut`, `as_deref`,
and temporary reborrows, but the optional-array renderer declared the same
values as owned vectors. That representation mismatch surfaced as invalid
generated Rust, misleading keyword-binding output, repeated optional receiver
adaptation, and apparent move-after-use failures.

Preserving `SharedBorrow` and `MutableBorrow` ownership through optional slice
rendering makes the declaration and use sides agree. The repair is structural:
owned optional arrays still render as `Option<Vec<T>>`; only borrowed solved
slots render as `Option<&[T]>` or `Option<&mut [T]>`.

## Verification

Generated-crate tests cover a Rust-keyword parameter, guarded writes, a
recursive nullable out parameter, and mutable-to-readonly reborrowing. All pass
without weakening the negative assertions against double adaptation or moves.
