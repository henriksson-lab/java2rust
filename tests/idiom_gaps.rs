use std::fs;
use std::path::Path;

use new_java2rust::translate_source;

fn idiom_gap(path: &str) -> String {
    fs::read_to_string(Path::new("examples/idiom-gaps").join(path)).unwrap()
}

#[test]
fn documents_gatk_acronym_identifier_splitting_gap() {
    let translation = translate_source(&idiom_gap("AcronymIdentifierNames.java"));

    assert!(
        translation.rust.contains("g_a_t_k_v_c_f_constants"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("plugin_package_names"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_java_string_method_spelling_gap() {
    let translation = translate_source(&idiom_gap("JavaStringMethods.java"));

    assert!(
        translation.rust.contains(".to_lower_case()"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains(".equals("),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains(".length()"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_string_format_static_call_gap() {
    let translation = translate_source(&idiom_gap("StringFormatStaticCall.java"));

    assert!(
        translation.rust.contains("string.format"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("format!(\"{}:{}\""),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_to_array_empty_string_array_gap() {
    let translation = translate_source(&idiom_gap("ToArrayEmptyStringArray.java"));

    assert!(
        translation.rust.contains("Java array creation"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("Vec::new()"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_get_name_getter_no_longer_requires_runtime_downcast() {
    let translation = translate_source(&idiom_gap("TypedGetterAvoidsDowncast.java"));

    assert!(
        !translation.rust.contains("downcast::<String>()"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("expect(\"invalid Java cast\")"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("let name: String = box.get_name();"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_nullable_concrete_object_gap() {
    let translation = translate_source(&idiom_gap("NullableParameterAndLocal.java"));

    assert!(translation.rust.contains("None"), "{}", translation.rust);
    assert!(
        !translation.rust.contains("Option<"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_proven_non_null_member_field_gap() {
    let translation = translate_source(&idiom_gap("NonNullFieldFromConstructors.java"));

    assert!(
        translation.rust.contains("name: String") && !translation.rust.contains("Option<String>"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_java_collection_api_gap() {
    let translation = translate_source(&idiom_gap("CollectionAddAllAndSize.java"));

    assert!(
        translation.rust.contains(".add_all("),
        "{}",
        translation.rust
    );
    assert!(translation.rust.contains(".size()"), "{}", translation.rust);
}

#[test]
fn documents_external_primitive_getter_dispatch_coercion_gap() {
    let translation = translate_source(&idiom_gap("PrimitiveGetterDispatchCoercion.java"));

    assert!(
        translation.rust.contains("get_read_length()")
            && translation
                .rust
                .contains("unsupported Java dispatch coercion: target type Primitive(Int)"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_boolean_getter_no_longer_requires_runtime_downcast() {
    let translation = translate_source(&idiom_gap("BooleanGetterDispatchCoercion.java"));

    assert!(
        translation
            .rust
            .contains("let value: bool = read.is_paired();")
            && !translation
                .rust
                .contains("unsupported Java dispatch coercion"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_external_byte_array_getter_dispatch_coercion_gap() {
    let translation = translate_source(&idiom_gap("ByteArrayGetterDispatchCoercion.java"));

    assert!(
        translation.rust.contains("get_bases_no_copy()")
            && translation
                .rust
                .contains("unsupported Java dispatch coercion: target type Array(Primitive(Byte))"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_java_bean_getter_setter_naming_gap() {
    let translation = translate_source(&idiom_gap("JavaBeanGetterSetterNames.java"));

    assert!(
        translation.rust.contains("pub fn get_value")
            && translation.rust.contains("pub fn set_value")
            && translation.rust.contains("pub fn copy_from(&mut self")
            && translation.rust.contains("self.value = other.value")
            && translation.rust.contains("return self.value;")
            && !translation.rust.contains("self.get_value()")
            && !translation.rust.contains("self.set_value("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_collection_remove_if_lambda_gap() {
    let translation = translate_source(&idiom_gap("CollectionRemoveIfLambda.java"));

    assert!(
        translation.rust.contains(".remove_if(")
            && translation
                .rust
                .contains("unsupported Java expression: lambda"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_for_each_lambda_side_effect_gap() {
    let translation = translate_source(&idiom_gap("ForEachLambdaSideEffect.java"));

    assert!(
        translation.rust.contains(".for_each(")
            && translation
                .rust
                .contains("unsupported Java expression: lambda"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_static_owner_and_utility_call_gap() {
    let translation = translate_source(&idiom_gap("StaticConstantOwner.java"));

    assert!(
        translation.rust.contains("owner.CONSTANT"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("math.max("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_primitive_array_allocation_gap() {
    let translation = translate_source(&idiom_gap("PrimitiveArrayWithComputedLength.java"));

    assert!(
        translation.rust.contains("Java array creation"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("Vec::new()"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("arrays.fill("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_string_array_literal_argument_gap() {
    let translation = translate_source(&idiom_gap("StringArrayLiteralArgument.java"));

    assert!(
        translation.rust.contains("Java array creation: String[]")
            && translation.rust.contains("Vec::new()"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_map_put_get_java_api_gap() {
    let translation = translate_source(&idiom_gap("MapPutGetJavaApi.java"));

    assert!(
        translation.rust.contains(".put(") && translation.rust.contains(".get("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_method_reference_constructor_gap() {
    let translation = translate_source(&idiom_gap("MethodReferenceConstructor.java"));

    assert!(
        translation
            .rust
            .contains("unsupported Java expression: method reference"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_method_reference_flat_map_gap() {
    let translation = translate_source(&idiom_gap("MethodReferenceFlatMap.java"));

    assert!(
        translation
            .rust
            .contains("unsupported Java expression: method reference"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_stream_callback_chain_gap() {
    let translation = translate_source(&idiom_gap("StreamCallbackChain.java"));

    assert!(
        translation
            .rust
            .contains("unsupported Java expression: method reference")
            || translation
                .rust
                .contains("unsupported Java expression: lambda"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_class_literal_logger_gap() {
    let translation = translate_source(&idiom_gap("ClassLiteralLogger.java"));

    assert!(
        translation.rust.contains("logger: Logger") && !translation.rust.contains("LogManager"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_class_literal_registration_gap() {
    let translation = translate_source(&idiom_gap("ClassLiteralRegistration.java"));

    assert!(
        translation
            .rust
            .contains("unsupported Java expression: class literal"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_instanceof_guard_narrowing_gap() {
    let translation = translate_source(&idiom_gap("InstanceofGuardNarrowsCast.java"));

    assert!(
        translation
            .rust
            .contains("unsupported Java instanceof shape")
            || translation.rust.contains("downcast::"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_super_constructor_state_gap() {
    let translation = translate_source(&idiom_gap("SuperConstructorState.java"));

    assert!(
        translation
            .rust
            .contains("super(...) requires inheritance lowering"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_constructor_delegation_with_extra_work_gap() {
    let translation = translate_source(&idiom_gap("ConstructorDelegationWithExtraWork.java"));

    assert!(
        translation
            .rust
            .contains("this(...) followed by additional statements"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_unresolved_constructor_delegation_gap() {
    let translation = translate_source(&idiom_gap("UnresolvedConstructorDelegation.java"));

    assert!(
        translation
            .rust
            .contains("unresolved Java constructor delegation"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_enum_name_value_of_java_api_gap() {
    let translation = translate_source(&idiom_gap("EnumNameAndValueOf.java"));

    assert!(
        translation.rust.contains("mode.value_of(") || translation.rust.contains(".name()"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_optional_get_is_present_java_api_gap() {
    let translation = translate_source(&idiom_gap("OptionalGetIsPresent.java"));

    assert!(
        translation.rust.contains(".is_present()") && translation.rust.contains(".get()"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_try_with_resources_copy_gap() {
    let translation = translate_source(&idiom_gap("TryWithResourcesCopy.java"));

    assert!(
        translation.rust.contains("pub fn copy")
            && !translation.rust.contains("new_input_stream")
            && !translation.rust.contains("write("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_switch_state_machine_gap() {
    let translation = translate_source(&idiom_gap("SwitchReturnsEnumMatch.java"));

    assert!(
        translation.rust.contains("__java_switch_matched"),
        "{}",
        translation.rust
    );
    assert!(!translation.rust.contains("match "), "{}", translation.rust);
}

#[test]
fn documents_diamond_single_unknown_gap() {
    let translation = translate_source(&idiom_gap("DiamondSingleUnknown.java"));

    assert!(
        translation
            .rust
            .contains("let out: Vec<String> = ArrayList::new();")
            && !translation.rust.contains("Vec::new()"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_diamond_nested_unknowns_gap() {
    let translation = translate_source(&idiom_gap("DiamondNestedUnknowns.java"));

    assert!(
        translation.rust.contains("let out: Map = HashMap::new();")
            && translation.rust.contains("ArrayList::new(values)")
            && !translation.rust.contains("HashMap<"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_class_literal_token_gap() {
    let translation = translate_source(&idiom_gap("ClassLiteralToken.java"));

    assert!(
        translation
            .rust
            .contains("unsupported Java expression: class literal")
            || translation.rust.contains(".get_class()"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_equals_and_objects_gap() {
    let translation = translate_source(&idiom_gap("EqualsAndObjects.java"));

    assert!(
        translation.rust.contains(".equals(") || translation.rust.contains("objects.equals("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_serializable_value_object_gap() {
    let translation = translate_source(&idiom_gap("SerializableValueObject.java"));

    assert!(
        translation.rust.contains("serial_version_u_i_d")
            || translation.rust.contains(".write_object(")
            || translation.rust.contains(".read_object("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_comparator_interface_gap() {
    let translation = translate_source(&idiom_gap("ComparatorInterface.java"));

    assert!(
        translation.rust.contains("collections.sort(")
            || translation.rust.contains("integer.compare(")
            || translation.rust.contains(".compare_to("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_core_functional_interfaces_gap() {
    let translation = translate_source(&idiom_gap("CoreFunctionalInterfaces.java"));

    assert!(
        translation.rust.contains(".test(")
            || translation.rust.contains(".apply(")
            || translation.rust.contains(".get()"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_mutual_recursion_gap() {
    let translation = translate_source(&idiom_gap("MutualRecursion.java"));

    assert!(
        translation.rust.contains("self.odd(") && translation.rust.contains("self.even("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_tail_recursive_accumulator_gap() {
    let translation = translate_source(&idiom_gap("TailRecursiveAccumulator.java"));

    assert!(
        translation.rust.contains("self.factorial_loop(") && !translation.rust.contains("loop {"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_while_assignment_not_null_gap() {
    let translation = translate_source(&idiom_gap("WhileAssignmentNotNull.java"));

    assert!(
        translation.rust.contains("while")
            && translation.rust.contains("cursor.next()")
            && translation.rust.contains("(row = cursor.next())")
            && !translation.rust.contains("while let"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_while_assignment_null_variants_gap() {
    let translation = translate_source(&idiom_gap("WhileAssignmentNullVariants.java"));

    assert!(
        translation.rust.contains("cursor.read()")
            && translation.rust.contains("cursor.advance()")
            && translation.rust.contains("(row = cursor.read())")
            && !translation.rust.contains("while let"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_external_getter_setter_mutability_gap() {
    let translation = translate_source(&idiom_gap("ExternalGetterSetterMutability.java"));

    assert!(
        translation.rust.contains("counter.get_value()")
            && translation.rust.contains("counter.set_value(")
            && translation.rust.contains("counter: &mut ExternalCounter"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_external_fluent_mutation_gap() {
    let translation = translate_source(&idiom_gap("ExternalFluentMutation.java"));

    assert!(
        translation.rust.contains("request.set_name(")
            && translation.rust.contains(".set_limit(")
            && translation.rust.contains(".enable_retries("),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_external_indexed_accessors_gap() {
    let translation = translate_source(&idiom_gap("ExternalIndexedAccessors.java"));

    assert!(
        translation.rust.contains("table.get(row, column)")
            && translation.rust.contains("table.set(row, column, value)")
            && translation.rust.contains("table: &mut ExternalTable"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_exception_subclass_catch_order_gap() {
    let translation = translate_source(&idiom_gap("ExceptionSubclassCatchOrder.java"));

    assert!(
        translation
            .rust
            .contains("unsupported Java statement: try/catch/finally")
            && translation.rust.contains("BaseProblem")
            && translation.rust.contains("SpecificProblem"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_exception_base_catch_covers_subclass_gap() {
    let translation = translate_source(&idiom_gap("ExceptionBaseCatchCoversSubclass.java"));

    assert!(
        translation
            .rust
            .contains("unsupported Java statement: try/catch/finally")
            && translation.rust.contains("RecoverableProblem")
            && translation.rust.contains("MissingFileProblem"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_exception_cause_chaining_gap() {
    let translation = translate_source(&idiom_gap("ExceptionCauseChaining.java"));

    assert!(
        translation
            .rust
            .contains("unsupported Java statement: try/catch/finally")
            && translation.rust.contains("Result<(), JavaException>")
            && translation.rust.contains("UserProblem"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_exception_rethrow_specific_gap() {
    let translation = translate_source(&idiom_gap("ExceptionRethrowSpecific.java"));

    assert!(
        translation
            .rust
            .contains("unsupported Java statement: try/catch/finally")
            && translation.rust.contains("Result<(), JavaException>")
            && translation.rust.contains("ValidationProblem"),
        "{}",
        translation.rust
    );
}

#[test]
fn documents_exception_runtime_checked_hierarchy_gap() {
    let translation = translate_source(&idiom_gap("ExceptionRuntimeCheckedHierarchy.java"));

    assert!(
        translation
            .rust
            .contains("unsupported Java statement: try/catch/finally")
            && translation.rust.contains("CheckedProblem")
            && translation.rust.contains("UncheckedProblem"),
        "{}",
        translation.rust
    );
}
