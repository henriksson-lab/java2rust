# 0093 — interface_type_arguments oracle predates scoped type-parameter normalization

## Symptom

`representation_lowering::type_emission_tests::interface_arguments_resolve_in_the_edge_owner_without_capturing_type_parameters`
fails its second assertion:

```
left:  Some([Named { name: "__JavaTypeParam_526f6f742f706172616d657465723a300054_0", args: [] }])
right: Some([Named { name: "T", args: [] }])
```

## Evidence

The probe program declares a nominal `class T {}` and
`class Root<T> implements RootSource<T> {}`. The scoped-type-parameter
normalizer (src/scoped_type_parameters.rs) renames any binder whose name
collides with a nominal declaration, because name-based resolution of plain
`T` inside the program would capture `class T`. The hex payload decodes to
`Root/parameter:0\0T`: the returned name is exactly Root's own (renamed) type
parameter 0, i.e. the argument resolves to the edge owner's binder — the
behavior the test's name demands ("without capturing type parameters").

Returning plain `T` would be the capture bug the normalizer exists to prevent,
so the translator output is correct and the literal `"T"` expectation predates
normalization.

## Classification

Stale-oracle correction, same category as 0087/0088/0091. No translator change.

## Fix

Assert against `root.type_params[0]` (the owner's declared, possibly renamed,
binder) instead of the pre-normalization literal `"T"`. This keeps the gate's
semantic content — interface arguments resolve to the edge owner's own type
parameter, not the colliding nominal class — while being robust to the
injective renaming scheme.
