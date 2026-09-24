# 0083 — Ndarray parameter ownership was applied twice

Status: **fixed** — borrowed rectangular primitive arrays now render as
`&ndarray::Array2<T>` or `&mut ndarray::Array2<T>` exactly once.

## Root cause

The representation-aware `slot_render` branch for a slice-shaped 2D primitive
array recursively rendered the complete slot, including its ownership. The
outer renderer then applied the same ownership again. Readonly parameters
became `&&Array2<T>` and mutable parameters became `&mut &mut Array2<T>`.

The branch now renders only the ndarray value carrier and leaves ownership to
the renderer's single common ownership step. A direct unit test pins shared and
mutable output types.

## Verification

Generated-crate tests cover side-effecting indices, ordinary 2D construction
and access, same-class static calls, row replacement, and overloaded throwing
helpers. All pass with single-reference signatures.
