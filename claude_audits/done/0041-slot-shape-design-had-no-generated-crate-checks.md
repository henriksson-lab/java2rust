# slot_shape_design: generated-crate check coverage

Recorded 2026-08-06.

`cargo_check_generated_modules` lived only in `tests/idiom_gaps.rs` (296 call
sites). `slot_shape_design` had **none**, so 596 tests could assert on rendered
Rust without anything checking that the Rust compiled. That is
[0018](../0018-the-compile-error-metric-cannot-see-silent-failures.md)'s blind spot
inside the suite itself, and it hid a real defect: an `Option<Rc<Cursor>>` field
against an `Option<Cursor>` local read as an assertion failure.

The helper now lives in `tests/common/mod.rs`. Of 151 translation tests that
assert on `translation.rust`:

| | count |
|---|---:|
| check added, output compiles | **99** |
| output does not compile -- genuine | **21** |
| fails only because the check crate does not link the fixture's deps | **31** |

The 99 are checked now, so anything that breaks their output is caught. The other
52 are listed below rather than checked, because turning them on would make the
suite red for two different reasons that must not be conflated.

## Genuine: the generated Rust does not compile

Nothing measured these. Error kinds: 9x `E0599` (no method), 4x `E0308`
(mismatched types), 4x `E0282` (needs annotations), plus `E0592`, `E0423`,
`E0277`, `E0034`.

- `borrowed_self_field_owned_recovery_applies_to_unshaped_external_arguments`
- `class_for_name_uses_java_class_token_support`
- `collections_sort_comparator_marks_interface_declared_local_vec_mutable`
- `collections_sort_comparator_marks_local_vec_mutable`
- `collections_sort_comparator_statement_lowers_to_sort_by`
- `constructor_assigned_local_keeps_owned_storage_despite_borrowed_external_uses`
- `external_builder_build_without_inferable_product_uses_named_unmodelled_result_shim`
- `external_builder_getter_without_return_fact_uses_named_unmodelled_result_shim`
- `external_enum_constant_facades_do_not_emit_recursive_storage_fields`
- `external_getter_without_return_fact_uses_named_unmodelled_result_shim`
- `external_static_facade_overloads_use_arity_specific_support_methods`
- `imported_static_getter_without_return_fact_uses_named_unmodelled_result_shim`
- `inherited_external_getter_without_return_fact_uses_named_unmodelled_result_shim`
- `lowering_passes_facts_to_item_assembly_for_function_shells`
- `non_constant_static_final_initializers_stay_unmodelled`
- `nullable_object_slot_wraps_concrete_return_after_erasure_once`
- `parsed_source_peer_params_and_exact_cast_locals_stay_concrete_under_carrier_pressure`
- `reused_local_recovery_applies_inside_fluent_external_builder_chains`
- `reused_local_recovery_applies_to_unshaped_external_arguments`
- `reused_local_recovery_uses_overload_specific_usage_facts`
- `summary_source_overloads_get_one_rust_name_from_every_path`

## Harness-only: fixture pulls a crate the check does not link

These are **not** translator defects. The generated crate references `ndarray`,
`hdf5_facade` and similar, which the nested `cargo check` does not have as
dependencies -- the same limitation `tests/differential/ObjectArray2D.java`
records for `int[][]`. Turning the check on for these would need the harness to
link the facade crates, which is a separate piece of work.

- `array_initializer_argument_uses_sequence_view_from_external_argument_fact`
- `borrowed_path_facade_return_recovers_owned_path_before_erasure`
- `crate_mapped_external_constructors_render_through_facade_target`
- `crate_mapped_external_method_calls_render_through_facade_target`
- `external_borrowed_literal_argument_is_hoisted_when_call_result_is_stored`
- `external_byte_array_return_recovers_java_signed_bytes_from_rust_unsigned_bytes`
- `external_collection_like_facade_no_alias_arg_clones_when_reused`
- `external_conversion_adapter_wraps_array_initializer_elements`
- `external_crate_mapped_type_uses_pack_rust_type_name_for_returns`
- `external_crate_static_call_without_import_uses_mapped_owner_type`
- `external_facade_support_does_not_require_default_for_crate_return_stubs`
- `external_facade_support_lowers_raw_collection_returns_to_erased_rust_shapes`
- `external_facade_support_uses_crate_mapped_return_type_policy`
- `external_facade_support_uses_exact_rust_return_facts_for_stub_signatures`
- `external_facade_support_uses_rust_acronym_type_name_for_crate_mapped_returns`
- `external_facade_support_uses_structured_abi_for_stub_return_signatures`
- `external_generic_facade_return_local_uses_inferred_rust_type`
- `external_nullable_owned_option_return_does_not_clone_option_value`
- `external_rust_scalar_return_casts_to_java_primitive_slot`
- `facade_borrowed_self_receiver_uses_receiver_slot_conversion`
- `fact_backed_typed_external_initializer_anchors_declared_local_shape`
- `ndarray_backed_two_dimensional_assignment_keeps_its_own_ordered_path`
- `non_null_external_object_initializer_beats_later_null_check`
- `optional_external_mutating_receiver_call_uses_mutable_unwrap`
- `optional_external_receiver_call_uses_facade_mapping_after_unwrap`
- `parsed_crate_mapping_facade_names_are_facts_not_java_snake_case`
- `rust_result_external_constructor_uses_context_result_policy`
- `rust_result_metadata_separates_facade_fallibility_from_java_throws`
- `sqlite_crate_mapped_external_constructors_render_through_facade_target`
- `sqlite_crate_mapped_external_method_calls_render_through_facade_target`
- `throwing_external_call_initializer_uses_slot_result_propagation`

## Why this ordering

Adding the check to the 99 keeps the gate exactly where it was (516/80) while
removing the blind spot for everything that can be checked today. The 21 genuine
failures are a real backlog and should be worked down; the 31 need harness
support first. Doing all three at once would have moved the baseline by 52 and
made it impossible to tell which kind of problem any future change caused.

Relevant to [0002b](../0002b-shared-representation-design.md): its remaining step
needs exactly this signal. Every previous attempt converged against assertion
text rather than against the compiler, which is why unwrapped slot-rendering
paths kept surfacing one at a time.
