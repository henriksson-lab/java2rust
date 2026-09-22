# 0076 — `Enum.values()` became a nested vector and borrowed enum loop value

Status: **fixed** — `Arrays.asList(Enum.values())` exposes the enum elements,
set construction targets the declared set carrier, and enhanced-for copies the
copy-valued enum elements.

## Root cause

Three lowering decisions interacted:

1. generic varargs lowering wrapped the already-array-valued `values()` result
   in `vec![...]`, producing `Vec<Vec<Enum>>`;
2. nested source-enum lookup did not resolve a qualified nested type path, so
   enhanced-for fell back to `.iter()` and bound `&Enum` rather than `Enum`;
3. without the slot-aware `Arrays.asList` path, the concrete
   `LinkedHashSet` carrier leaked through a method whose declared `Set` ABI is
   represented as `HashSet`.

Nested-path type lookup now participates in source-enum recognition. That
routes the call through the existing object-array-aware `Arrays.asList` path,
and the set-slot constructor collects the flattened iterator into the declared
carrier.

## Verification

`source_enum_values_spread_into_arrays_as_list_and_compare_borrowed_loop_items`
asserts the flattened `HashSet` collection and copied enum iteration, then
compiles the complete generated crate.
