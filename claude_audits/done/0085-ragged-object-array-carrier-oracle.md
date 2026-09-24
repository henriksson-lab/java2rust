# 0085 — Ragged object-array carrier test rejected unrelated `borrow_mut`

Status: **fixed as an oracle defect** — ragged object arrays remain nested
vectors and their generated crate compiles.

## Finding

The translator already emitted `Vec<Vec<Option<File>>>`, length-preserving empty
rows, and ordinary indexed row replacement. The test attempted to reject
ndarray row mutation using `!rust.contains("row_mut")`; generated runtime support
contains the unrelated method call `borrow_mut()`, so the substring assertion
failed before the generated-crate check ran.

The negative assertion now checks the actual ndarray method token
`.row_mut(`. This retains the representation invariant without matching an
unrelated identifier suffix.

## Verification

`ragged_object_two_dimensional_arrays_stay_nested_vecs` passes and compiles its
generated crate. Neighboring ragged and ndarray assignment-order tests remain
green.
