# 0086 — Constructor-argument rectangular array misses the from_rows adapter

Status: **fixed** — `ndarray_array2_element_type_rows_nullable` unwraps the Optional row layer of partially specified `T[][]` storage before the rectangular check. Lib suite: 1842 passed / 12 failed (one gate closed, no new failures).

## Finding

Gate: `lower::tests::constructor_rectangular_array_slot_reuses_nested_rows_adapter`
(fails at the vendored revision; one of 13 named failures in the lib baseline
of 1841 passed / 13 failed, measured 2026-11-13 with `cargo test --release`).

The probe builds a `double[][]` local with partial allocation
(`new double[rows][]`), fills every row in a loop, and hands the array to a
constructor taking `double[][]`. Expected: the constructor argument
materializes through `ndarray::Array2::from_rows`. Observed: the local keeps
the nullable nested carrier `Rc<RefCell<Vec<Option<Vec<f64>>>>>` and the call
is emitted as `MatrixSink::new(&values)` with no adapter.

## Trace

The projection exists: `lower_nested_array2_argument_for_rectangular_slot`
(`src/lower.rs`, near the 50339 line region) handles exactly this handoff,
including nullable rows (`rows_are_nullable` selects the `.iter().map(...)`
adapter in `array2_from_nested_rows_projection`). It is consulted from the
argument-coercion chain (near 49816) and from one other call path (53250).

Two candidate breakpoints, in probe order:

1. The constructor-call argument path may not reach that coercion chain at
   all. Method calls and `new` expressions do not obviously share one
   argument lowering; `constructor_plan.rs` may lower constructor arguments
   separately. A one-line probe with a plain static method taking
   `double[][]` distinguishes this: if the method call gets `from_rows` and
   the constructor does not, the gap is the constructor path, not the gate.
2. The gate `two_dimensional_array_expr_uses_nested_slice` (95153 region) may
   return false for the local: `env.local_uses_nested_array2_slice` covers
   the slice view, and the fallback consults the *solved storage shape* for
   `Array<Optional<Array<T>>>`. If the solver records the declared
   `double[][]` rather than the partial-allocation provenance for this
   local, the gate never fires.

## Probe result (2026-11-13)

Hypothesis 1 is rejected. The probe ran both call kinds with the fill loop in
the caller: `new Sink(values)` and a static `viaMethod(values)` taking
`double[][]`. Neither applies the adapter — both emit the raw nullable
carrier (`Sink::new(&values)` / `via_method(clone(&values))`). The gap is
therefore in the gate, not the constructor path: for a local allocated with
`new double[rows][]` and row-filled, neither
`env.local_uses_nested_array2_slice` nor the solved-storage
`Array<Optional<Array<T>>>` fallback recognizes the nested carrier, so
`lower_nested_array2_argument_for_rectangular_slot` returns before its
nullable-rows handling can run.

## Verification plan

Instrument the gate to report which branch decides. Fix only after the failing branch is identified; per the
entry-gate rule, no generated-output patch counts as a fix.
