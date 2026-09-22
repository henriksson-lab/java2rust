# 0080 — Nullable array ABI shaping disagreed with solved slot ownership

Status: **fixed** — nullable borrowed arrays retain their solved Rust borrow
shape, while freshly allocated caller arrays remain owned storage.

## Root causes

`slot_render` converted every optional slice-shaped array element to `Vec<T>`,
including `SharedBorrow` and `MutableBorrow` slots. Lowering still produced
borrowed slices, so declarations such as `Option<&[T]>` and
`Option<&mut [T]>` were rendered as `Option<Vec<T>>` and generated crates did
not compile.

Separately, signature propagation treated a callee parameter's nullable input
contract as a storage fact about the caller's array binding. Passing a freshly
allocated `T[]` to a nullable parameter therefore changed the caller local from
`Vec<T>` to `Option<Vec<T>>`. Nullability propagation is now suppressed only
for caller array bindings; non-array reference propagation remains intact.

Optional ndarray `.length` also retained an array-slice projection before
emitting `.nrows()`. The Array2 path now strips that incompatible projection
before applying the Java null-receiver check.

## Verification

The focused ABI, readonly-slice, owned-output, and optional-array receiver tests
all compile their generated crates. Unit tests pin borrowed optional rendering
and the fresh-caller-array nullability boundary. The complete `idiom_gaps`
census moved from 787/163 to 797/153 with no new failure names.
