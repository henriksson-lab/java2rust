# 0092 — Shared-handle array parameters spelled `mut` on element writes only

## Symptom

`lower::tests::shared_self_field_mutable_argument_clones_array_handle` fails its
first assertion: the generated signature is

```rust
pub fn fill(mut output: std::rc::Rc<std::cell::RefCell<Vec<JavaCodeUnit>>>)
```

while the gate expects the non-`mut` binding. The body additionally emits a
redundant `let mut output = output;` shadow.

## Evidence

Java source (`fill(char[] output) { output[0] = 'x'; }`): the parameter is
never rebound; only an element is written.

Constraint collection (src/signature.rs, param constraint block near
`array_element_mutated_names` and the `collect_mutated_local_names` fallback)
pushes `SignatureConstraint::MutableBinding` for both whole-variable rebinding
and array-element writes, because `collect_assignment_target_name` roots an
`Expr::ArrayAccess` target at the array variable. Separately, shared-storage
solving upgrades the parameter to an owned shared handle
(`Rc<RefCell<Vec<_>>>`; see the assigned-shared-array override in
src/lower.rs near `solved_shape.native_allocation.shared()`), so all element
mutation flows through `RefCell::borrow_mut()` on the handle.

Emission (src/item_assembly.rs `method_params_with_optional_lifetime`) then
prefixes `mut ` whenever `mutable_array_target_params` contains the name (set
from element writes) or `binding_mutable()` is set on an owned shape — without
checking whether the solved representation is a shared handle that already
provides interior mutability.

## Classification

Local emission fix. The `mut` prefix decision must consider the solved
representation: for shared native-allocation handles, only rebinding the
parameter binding itself (whole-variable assignment) requires `mut`;
element/field writes do not. The caller-side contract is unaffected: callers
already clone the handle (`<Rc<_> as Clone>::clone(&...)`), which the remaining
assertions of this gate verify.

## Fix

In `method_params_with_optional_lifetime`, compute the set of parameters that
are rebound as whole variables in the body and suppress the `mut` prefix when
the solved representation provides interior mutability — a shared native
allocation handle, or a source-hierarchy carrier whose variants are already
Rc/Arc-backed (`source_hierarchy_carrier_uses_only_interior_mutable_handles`,
which also closes `source_hierarchy_carrier_field_write_uses_storage_independent_setter`,
where `root.value = value` on a `RootValue` carrier parameter spelled
`mut root`) — and the parameter is not rebound. Rebinding still yields
`mut output` so `output = new_handle` remains compilable. The
`prepend_assigned_parameter_shadows` body shadow is harmless and left
untouched; these gates do not assert on it.
