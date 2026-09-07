use crate::call_plan::{
    CallTargetPlan, NativeCollectionMethod, NativeCollectionNewTarget,
    NativeCollectionStaticMethod, NativeJavaInstanceMethod, NativeJavaStaticMethod,
    NativeStringNewTarget, NewTargetPlan,
};
use crate::coercion::{
    IndexToUsizeProjection, JavaDisplayPlan, apply_java_display_plan, arrays_as_list_projection,
    arrays_binary_search_projection, arrays_binary_search_range_projection,
    arrays_compare_projection, arrays_compare_range_projection, arrays_copy_of_projection,
    arrays_copy_of_range_projection, arrays_deep_equals_projection,
    arrays_deep_hash_code_projection, arrays_deep_to_string_projection, arrays_equals_projection,
    arrays_equals_range_projection, arrays_hash_code_projection, arrays_mismatch_projection,
    arrays_mismatch_range_projection, arrays_to_string_projection, collection_remove_if_projection,
    collections_empty_list_projection, collections_empty_map_projection,
    collections_empty_set_projection, collections_empty_sorted_set_projection,
    collections_n_copies_projection, collections_singleton_list_projection,
    collections_singleton_map_projection, collections_singleton_projection,
    collections_unmodifiable_projection, discard_value_projection, erased_object_box_projection,
    erased_object_constructor_projection, external_crate_associated_path_projection,
    external_crate_path_projection, external_facade_rust_path_projection, facade_method_name,
    field_projection, file_exists_projection, free_function_path_projection,
    function_call_projection, index_to_usize_projection, java_class_literal_path_projection,
    java_class_static_path_projection, java_integral_parse_null_argument_as_number_format,
    java_iterator_from_collection_projection, java_list_hash_code_projection,
    java_map_hash_code_projection, java_plain_object_constructor_projection,
    java_set_hash_code_projection, java_value_hash_code_projection, len_i32_projection,
    math_abs_projection, math_floor_div_projection, math_floor_mod_projection, math_max_projection,
    math_min_projection, math_pow_projection, math_round_projection, math_to_int_exact_projection,
    native_collection_default_constructor_projection, native_collection_empty_new_projection,
    native_collection_from_source_projection, native_collection_with_capacity_projection,
    objects_equals_projection, objects_is_null_projection, objects_non_null_projection,
    optional_get_owned_value_result_projection, optional_of_projection,
    optional_or_else_owned_value_projection, owned_collection_to_vec_projection_for_receiver_type,
    owned_map_keys_vec_projection, owned_map_values_vec_projection,
    owner_associated_path_projection, primitive_compare_to_projection,
    primitive_f64_binary_method_projection, primitive_f64_method_projection,
    primitive_hash_code_projection, primitive_is_nan_projection, primitive_number_value_projection,
    primitive_parse_projection, primitive_parse_radix_projection, receiver_method_call_projection,
    receiver_method_call_projection_exact, rust_string_literal_projection,
    rust_type_associated_path_projection, self_associated_path_projection,
    sequence_index_of_projection, string_char_at_result_projection,
    string_compare_to_ignore_case_projection, string_compare_to_projection,
    string_equals_ignore_case_projection, string_format_projection,
    string_from_byte_range_projection, string_from_bytes_projection, string_get_bytes_projection,
    string_hash_code_projection, string_index_of_projection, string_join_collection_projection,
    string_join_varargs_projection, string_last_index_of_projection,
    string_literal_from_rust_char_literal_expr, string_matches_projection,
    string_replace_all_projection, string_replace_first_projection, string_split_projection,
    string_substring_projection, string_to_char_array_projection,
    strip_redundant_java_null_expect_after_unwrap, system_exit_projection, thread_sleep_projection,
    unwrap_result_or_panic_uncaught_java_exception,
};
use crate::facts::FactSet;
use crate::facts::ReceiverAbi;
use crate::java_ir::{Expr, JavaType, LambdaBody, PrimitiveType};
use crate::output_markers::noop_facade_method_call_site_marker;
use crate::representation::RepresentationPlan;
use crate::rust_ir::{RustClosureBody, RustClosureParam, RustExpr, RustStmt, RustType};
use crate::shape_projection::shape_is_erased_object_boundary;
use crate::slot_render::{rust_type_for_java_type_with_facts, rust_type_for_slot_with_facts};
use crate::stream_lowering::{StreamLambdaValue, emit_stream_lambda_body_expr};

pub fn apply_call_target(
    target: &CallTargetPlan<'_>,
    lowered_receiver: Option<RustExpr>,
    args: Vec<RustExpr>,
) -> RustExpr {
    match target {
        CallTargetPlan::StaticSelf { rust_name } => {
            function_call_projection(self_associated_path_projection(rust_name), args)
        }
        CallTargetPlan::StaticOwner { owner, rust_name }
            if static_owner_simple_name(owner) == "String"
                && matches!(rust_name.as_str(), "valueOf" | "value_of")
                && args.len() == 1 =>
        {
            string_value_of_lowered_arg_to_string(
                args.into_iter()
                    .next()
                    .expect("String.valueOf target checked one argument"),
            )
        }
        CallTargetPlan::StaticOwner { owner, rust_name }
            if matches!(rust_name.as_str(), "valueOf" | "value_of")
                && args.len() == 1
                && owner
                    .rsplit("::")
                    .next()
                    .and_then(|simple| simple.chars().next())
                    .is_some_and(|ch| ch.is_ascii_lowercase()) =>
        {
            let owner = crate::call_plan::rust_static_owner_type_name(owner);
            function_call_projection(
                rust_type_associated_path_projection(
                    crate::rust_ir::RustType::Named(owner),
                    "value_of",
                ),
                args,
            )
        }
        CallTargetPlan::StaticOwner { owner, rust_name } => {
            function_call_projection(owner_associated_path_projection(owner, rust_name), args)
        }
        CallTargetPlan::InstanceReceiver { rust_name, .. } => {
            receiver_method_call_projection_exact(
                lowered_receiver.expect("receiver target must provide lowered receiver"),
                rust_name,
                args,
            )
        }
        CallTargetPlan::FallbackReceiver { rust_name, .. } => {
            receiver_method_call_projection_exact(
                lowered_receiver.expect("receiver target must provide lowered receiver"),
                rust_name,
                args,
            )
        }
        CallTargetPlan::InstanceSelf { rust_name } => receiver_method_call_projection_exact(
            RustExpr::Var("self".to_string()),
            rust_name,
            args,
        ),
        CallTargetPlan::FallbackSelf { rust_name } => receiver_method_call_projection_exact(
            RustExpr::Var("self".to_string()),
            rust_name,
            args,
        ),
        CallTargetPlan::FallbackFree { rust_name } => {
            function_call_projection(free_function_path_projection(rust_name), args)
        }
        CallTargetPlan::JavaClassStatic { rust_name } => {
            function_call_projection(java_class_static_path_projection(rust_name), args)
        }
        CallTargetPlan::NativeCollectionMethod {
            method,
            receiver_ty,
            ..
        } => apply_native_collection_method(
            lowered_receiver.expect("native collection target must provide receiver"),
            receiver_ty.as_ref(),
            *method,
            args,
        ),
        CallTargetPlan::NativeCollectionStatic { method } => {
            apply_native_collection_static_method(*method, args)
        }
        CallTargetPlan::NativeJavaInstance { method, .. } => apply_native_java_instance_method(
            lowered_receiver.expect("native Java instance target must provide receiver"),
            *method,
            args,
        ),
        CallTargetPlan::NativeJavaStatic { method } => {
            apply_native_java_static_method(*method, args)
        }
        CallTargetPlan::ExternalCrateReceiver {
            crate_name,
            rust_name,
            ..
        } => {
            let mut all_args = Vec::with_capacity(args.len() + 1);
            all_args.push(
                lowered_receiver.expect("external crate receiver target must provide receiver"),
            );
            all_args.extend(args);
            function_call_projection(
                external_crate_path_projection(crate_name, rust_name),
                all_args,
            )
        }
        CallTargetPlan::ExternalFacadeReceiver {
            rust_path,
            receiver_abi,
            noop_facade_method_call,
            receiver_type,
            method_name,
            arg_count,
            ..
        } => {
            let lowered = if *receiver_abi == ReceiverAbi::AssociatedFunction {
                function_call_projection(external_facade_rust_path_projection(rust_path), args)
            } else {
                apply_external_facade_receiver_call(
                    lowered_receiver.expect("external facade target must provide receiver"),
                    rust_path,
                    *receiver_abi,
                    args,
                )
            };
            if *noop_facade_method_call {
                mark_noop_facade_method_call_site(lowered, receiver_type, method_name, *arg_count)
            } else {
                lowered
            }
        }
        CallTargetPlan::SuperBoundary {
            declaring_type,
            rust_name,
            ..
        } => apply_super_boundary_call(declaring_type, rust_name, args),
    }
}

fn static_owner_simple_name(owner: &str) -> &str {
    owner
        .rsplit([':', '.', '$'])
        .find(|part| !part.is_empty())
        .unwrap_or(owner)
        .trim_start_matches('<')
        .trim_end_matches('>')
}

fn mark_noop_facade_method_call_site(
    expr: RustExpr,
    receiver_type: &str,
    method_name: &str,
    arg_count: usize,
) -> RustExpr {
    RustExpr::MarkedBlock {
        marker: noop_facade_method_call_site_marker(receiver_type, method_name, arg_count),
        expr: Box::new(expr),
    }
}

fn apply_external_facade_receiver_call(
    receiver: RustExpr,
    rust_path: &str,
    receiver_abi: ReceiverAbi,
    args: Vec<RustExpr>,
) -> RustExpr {
    if rust_path == "str::chars::nth" {
        let Some(index) = args.into_iter().next() else {
            return compile_error_call("Java String.charAt requires an index");
        };
        return unwrap_result_or_panic_uncaught_java_exception(string_char_at_result_projection(
            receiver, index,
        ));
    }
    match receiver_abi {
        ReceiverAbi::FreeFunction => {
            let mut all_args = Vec::with_capacity(args.len() + 1);
            all_args.push(receiver);
            all_args.extend(args);
            function_call_projection(external_facade_rust_path_projection(rust_path), all_args)
        }
        ReceiverAbi::AssociatedFunction => {
            function_call_projection(external_facade_rust_path_projection(rust_path), args)
        }
        ReceiverAbi::BorrowedSelf | ReceiverAbi::MutBorrowedSelf | ReceiverAbi::OwnedSelf => {
            receiver_method_call_projection_exact(receiver, &facade_method_name(rust_path), args)
        }
    }
}

fn apply_super_boundary_call(
    declaring_type: &str,
    rust_name: &str,
    args: Vec<RustExpr>,
) -> RustExpr {
    if rust_name == "read_3" && declaring_type.contains("InputStream") {
        return receiver_method_call_projection_exact(
            field_projection(RustExpr::Var("self".to_string()), "r#in"),
            "read_3",
            args,
        );
    }
    compile_error_call(&format!(
        "unsupported Java super call: {declaring_type}.{rust_name} requires class hierarchy lowering"
    ))
}

fn unit_expr() -> RustExpr {
    RustExpr::Tuple(Vec::new())
}

fn default_trait_expr() -> RustExpr {
    function_call_projection(RustExpr::path(["Default", "default"]), Vec::new())
}

fn compile_error_call(message: &str) -> RustExpr {
    crate::unsupported::compile_error_expr(message)
}

fn string_value_of_lowered_arg_to_string(value: RustExpr) -> RustExpr {
    apply_java_display_plan(
        string_value_of_lowered_arg_for_display(value),
        JavaDisplayPlan::FormatValue,
    )
}

fn string_value_of_lowered_arg_for_display(value: RustExpr) -> RustExpr {
    strip_terminal_java_null_receiver_expect_chain(value.clone())
        .map(|optional| apply_java_display_plan(optional, JavaDisplayPlan::OptionalString))
        .unwrap_or(value)
}

fn strip_terminal_java_null_receiver_expect_chain(expr: RustExpr) -> Option<RustExpr> {
    match expr {
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if method == "expect"
            && matches!(
                args.as_slice(),
                [RustExpr::Literal(message)] if message == "\"Java null receiver\""
            ) =>
        {
            let receiver = *receiver;
            Some(
                strip_terminal_java_null_receiver_expect_chain(receiver.clone())
                    .unwrap_or(receiver),
            )
        }
        RustExpr::Shaped { expr, .. } => strip_terminal_java_null_receiver_expect_chain(*expr),
        RustExpr::Grouped(expr) => strip_terminal_java_null_receiver_expect_chain(*expr),
        _ => None,
    }
}

fn math_arity_error(java_name: &str, expected: &str) -> RustExpr {
    compile_error_call(&format!("Math.{java_name} requires {expected}"))
}

fn apply_native_java_instance_method(
    receiver: RustExpr,
    method: NativeJavaInstanceMethod,
    args: Vec<RustExpr>,
) -> RustExpr {
    match method {
        NativeJavaInstanceMethod::StringLen => {
            len_i32_projection(strip_redundant_java_null_expect_after_unwrap(receiver))
        }
        NativeJavaInstanceMethod::StringIsEmpty => {
            receiver_method_call_projection(receiver, "is_empty", args)
        }
        NativeJavaInstanceMethod::StringTrim => receiver_method_call_projection(
            receiver_method_call_projection(receiver, "trim", args),
            "to_string",
            Vec::new(),
        ),
        NativeJavaInstanceMethod::StringToLowercase => {
            receiver_method_call_projection(receiver, "to_lowercase", args)
        }
        NativeJavaInstanceMethod::StringToUppercase => {
            receiver_method_call_projection(receiver, "to_uppercase", args)
        }
        NativeJavaInstanceMethod::StringToString => {
            receiver_method_call_projection(receiver, "to_string", args)
        }
        NativeJavaInstanceMethod::ObjectToString => {
            apply_java_display_plan(receiver, JavaDisplayPlan::ErasedObject)
        }
        NativeJavaInstanceMethod::SourceObjectToString => {
            receiver_method_call_projection(receiver, "to_string", args)
        }
        NativeJavaInstanceMethod::SourceHashCode => java_value_hash_code_projection(receiver),
        NativeJavaInstanceMethod::StringContains => {
            receiver_method_call_projection(receiver, "contains", args)
        }
        NativeJavaInstanceMethod::StringStartsWith => {
            receiver_method_call_projection(receiver, "starts_with", args)
        }
        NativeJavaInstanceMethod::StringEndsWith => {
            receiver_method_call_projection(receiver, "ends_with", args)
        }
        NativeJavaInstanceMethod::BitSetGet => {
            receiver_method_call_projection(receiver, "get", args)
        }
        NativeJavaInstanceMethod::FileExists => file_exists_projection(receiver),
        NativeJavaInstanceMethod::ReaderReady => RustExpr::Literal("true".to_string()),
        NativeJavaInstanceMethod::BufferPut => {
            let Some(arg) = args.into_iter().next() else {
                return unit_expr();
            };
            discard_value_projection(arg)
        }
        NativeJavaInstanceMethod::StringReplace => apply_string_replace(receiver, args),
        NativeJavaInstanceMethod::StringReplaceFirst => apply_string_replace_first(receiver, args),
        NativeJavaInstanceMethod::StringReplaceAll => apply_string_replace_all(receiver, args),
        NativeJavaInstanceMethod::StringMatches => apply_string_matches_helper(receiver, args),
        NativeJavaInstanceMethod::StringSplit => apply_string_split(receiver, args),
        NativeJavaInstanceMethod::StringSubstring => apply_string_substring(receiver, args),
        NativeJavaInstanceMethod::StringToCharArray => string_to_char_array_projection(receiver),
        NativeJavaInstanceMethod::StringHashCode => string_hash_code_projection(receiver),
        NativeJavaInstanceMethod::StringGetBytes => string_get_bytes_projection(receiver),
        NativeJavaInstanceMethod::StringIndexOf => apply_string_index_of(receiver, args),
        NativeJavaInstanceMethod::StringLastIndexOf => apply_string_last_index_of(receiver, args),
        NativeJavaInstanceMethod::StringCharAt => apply_string_char_at(receiver, args),
        NativeJavaInstanceMethod::StringCompareTo => apply_string_compare_to(receiver, args),
        NativeJavaInstanceMethod::StringCompareToIgnoreCase => {
            apply_string_compare_to_ignore_case(receiver, args)
        }
        NativeJavaInstanceMethod::StringEqualsIgnoreCase => {
            apply_string_equals_ignore_case(receiver, args)
        }
        NativeJavaInstanceMethod::OptionalIsPresent => {
            receiver_method_call_projection(receiver, "is_some", Vec::new())
        }
        NativeJavaInstanceMethod::OptionalGet => unwrap_result_or_panic_uncaught_java_exception(
            optional_get_owned_value_result_projection(receiver),
        ),
        NativeJavaInstanceMethod::OptionalOrElse => {
            let Some(fallback) = args.into_iter().next() else {
                return compile_error_call("Java Optional.orElse requires one argument");
            };
            optional_or_else_owned_value_projection(receiver, fallback)
        }
        NativeJavaInstanceMethod::PrimitiveNumberValue { source, target } => {
            apply_primitive_number_value(receiver, source, target)
        }
        NativeJavaInstanceMethod::PrimitiveHashCode { source } => {
            primitive_hash_code_projection(receiver, source)
        }
        NativeJavaInstanceMethod::PrimitiveCompareTo { source } => {
            apply_primitive_compare_to(receiver, args, source)
        }
        NativeJavaInstanceMethod::DecimalFormatFormat => {
            let Some(value) = args.into_iter().next() else {
                return compile_error_call("Java DecimalFormat.format requires one argument");
            };
            apply_java_display_plan(value, JavaDisplayPlan::FormatValue)
        }
        NativeJavaInstanceMethod::MessageFormatFormat => {
            let Some(value) = args.into_iter().next() else {
                return compile_error_call("Java MessageFormat.format requires one argument");
            };
            receiver_method_call_projection(receiver, "format", vec![value])
        }
        NativeJavaInstanceMethod::Equals => {
            let Some(other) = args.into_iter().next() else {
                return compile_error_call("Java equals requires one argument");
            };
            objects_equals_projection(receiver, other)
        }
    }
}

fn apply_primitive_number_value(
    receiver: RustExpr,
    source: Option<PrimitiveType>,
    target: PrimitiveType,
) -> RustExpr {
    if source == Some(target) {
        return receiver;
    }
    primitive_number_value_projection(receiver, target)
}

fn apply_primitive_compare_to(
    receiver: RustExpr,
    args: Vec<RustExpr>,
    _source: PrimitiveType,
) -> RustExpr {
    let Some(other) = args.into_iter().next() else {
        return compile_error_call("Java boxed primitive compareTo requires one argument");
    };
    primitive_compare_to_projection(receiver, other)
}

fn apply_string_char_at(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    let Some(index) = args.into_iter().next() else {
        return compile_error_call("Java String.charAt requires an index");
    };
    unwrap_result_or_panic_uncaught_java_exception(string_char_at_result_projection(
        receiver, index,
    ))
}

fn apply_string_index_of(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    let mut args = args.into_iter();
    let Some(needle) = args.next() else {
        return compile_error_call("Java String.indexOf requires at least one argument");
    };
    string_index_of_projection(receiver, needle, args.next())
}

fn apply_string_last_index_of(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    let mut args = args.into_iter();
    let Some(needle) = args.next() else {
        return compile_error_call("Java String.lastIndexOf requires at least one argument");
    };
    string_last_index_of_projection(receiver, needle, args.next())
}

fn apply_string_replace(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 2 {
        return compile_error_call("Java String.replace requires two arguments");
    }
    let mut args = args.into_iter();
    let pattern = args.next().unwrap();
    let replacement = string_replace_replacement_projection(args.next().unwrap());
    receiver_method_call_projection(receiver, "replace", vec![pattern, replacement])
}

fn string_replace_replacement_projection(replacement: RustExpr) -> RustExpr {
    if let Some(literal) = string_literal_from_rust_char_literal_expr(&replacement) {
        RustExpr::Literal(literal)
    } else {
        replacement
    }
}

fn apply_string_replace_first(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 2 {
        return compile_error_call("Java String.replaceFirst requires two arguments");
    }
    let mut args = args.into_iter();
    let regex = args.next().unwrap();
    let replacement = args.next().unwrap();
    string_replace_first_projection(receiver, regex, replacement)
}

fn apply_string_replace_all(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 2 {
        return compile_error_call("Java String.replaceAll requires two arguments");
    }
    let mut args = args.into_iter();
    let regex = args.next().unwrap();
    let replacement = args.next().unwrap();
    string_replace_all_projection(receiver, regex, replacement)
}

fn apply_string_matches_helper(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 1 {
        return compile_error_call("Java String.matches requires one argument");
    }
    string_matches_projection(receiver, args.into_iter().next().unwrap())
}

fn apply_string_substring(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    match args.len() {
        1 => {
            let mut args = args.into_iter();
            let begin = args.next().unwrap();
            let end = len_i32_projection(receiver.clone());
            string_substring_projection(receiver, begin, end)
        }
        2 => {
            let mut args = args.into_iter();
            let begin = args.next().unwrap();
            let end = args.next().unwrap();
            string_substring_projection(receiver, begin, end)
        }
        _ => compile_error_call("Java String.substring requires one or two arguments"),
    }
}

fn apply_string_compare_to(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    let Some(other) = args.into_iter().next() else {
        return compile_error_call("Java String.compareTo requires one argument");
    };
    string_compare_to_projection(receiver, other)
}

fn apply_string_compare_to_ignore_case(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    let Some(other) = args.into_iter().next() else {
        return compile_error_call("Java String.compareToIgnoreCase requires one argument");
    };
    string_compare_to_ignore_case_projection(receiver, other)
}

fn apply_string_equals_ignore_case(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    let Some(other) = args.into_iter().next() else {
        return compile_error_call("Java String.equalsIgnoreCase requires one argument");
    };
    string_equals_ignore_case_projection(receiver, other)
}

fn apply_string_split(receiver: RustExpr, mut args: Vec<RustExpr>) -> RustExpr {
    if args.is_empty() || args.len() > 2 {
        return compile_error_call("Java String.split requires one or two arguments");
    }
    let regex = args.remove(0);
    let limit = args
        .pop()
        .unwrap_or_else(|| RustExpr::Literal("0".to_string()));
    string_split_projection(receiver, regex, limit)
}

fn apply_native_java_static_method(
    method: NativeJavaStaticMethod,
    args: Vec<RustExpr>,
) -> RustExpr {
    match method {
        NativeJavaStaticMethod::MathMax { primitive } => {
            let mut args = args.into_iter();
            let Some(left) = args.next() else {
                return compile_error_call("Math.max requires two arguments");
            };
            let Some(right) = args.next() else {
                return compile_error_call("Math.max requires two arguments");
            };
            math_max_projection(left, right, primitive)
        }
        NativeJavaStaticMethod::MathMin { primitive } => {
            let mut args = args.into_iter();
            let Some(left) = args.next() else {
                return compile_error_call("Math.min requires two arguments");
            };
            let Some(right) = args.next() else {
                return compile_error_call("Math.min requires two arguments");
            };
            math_min_projection(left, right, primitive)
        }
        NativeJavaStaticMethod::MathLog => {
            let Some(arg) = args.into_iter().next() else {
                return compile_error_call("Math.log requires one argument");
            };
            primitive_f64_method_projection(arg, "ln")
        }
        NativeJavaStaticMethod::MathLog10 => {
            let Some(arg) = args.into_iter().next() else {
                return compile_error_call("Math.log10 requires one argument");
            };
            primitive_f64_method_projection(arg, "log10")
        }
        NativeJavaStaticMethod::MathExp => {
            let Some(arg) = args.into_iter().next() else {
                return compile_error_call("Math.exp requires one argument");
            };
            primitive_f64_method_projection(arg, "exp")
        }
        NativeJavaStaticMethod::MathSqrt => {
            let Some(arg) = args.into_iter().next() else {
                return compile_error_call("Math.sqrt requires one argument");
            };
            primitive_f64_method_projection(arg, "sqrt")
        }
        NativeJavaStaticMethod::MathPow => {
            let mut args = args.into_iter();
            let Some(base) = args.next() else {
                return compile_error_call("Math.pow requires two arguments");
            };
            let Some(exponent) = args.next() else {
                return compile_error_call("Math.pow requires two arguments");
            };
            math_pow_projection(base, exponent)
        }
        NativeJavaStaticMethod::MathCeil => {
            let Some(arg) = args.into_iter().next() else {
                return compile_error_call("Math.ceil requires one argument");
            };
            primitive_f64_method_projection(arg, "ceil")
        }
        NativeJavaStaticMethod::MathFloor => {
            let Some(arg) = args.into_iter().next() else {
                return compile_error_call("Math.floor requires one argument");
            };
            primitive_f64_method_projection(arg, "floor")
        }
        NativeJavaStaticMethod::MathUnaryF64 {
            java_name,
            rust_method,
        } => {
            let Some(arg) = args.into_iter().next() else {
                return math_arity_error(java_name, "one argument");
            };
            primitive_f64_method_projection(arg, rust_method)
        }
        NativeJavaStaticMethod::MathBinaryF64 {
            java_name,
            rust_method,
        } => {
            let mut args = args.into_iter();
            let Some(left) = args.next() else {
                return math_arity_error(java_name, "two arguments");
            };
            let Some(right) = args.next() else {
                return math_arity_error(java_name, "two arguments");
            };
            primitive_f64_binary_method_projection(left, right, rust_method)
        }
        NativeJavaStaticMethod::MathRound { source } => {
            let Some(arg) = args.into_iter().next() else {
                return compile_error_call("Math.round requires one argument");
            };
            math_round_projection(arg, source)
        }
        NativeJavaStaticMethod::MathAbs { source } => {
            let Some(arg) = args.into_iter().next() else {
                return compile_error_call("Math.abs requires one argument");
            };
            math_abs_projection(arg, source)
        }
        NativeJavaStaticMethod::MathToIntExact => {
            let Some(arg) = args.into_iter().next() else {
                return compile_error_call("Math.toIntExact requires one argument");
            };
            math_to_int_exact_projection(arg)
        }
        NativeJavaStaticMethod::MathFloorDiv { primitive } => {
            let mut args = args.into_iter();
            let Some(left) = args.next() else {
                return compile_error_call("Math.floorDiv requires two arguments");
            };
            let Some(right) = args.next() else {
                return compile_error_call("Math.floorDiv requires two arguments");
            };
            math_floor_div_projection(left, right, primitive)
        }
        NativeJavaStaticMethod::MathFloorMod {
            primitive,
            return_primitive,
        } => {
            let mut args = args.into_iter();
            let Some(left) = args.next() else {
                return compile_error_call("Math.floorMod requires two arguments");
            };
            let Some(right) = args.next() else {
                return compile_error_call("Math.floorMod requires two arguments");
            };
            math_floor_mod_projection(left, right, primitive, return_primitive)
        }
        NativeJavaStaticMethod::IntegerParseInt => {
            let mut args = args.into_iter();
            let Some(arg) = args.next() else {
                return compile_error_call("Integer.parseInt requires at least one argument");
            };
            let arg = java_integral_parse_null_argument_as_number_format(arg);
            match args.next() {
                Some(radix) => primitive_parse_radix_projection(arg, radix, PrimitiveType::Int),
                None => primitive_parse_projection(arg, PrimitiveType::Int),
            }
        }
        NativeJavaStaticMethod::ByteParseByte => {
            let mut args = args.into_iter();
            let Some(arg) = args.next() else {
                return compile_error_call("Byte.parseByte requires at least one argument");
            };
            let arg = java_integral_parse_null_argument_as_number_format(arg);
            match args.next() {
                Some(radix) => primitive_parse_radix_projection(arg, radix, PrimitiveType::Byte),
                None => primitive_parse_projection(arg, PrimitiveType::Byte),
            }
        }
        NativeJavaStaticMethod::ShortParseShort => {
            let mut args = args.into_iter();
            let Some(arg) = args.next() else {
                return compile_error_call("Short.parseShort requires at least one argument");
            };
            let arg = java_integral_parse_null_argument_as_number_format(arg);
            match args.next() {
                Some(radix) => primitive_parse_radix_projection(arg, radix, PrimitiveType::Short),
                None => primitive_parse_projection(arg, PrimitiveType::Short),
            }
        }
        NativeJavaStaticMethod::LongParseLong => {
            let mut args = args.into_iter();
            let Some(arg) = args.next() else {
                return compile_error_call("Long.parseLong requires at least one argument");
            };
            let arg = java_integral_parse_null_argument_as_number_format(arg);
            match args.next() {
                Some(radix) => primitive_parse_radix_projection(arg, radix, PrimitiveType::Long),
                None => primitive_parse_projection(arg, PrimitiveType::Long),
            }
        }
        NativeJavaStaticMethod::FloatParseFloat => {
            let Some(arg) = args.into_iter().next() else {
                return compile_error_call("Float.parseFloat requires one argument");
            };
            primitive_parse_projection(arg, PrimitiveType::Float)
        }
        NativeJavaStaticMethod::DoubleParseDouble => {
            let Some(arg) = args.into_iter().next() else {
                return compile_error_call("Double.parseDouble requires one argument");
            };
            primitive_parse_projection(arg, PrimitiveType::Double)
        }
        NativeJavaStaticMethod::FloatingIsNaN => {
            let Some(arg) = args.into_iter().next() else {
                return compile_error_call("Float.isNaN/Double.isNaN requires one argument");
            };
            primitive_is_nan_projection(arg)
        }
        NativeJavaStaticMethod::CharacterPredicate {
            java_name,
            rust_method,
        }
        | NativeJavaStaticMethod::CharacterTransform {
            java_name,
            rust_method,
        } => {
            let Some(arg) = args.into_iter().next() else {
                return compile_error_call(&format!("Character.{java_name} requires one argument"));
            };
            receiver_method_call_projection(arg, rust_method, Vec::new())
        }
        NativeJavaStaticMethod::SystemGc => unit_expr(),
        NativeJavaStaticMethod::SystemExit => {
            let Some(status) = args.into_iter().next() else {
                return compile_error_call("System.exit requires one argument");
            };
            system_exit_projection(status)
        }
        NativeJavaStaticMethod::ThreadSleep => {
            if args.len() != 1 && args.len() != 2 {
                return compile_error_call("Thread.sleep requires one or two arguments");
            }
            let mut args = args.into_iter();
            let Some(millis) = args.next() else {
                return compile_error_call("Thread.sleep requires one argument");
            };
            thread_sleep_projection(millis, args.next())
        }
        NativeJavaStaticMethod::StringFormat => string_format_projection(args),
        NativeJavaStaticMethod::StringJoin => apply_string_join(args),
        NativeJavaStaticMethod::StringValueOf => {
            let Some(value) = args.into_iter().next() else {
                return compile_error_call("String.valueOf requires one argument");
            };
            string_value_of_lowered_arg_to_string(value)
        }
        NativeJavaStaticMethod::OptionalEmpty => {
            if !args.is_empty() {
                return compile_error_call("Optional.empty requires no arguments");
            }
            RustExpr::Var("None".to_string())
        }
        NativeJavaStaticMethod::OptionalOf => {
            let Some(value) = args.into_iter().next() else {
                return compile_error_call("Optional.of requires one argument");
            };
            optional_of_projection(value)
        }
        NativeJavaStaticMethod::OptionalOfNullable => {
            let Some(value) = args.into_iter().next() else {
                return compile_error_call("Optional.ofNullable requires one argument");
            };
            value
        }
        NativeJavaStaticMethod::ObjectsEquals => {
            let mut args = args.into_iter();
            let Some(left) = args.next() else {
                return compile_error_call("Objects.equals requires two arguments");
            };
            let Some(right) = args.next() else {
                return compile_error_call("Objects.equals requires two arguments");
            };
            objects_equals_projection(left, right)
        }
        NativeJavaStaticMethod::ObjectsToString => {
            let mut args = args.into_iter();
            let Some(value) = args.next() else {
                return compile_error_call("Objects.toString requires one or two arguments");
            };
            if args.next().is_some() {
                return compile_error_call(
                    "Objects.toString with a default requires shaped lowering",
                );
            }
            string_value_of_lowered_arg_to_string(value)
        }
        NativeJavaStaticMethod::ObjectsNonNull => {
            let Some(arg) = args.into_iter().next() else {
                return compile_error_call("Objects.nonNull requires one argument");
            };
            objects_non_null_projection(arg)
        }
        NativeJavaStaticMethod::ObjectsIsNull => {
            let Some(arg) = args.into_iter().next() else {
                return compile_error_call("Objects.isNull requires one argument");
            };
            objects_is_null_projection(arg)
        }
        NativeJavaStaticMethod::PatternCompile => {
            let mut args = args.into_iter();
            let Some(pattern) = args.next() else {
                return compile_error_call("Pattern.compile requires one argument");
            };
            if args.next().is_some() {
                return compile_error_call(
                    "Pattern.compile with flags requires regex flag lowering",
                );
            }
            function_call_projection(
                owner_associated_path_projection("Pattern", "compile"),
                vec![pattern],
            )
        }
    }
}

fn apply_string_join(args: Vec<RustExpr>) -> RustExpr {
    let mut args = args.into_iter();
    let Some(delimiter) = args.next() else {
        return compile_error_call("String.join requires arguments");
    };
    let Some(values) = args.next() else {
        return compile_error_call("String.join requires values");
    };
    let rest = args.collect::<Vec<_>>();
    if !rest.is_empty() {
        let mut varargs = Vec::with_capacity(rest.len() + 1);
        varargs.push(values);
        varargs.extend(rest);
        return string_join_varargs_projection(delimiter, varargs);
    }
    string_join_collection_projection(delimiter, values)
}

fn apply_native_collection_static_method(
    method: NativeCollectionStaticMethod,
    args: Vec<RustExpr>,
) -> RustExpr {
    match method {
        NativeCollectionStaticMethod::ArraysAsList => arrays_as_list_projection(args),
        NativeCollectionStaticMethod::ArraysEquals => {
            let left = args.first().cloned().unwrap_or_else(unit_expr);
            match args.as_slice() {
                [_, right] => arrays_equals_projection(left, right.clone()),
                [_, left_from, left_to, right, right_from, right_to] => {
                    arrays_equals_range_projection(
                        left,
                        left_from.clone(),
                        left_to.clone(),
                        right.clone(),
                        right_from.clone(),
                        right_to.clone(),
                    )
                }
                _ => arrays_equals_projection(left, unit_expr()),
            }
        }
        NativeCollectionStaticMethod::ArraysDeepEquals => {
            let left = args.first().cloned().unwrap_or_else(unit_expr);
            let right = args.get(1).cloned().unwrap_or_else(unit_expr);
            arrays_deep_equals_projection(left, right)
        }
        NativeCollectionStaticMethod::ArraysBinarySearch => {
            let source = args
                .first()
                .cloned()
                .unwrap_or_else(collections_empty_list_projection);
            match args.as_slice() {
                [_, key] => arrays_binary_search_projection(source, key.clone()),
                [_, from, to, key] => arrays_binary_search_range_projection(
                    source,
                    from.clone(),
                    to.clone(),
                    key.clone(),
                ),
                _ => arrays_binary_search_projection(source, unit_expr()),
            }
        }
        NativeCollectionStaticMethod::ArraysCompare => {
            let left = args
                .first()
                .cloned()
                .unwrap_or_else(collections_empty_list_projection);
            match args.as_slice() {
                [_, right] => arrays_compare_projection(left, right.clone()),
                [_, left_from, left_to, right, right_from, right_to] => {
                    arrays_compare_range_projection(
                        left,
                        left_from.clone(),
                        left_to.clone(),
                        right.clone(),
                        right_from.clone(),
                        right_to.clone(),
                    )
                }
                _ => arrays_compare_projection(left, collections_empty_list_projection()),
            }
        }
        NativeCollectionStaticMethod::ArraysMismatch => {
            let left = args
                .first()
                .cloned()
                .unwrap_or_else(collections_empty_list_projection);
            match args.as_slice() {
                [_, right] => arrays_mismatch_projection(left, right.clone()),
                [_, left_from, left_to, right, right_from, right_to] => {
                    arrays_mismatch_range_projection(
                        left,
                        left_from.clone(),
                        left_to.clone(),
                        right.clone(),
                        right_from.clone(),
                        right_to.clone(),
                    )
                }
                _ => arrays_mismatch_projection(left, collections_empty_list_projection()),
            }
        }
        NativeCollectionStaticMethod::ArraysHashCode => args
            .first()
            .cloned()
            .map(arrays_hash_code_projection)
            .unwrap_or_else(|| RustExpr::Literal("0".to_string())),
        NativeCollectionStaticMethod::ArraysDeepHashCode => args
            .first()
            .cloned()
            .map(arrays_deep_hash_code_projection)
            .unwrap_or_else(|| RustExpr::Literal("1".to_string())),
        NativeCollectionStaticMethod::ArraysCopyOf => {
            let source = args
                .first()
                .cloned()
                .unwrap_or_else(collections_empty_list_projection);
            let len = args
                .get(1)
                .cloned()
                .unwrap_or_else(|| RustExpr::Literal("0".to_string()));
            arrays_copy_of_projection(source, len)
        }
        NativeCollectionStaticMethod::ArraysCopyOfRange => {
            let source = args
                .first()
                .cloned()
                .unwrap_or_else(collections_empty_list_projection);
            let from = args
                .get(1)
                .cloned()
                .unwrap_or_else(|| RustExpr::Literal("0".to_string()));
            let to = args
                .get(2)
                .cloned()
                .unwrap_or_else(|| RustExpr::Literal("0".to_string()));
            arrays_copy_of_range_projection(source, from, to)
        }
        NativeCollectionStaticMethod::ArraysToString => args
            .first()
            .cloned()
            .map(arrays_to_string_projection)
            .unwrap_or_else(|| RustExpr::Literal("\"[]\"".to_string())),
        NativeCollectionStaticMethod::ArraysDeepToString => args
            .first()
            .cloned()
            .map(arrays_deep_to_string_projection)
            .unwrap_or_else(|| RustExpr::Literal("\"[]\"".to_string())),
        NativeCollectionStaticMethod::CollectionsEmptyList => collections_empty_list_projection(),
        NativeCollectionStaticMethod::CollectionsEmptySet => collections_empty_set_projection(),
        NativeCollectionStaticMethod::CollectionsEmptyMap => collections_empty_map_projection(),
        NativeCollectionStaticMethod::CollectionsEmptySortedSet => {
            collections_empty_sorted_set_projection()
        }
        NativeCollectionStaticMethod::CollectionsUnmodifiableList
        | NativeCollectionStaticMethod::CollectionsUnmodifiableSet
        | NativeCollectionStaticMethod::CollectionsUnmodifiableMap => args
            .first()
            .cloned()
            .map(collections_unmodifiable_projection)
            .unwrap_or_else(default_trait_expr),
        NativeCollectionStaticMethod::CollectionsSingleton => {
            let value = args.first().cloned().unwrap_or_else(unit_expr);
            collections_singleton_projection(value)
        }
        NativeCollectionStaticMethod::CollectionsSingletonList => {
            let value = args.first().cloned().unwrap_or_else(unit_expr);
            collections_singleton_list_projection(value)
        }
        NativeCollectionStaticMethod::CollectionsSingletonMap => {
            let key = args.first().cloned().unwrap_or_else(unit_expr);
            let value = args.get(1).cloned().unwrap_or_else(unit_expr);
            collections_singleton_map_projection(key, value)
        }
        NativeCollectionStaticMethod::CollectionsNCopies => {
            let count = args
                .first()
                .cloned()
                .unwrap_or_else(|| RustExpr::Literal("0".to_string()));
            let value = args.get(1).cloned().unwrap_or_else(unit_expr);
            collections_n_copies_projection(count, value)
        }
    }
}

pub(crate) fn apply_collection_remove_if(receiver: RustExpr, predicate: &Expr) -> Option<RustExpr> {
    let Expr::Lambda { params, body } = predicate else {
        return None;
    };
    let param = &params.first()?.name;
    let LambdaBody::Expr(body) = body else {
        return None;
    };
    let predicate = emit_stream_lambda_body_expr(body, param, StreamLambdaValue::BorrowedItem)?;
    Some(collection_remove_if_projection(receiver, param, predicate))
}

fn apply_native_collection_method(
    receiver: RustExpr,
    receiver_ty: Option<&JavaType>,
    method: NativeCollectionMethod,
    mut args: Vec<RustExpr>,
) -> RustExpr {
    if matches!(method, NativeCollectionMethod::Len) {
        return len_i32_projection(strip_redundant_java_null_expect_after_unwrap(receiver));
    }
    let receiver = if matches!(method, NativeCollectionMethod::IsEmpty) {
        strip_redundant_java_null_expect_after_unwrap(receiver)
    } else {
        receiver
    };
    if matches!(method, NativeCollectionMethod::MapKeys) {
        return owned_map_keys_vec_projection(receiver);
    }
    if matches!(method, NativeCollectionMethod::MapValues) {
        return owned_map_values_vec_projection(receiver);
    }
    if matches!(method, NativeCollectionMethod::MapFirstKey) {
        return map_first_key_projection(receiver);
    }
    if matches!(method, NativeCollectionMethod::MapLastKey) {
        return map_last_key_projection(receiver);
    }
    if matches!(
        method,
        NativeCollectionMethod::MapLowerKey
            | NativeCollectionMethod::MapFloorKey
            | NativeCollectionMethod::MapCeilingKey
            | NativeCollectionMethod::MapHigherKey
    ) {
        return apply_sorted_map_navigation_key(receiver, method, args);
    }
    if matches!(method, NativeCollectionMethod::MapContainsValue) {
        return apply_map_contains_value(receiver, args);
    }
    if matches!(method, NativeCollectionMethod::MapGetOrDefault) {
        return apply_map_get_or_default(receiver, args);
    }
    if matches!(method, NativeCollectionMethod::MapPutIfAbsent) {
        return apply_map_put_if_absent(receiver, args);
    }
    if matches!(method, NativeCollectionMethod::ToArray) {
        return owned_collection_to_vec_projection_for_receiver_type(receiver, receiver_ty);
    }
    if matches!(method, NativeCollectionMethod::Iterator) {
        return java_iterator_from_collection_projection(receiver);
    }
    if native_collection_mutation_needs_erased_object_arg(receiver_ty, method)
        && let Some(arg) = args.get_mut(0)
    {
        *arg = erased_object_box_projection(arg.clone());
    }
    let rust_name = match method {
        NativeCollectionMethod::Len => "len",
        NativeCollectionMethod::IsEmpty => "is_empty",
        NativeCollectionMethod::ListHashCode => {
            return java_list_hash_code_projection(receiver);
        }
        NativeCollectionMethod::ListAdd => {
            return apply_list_add(receiver, args);
        }
        NativeCollectionMethod::ListInsert => {
            return apply_list_insert(receiver, args);
        }
        NativeCollectionMethod::ListGet => {
            return apply_list_get(receiver, args);
        }
        NativeCollectionMethod::ListSubList => {
            return apply_list_sub_list(receiver, args);
        }
        NativeCollectionMethod::ListIndexOf => {
            return apply_list_index_of(receiver, args);
        }
        NativeCollectionMethod::ListSet => {
            return apply_list_set(receiver, args);
        }
        NativeCollectionMethod::ListRemoveIndex => {
            return apply_list_remove_index(receiver, args);
        }
        NativeCollectionMethod::ListRemoveObject => {
            return apply_list_remove_object(receiver, args);
        }
        NativeCollectionMethod::QueueElement => {
            return queue_element_projection(receiver);
        }
        NativeCollectionMethod::QueueRemove => {
            return queue_remove_projection(receiver);
        }
        NativeCollectionMethod::Contains => "contains",
        NativeCollectionMethod::SetHashCode => {
            return java_set_hash_code_projection(receiver);
        }
        NativeCollectionMethod::SetInsert => "insert",
        NativeCollectionMethod::SetRemove => "remove",
        NativeCollectionMethod::AddAll => {
            return apply_native_collection_add_all(receiver, args);
        }
        NativeCollectionMethod::RemoveAll => {
            return apply_native_collection_remove_all(receiver, args);
        }
        NativeCollectionMethod::ContainsAll => {
            return apply_native_collection_contains_all(receiver, args);
        }
        NativeCollectionMethod::RetainAll => {
            return apply_native_collection_retain_all(receiver, args);
        }
        NativeCollectionMethod::Extend => "extend",
        NativeCollectionMethod::MapHashCode => {
            return java_map_hash_code_projection(receiver);
        }
        NativeCollectionMethod::MapInsert => "insert",
        NativeCollectionMethod::MapPutIfAbsent => {
            unreachable!("MapPutIfAbsent is rendered from contains_key/get/insert")
        }
        NativeCollectionMethod::MapGet => "get",
        NativeCollectionMethod::MapGetOrDefault => {
            unreachable!("MapGetOrDefault is rendered from get(...).cloned().unwrap_or(...)")
        }
        NativeCollectionMethod::MapRemove => "remove",
        NativeCollectionMethod::MapContainsKey => "contains_key",
        NativeCollectionMethod::MapContainsValue => {
            unreachable!("MapContainsValue is rendered from values().any(...)")
        }
        NativeCollectionMethod::MapKeys => unreachable!("MapKeys is rendered as an owned key view"),
        NativeCollectionMethod::MapValues => {
            unreachable!("MapValues is rendered as an owned value view")
        }
        NativeCollectionMethod::MapIter => "iter",
        NativeCollectionMethod::MapFirstKey => unreachable!("MapFirstKey is rendered from keys"),
        NativeCollectionMethod::MapLastKey => unreachable!("MapLastKey is rendered from keys"),
        NativeCollectionMethod::MapLowerKey
        | NativeCollectionMethod::MapFloorKey
        | NativeCollectionMethod::MapCeilingKey
        | NativeCollectionMethod::MapHigherKey => {
            unreachable!("sorted-map navigation keys are rendered from ranges")
        }
        NativeCollectionMethod::Iterator => unreachable!("Iterator is rendered as JavaIterator"),
        NativeCollectionMethod::ToArray => unreachable!("ToArray is rendered as an owned Vec"),
    };
    receiver_method_call_projection_exact(receiver, rust_name, args)
}

fn map_first_key_projection(receiver: RustExpr) -> RustExpr {
    unwrap_result_or_panic_uncaught_java_exception(map_first_key_result_projection(receiver))
}

fn map_last_key_projection(receiver: RustExpr) -> RustExpr {
    unwrap_result_or_panic_uncaught_java_exception(map_last_key_result_projection(receiver))
}

pub(crate) fn map_first_key_result_projection(receiver: RustExpr) -> RustExpr {
    map_key_result_projection(receiver, "next", "Java TreeMap.firstKey on empty map")
}

pub(crate) fn map_last_key_result_projection(receiver: RustExpr) -> RustExpr {
    map_key_result_projection(receiver, "next_back", "Java TreeMap.lastKey on empty map")
}

fn map_key_result_projection(receiver: RustExpr, iterator_method: &str, message: &str) -> RustExpr {
    let key_ref = receiver_method_call_projection_exact(
        receiver_method_call_projection_exact(receiver, "keys", Vec::new()),
        iterator_method,
        Vec::new(),
    );
    let key_ref = receiver_method_call_projection_exact(
        key_ref,
        "ok_or_else",
        vec![RustExpr::Closure {
            is_move: false,
            params: Vec::new(),
            return_type: None,
            body: crate::rust_ir::RustClosureBody::Expr(Box::new(
                crate::exception_lowering::new_exception_expr(
                    "NoSuchElementException",
                    rust_string_literal_projection(message),
                ),
            )),
        }],
    );
    receiver_method_call_projection_exact(
        key_ref,
        "map",
        vec![RustExpr::Closure {
            is_move: false,
            return_type: None,
            params: vec![RustClosureParam {
                pattern: "value".to_string(),
                ty: None,
            }],
            body: RustClosureBody::Expr(Box::new(receiver_method_call_projection_exact(
                RustExpr::Var("value".to_string()),
                "clone",
                Vec::new(),
            ))),
        }],
    )
}

fn apply_sorted_map_navigation_key(
    receiver: RustExpr,
    method: NativeCollectionMethod,
    args: Vec<RustExpr>,
) -> RustExpr {
    let key = args.first().cloned().unwrap_or_else(unit_expr);
    let range = match method {
        NativeCollectionMethod::MapLowerKey => sorted_map_range_bounds(
            sorted_map_unbounded_bound(),
            sorted_map_excluded_bound(sorted_map_navigation_key_value(key)),
        ),
        NativeCollectionMethod::MapFloorKey => sorted_map_range_bounds(
            sorted_map_unbounded_bound(),
            sorted_map_included_bound(sorted_map_navigation_key_value(key)),
        ),
        NativeCollectionMethod::MapCeilingKey => sorted_map_range_bounds(
            sorted_map_included_bound(sorted_map_navigation_key_value(key)),
            sorted_map_unbounded_bound(),
        ),
        NativeCollectionMethod::MapHigherKey => sorted_map_range_bounds(
            sorted_map_excluded_bound(sorted_map_navigation_key_value(key)),
            sorted_map_unbounded_bound(),
        ),
        _ => unreachable!("checked sorted-map navigation method"),
    };
    let entry = receiver_method_call_projection_exact(receiver, "range::<_, _>", vec![range]);
    let entry = receiver_method_call_projection_exact(
        entry,
        if matches!(
            method,
            NativeCollectionMethod::MapLowerKey | NativeCollectionMethod::MapFloorKey
        ) {
            "next_back"
        } else {
            "next"
        },
        Vec::new(),
    );
    receiver_method_call_projection_exact(
        entry,
        "map",
        vec![RustExpr::Closure {
            is_move: false,
            params: vec![RustClosureParam {
                pattern: "(key, _)".to_string(),
                ty: None,
            }],
            return_type: None,
            body: RustClosureBody::Expr(Box::new(receiver_method_call_projection_exact(
                RustExpr::Var("key".to_string()),
                "clone",
                Vec::new(),
            ))),
        }],
    )
}

fn sorted_map_navigation_key_value(key: RustExpr) -> RustExpr {
    receiver_method_call_projection_exact(RustExpr::Grouped(Box::new(key)), "clone", Vec::new())
}

fn sorted_map_range_bounds(start: RustExpr, end: RustExpr) -> RustExpr {
    RustExpr::Tuple(vec![start, end])
}

fn sorted_map_unbounded_bound() -> RustExpr {
    RustExpr::path(["std", "ops", "Bound", "Unbounded"])
}

fn sorted_map_included_bound(value: RustExpr) -> RustExpr {
    function_call_projection(
        RustExpr::path(["std", "ops", "Bound", "Included"]),
        vec![value],
    )
}

fn sorted_map_excluded_bound(value: RustExpr) -> RustExpr {
    function_call_projection(
        RustExpr::path(["std", "ops", "Bound", "Excluded"]),
        vec![value],
    )
}

fn native_collection_mutation_needs_erased_object_arg(
    receiver_ty: Option<&JavaType>,
    method: NativeCollectionMethod,
) -> bool {
    if !matches!(
        method,
        NativeCollectionMethod::ListAdd
            | NativeCollectionMethod::ListInsert
            | NativeCollectionMethod::SetInsert
    ) {
        return false;
    }
    receiver_ty
        .and_then(crate::types::sequence_element_type)
        .is_some_and(|element_ty| matches!(element_ty, JavaType::Wildcard(_)))
}

fn apply_list_add(receiver: RustExpr, mut args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 1 {
        return compile_error_call("Java List.add(value) requires one value argument");
    }
    let value = args.pop().expect("len checked");
    RustExpr::Block {
        stmts: vec![RustStmt::Expr(receiver_method_call_projection_exact(
            receiver,
            "push",
            vec![value],
        ))],
        tail: Some(Box::new(RustExpr::Literal("true".to_string()))),
    }
}

fn apply_list_insert(receiver: RustExpr, mut args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 2 {
        return compile_error_call(
            "Java List.add(index, value) requires index and value arguments",
        );
    }
    let value = args.pop().expect("len checked");
    let index = args.pop().expect("len checked");
    receiver_method_call_projection_exact(
        receiver,
        "insert",
        vec![
            index_to_usize_projection(index, IndexToUsizeProjection::StreamLambdaArgument),
            value,
        ],
    )
}

pub(crate) fn list_insert_result_projection(
    receiver: RustExpr,
    index: RustExpr,
    value: RustExpr,
) -> RustExpr {
    let index_name = "__java_list_insert_index";
    let index_usize_name = "__java_list_insert_index_usize";
    let index_var = RustExpr::Var(index_name.to_string());
    let index_usize_var = RustExpr::Var(index_usize_name.to_string());
    let out_of_bounds = list_index_out_of_bounds_exception_expr();
    let insert_value = receiver_method_call_projection_exact(
        receiver.clone(),
        "insert",
        vec![index_usize_var.clone(), value],
    );
    RustExpr::Block {
        stmts: vec![RustStmt::Let {
            mutable: false,
            name: index_name.to_string(),
            ty: None,
            value: Some(index),
        }],
        tail: Some(Box::new(RustExpr::If {
            condition: Box::new(RustExpr::Binary {
                op: "<".to_string(),
                left: Box::new(index_var.clone()),
                right: Box::new(RustExpr::Literal("0".to_string())),
            }),
            then_expr: Box::new(function_call_projection(
                RustExpr::Var("Err".to_string()),
                vec![out_of_bounds.clone()],
            )),
            else_expr: Box::new(RustExpr::Block {
                stmts: vec![RustStmt::Let {
                    mutable: false,
                    name: index_usize_name.to_string(),
                    ty: None,
                    value: Some(RustExpr::Cast {
                        expr: Box::new(index_var),
                        ty: "usize".to_string(),
                    }),
                }],
                tail: Some(Box::new(RustExpr::If {
                    condition: Box::new(RustExpr::Binary {
                        op: ">".to_string(),
                        left: Box::new(index_usize_var.clone()),
                        right: Box::new(receiver_method_call_projection_exact(
                            receiver,
                            "len",
                            Vec::new(),
                        )),
                    }),
                    then_expr: Box::new(function_call_projection(
                        RustExpr::Var("Err".to_string()),
                        vec![out_of_bounds],
                    )),
                    else_expr: Box::new(RustExpr::Block {
                        stmts: vec![RustStmt::Expr(insert_value)],
                        tail: Some(Box::new(function_call_projection(
                            RustExpr::Var("Ok".to_string()),
                            vec![RustExpr::Tuple(Vec::new())],
                        ))),
                    }),
                })),
            }),
        })),
    }
}

fn apply_list_index_of(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    let Some(needle) = args.into_iter().next() else {
        return compile_error_call("Java List.indexOf requires one argument");
    };
    sequence_index_of_projection(receiver, needle)
}

fn apply_list_get(receiver: RustExpr, mut args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 1 {
        return compile_error_call("Java List.get requires one index argument");
    }
    let index = args.pop().expect("len checked");
    receiver_method_call_projection_exact(
        receiver,
        "get",
        vec![index_to_usize_projection(
            index,
            IndexToUsizeProjection::StreamLambdaArgument,
        )],
    )
}

pub(crate) fn list_get_result_projection(receiver: RustExpr, index: RustExpr) -> RustExpr {
    let index_name = "__java_list_get_index";
    let index_var = RustExpr::Var(index_name.to_string());
    let value = receiver_method_call_projection_exact(
        receiver,
        "get",
        vec![RustExpr::Cast {
            expr: Box::new(index_var.clone()),
            ty: "usize".to_string(),
        }],
    );
    let value = receiver_method_call_projection_exact(value, "cloned", Vec::new());
    let out_of_bounds = crate::exception_lowering::new_exception_expr(
        "IndexOutOfBoundsException",
        rust_string_literal_projection("Java list index out of bounds"),
    );
    let missing_value = receiver_method_call_projection_exact(
        value,
        "ok_or_else",
        vec![RustExpr::Closure {
            is_move: false,
            params: Vec::new(),
            return_type: None,
            body: RustClosureBody::Expr(Box::new(out_of_bounds.clone())),
        }],
    );
    RustExpr::Block {
        stmts: vec![RustStmt::Let {
            mutable: false,
            name: index_name.to_string(),
            ty: None,
            value: Some(index),
        }],
        tail: Some(Box::new(RustExpr::If {
            condition: Box::new(RustExpr::Binary {
                op: "<".to_string(),
                left: Box::new(index_var),
                right: Box::new(RustExpr::Literal("0".to_string())),
            }),
            then_expr: Box::new(function_call_projection(
                RustExpr::Var("Err".to_string()),
                vec![out_of_bounds],
            )),
            else_expr: Box::new(missing_value),
        })),
    }
}

fn apply_list_sub_list(receiver: RustExpr, mut args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 2 {
        return compile_error_call("Java List.subList requires begin and end indexes");
    }
    let end = args.pop().expect("len checked");
    let begin = args.pop().expect("len checked");
    let begin = index_to_usize_projection(begin, IndexToUsizeProjection::StreamLambdaArgument);
    let end = index_to_usize_projection(end, IndexToUsizeProjection::StreamLambdaArgument);
    receiver_method_call_projection_exact(
        RustExpr::Index(
            Box::new(receiver),
            Box::new(RustExpr::Range {
                start: Box::new(begin),
                end: Box::new(end),
                inclusive: false,
            }),
        ),
        "to_vec",
        Vec::new(),
    )
}

pub(crate) fn list_sub_list_result_projection(
    receiver: RustExpr,
    begin: RustExpr,
    end: RustExpr,
) -> RustExpr {
    let begin_name = "__java_list_sub_list_begin";
    let end_name = "__java_list_sub_list_end";
    let begin_var = RustExpr::Var(begin_name.to_string());
    let end_var = RustExpr::Var(end_name.to_string());
    let begin_usize = RustExpr::Cast {
        expr: Box::new(begin_var.clone()),
        ty: "usize".to_string(),
    };
    let end_usize = RustExpr::Cast {
        expr: Box::new(end_var.clone()),
        ty: "usize".to_string(),
    };
    let out_of_bounds = list_index_out_of_bounds_exception_expr();
    let invalid_order = RustExpr::Binary {
        op: "<".to_string(),
        left: Box::new(end_var.clone()),
        right: Box::new(begin_var.clone()),
    };
    let invalid_bounds = RustExpr::Binary {
        op: "||".to_string(),
        left: Box::new(RustExpr::Binary {
            op: "<".to_string(),
            left: Box::new(begin_var.clone()),
            right: Box::new(RustExpr::Literal("0".to_string())),
        }),
        right: Box::new(RustExpr::Binary {
            op: ">".to_string(),
            left: Box::new(end_var.clone()),
            right: Box::new(len_i32_projection(receiver.clone())),
        }),
    };
    let slice = RustExpr::Index(
        Box::new(receiver),
        Box::new(RustExpr::Range {
            start: Box::new(begin_usize),
            end: Box::new(end_usize),
            inclusive: false,
        }),
    );
    RustExpr::Block {
        stmts: vec![
            RustStmt::Let {
                mutable: false,
                name: begin_name.to_string(),
                ty: None,
                value: Some(begin),
            },
            RustStmt::Let {
                mutable: false,
                name: end_name.to_string(),
                ty: None,
                value: Some(end),
            },
        ],
        tail: Some(Box::new(RustExpr::If {
            condition: Box::new(invalid_bounds),
            then_expr: Box::new(function_call_projection(
                RustExpr::Var("Err".to_string()),
                vec![out_of_bounds],
            )),
            else_expr: Box::new(RustExpr::If {
                condition: Box::new(invalid_order),
                then_expr: Box::new(function_call_projection(
                    RustExpr::Var("Err".to_string()),
                    vec![list_invalid_range_exception_expr()],
                )),
                else_expr: Box::new(function_call_projection(
                    RustExpr::Var("Ok".to_string()),
                    vec![receiver_method_call_projection_exact(
                        slice,
                        "to_vec",
                        Vec::new(),
                    )],
                )),
            }),
        })),
    }
}

fn apply_list_set(receiver: RustExpr, mut args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 2 {
        return compile_error_call("Java List.set requires index and value arguments");
    }
    let value = args.pop().expect("len checked");
    let index = args.pop().expect("len checked");
    let index_name = "__java_list_set_index";
    let old_name = "__java_list_set_old";
    let index_expr = RustExpr::Var(index_name.to_string());
    RustExpr::Block {
        stmts: vec![
            RustStmt::Let {
                mutable: false,
                name: index_name.to_string(),
                ty: None,
                value: Some(index_to_usize_projection(
                    index,
                    IndexToUsizeProjection::StreamLambdaArgument,
                )),
            },
            RustStmt::Let {
                mutable: false,
                name: old_name.to_string(),
                ty: None,
                value: Some(receiver_method_call_projection_exact(
                    RustExpr::Index(Box::new(receiver.clone()), Box::new(index_expr.clone())),
                    "clone",
                    Vec::new(),
                )),
            },
            RustStmt::Assign {
                target: RustExpr::Index(Box::new(receiver), Box::new(index_expr)),
                value,
            },
        ],
        tail: Some(Box::new(RustExpr::Var(old_name.to_string()))),
    }
}

pub(crate) fn list_set_result_projection(
    receiver: RustExpr,
    index: RustExpr,
    value: RustExpr,
) -> RustExpr {
    let index_name = "__java_list_set_index";
    let index_usize_name = "__java_list_set_index_usize";
    let old_name = "__java_list_set_old";
    let index_var = RustExpr::Var(index_name.to_string());
    let index_usize_var = RustExpr::Var(index_usize_name.to_string());
    let out_of_bounds = list_index_out_of_bounds_exception_expr();
    let old_value = receiver_method_call_projection_exact(
        receiver_method_call_projection_exact(
            receiver.clone(),
            "get",
            vec![index_usize_var.clone()],
        ),
        "cloned",
        Vec::new(),
    );
    let old_value = RustExpr::Try(Box::new(ok_or_else_java_exception_projection(
        old_value,
        out_of_bounds.clone(),
    )));
    let target = RustExpr::Unary {
        op: "*".to_string(),
        expr: Box::new(RustExpr::Try(Box::new(
            ok_or_else_java_exception_projection(
                receiver_method_call_projection_exact(receiver, "get_mut", vec![index_usize_var]),
                out_of_bounds.clone(),
            ),
        ))),
    };
    RustExpr::Block {
        stmts: vec![RustStmt::Let {
            mutable: false,
            name: index_name.to_string(),
            ty: None,
            value: Some(index),
        }],
        tail: Some(Box::new(RustExpr::If {
            condition: Box::new(RustExpr::Binary {
                op: "<".to_string(),
                left: Box::new(index_var.clone()),
                right: Box::new(RustExpr::Literal("0".to_string())),
            }),
            then_expr: Box::new(function_call_projection(
                RustExpr::Var("Err".to_string()),
                vec![out_of_bounds],
            )),
            else_expr: Box::new(RustExpr::Block {
                stmts: vec![
                    RustStmt::Let {
                        mutable: false,
                        name: index_usize_name.to_string(),
                        ty: None,
                        value: Some(RustExpr::Cast {
                            expr: Box::new(index_var),
                            ty: "usize".to_string(),
                        }),
                    },
                    RustStmt::Let {
                        mutable: false,
                        name: old_name.to_string(),
                        ty: None,
                        value: Some(old_value),
                    },
                    RustStmt::Assign { target, value },
                ],
                tail: Some(Box::new(function_call_projection(
                    RustExpr::Var("Ok".to_string()),
                    vec![RustExpr::Var(old_name.to_string())],
                ))),
            }),
        })),
    }
}

fn list_index_out_of_bounds_exception_expr() -> RustExpr {
    crate::exception_lowering::new_exception_expr(
        "IndexOutOfBoundsException",
        rust_string_literal_projection("Java list index out of bounds"),
    )
}

fn list_invalid_range_exception_expr() -> RustExpr {
    crate::exception_lowering::new_exception_expr(
        "IllegalArgumentException",
        rust_string_literal_projection("fromIndex > toIndex"),
    )
}

fn ok_or_else_java_exception_projection(value: RustExpr, exception: RustExpr) -> RustExpr {
    receiver_method_call_projection_exact(
        value,
        "ok_or_else",
        vec![RustExpr::Closure {
            is_move: false,
            params: Vec::new(),
            return_type: None,
            body: RustClosureBody::Expr(Box::new(exception)),
        }],
    )
}

fn apply_list_remove_index(receiver: RustExpr, mut args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 1 {
        return compile_error_call("Java List.remove(int) requires one index argument");
    }
    let index = args.pop().expect("len checked");
    receiver_method_call_projection_exact(
        receiver,
        "remove",
        vec![index_to_usize_projection(
            index,
            IndexToUsizeProjection::StreamLambdaArgument,
        )],
    )
}

pub(crate) fn list_remove_index_result_projection(receiver: RustExpr, index: RustExpr) -> RustExpr {
    let index_name = "__java_list_remove_index";
    let index_usize_name = "__java_list_remove_index_usize";
    let old_name = "__java_list_remove_old";
    let index_var = RustExpr::Var(index_name.to_string());
    let index_usize_var = RustExpr::Var(index_usize_name.to_string());
    let out_of_bounds = list_index_out_of_bounds_exception_expr();
    let old_value = receiver_method_call_projection_exact(
        receiver_method_call_projection_exact(
            receiver.clone(),
            "get",
            vec![index_usize_var.clone()],
        ),
        "cloned",
        Vec::new(),
    );
    let old_value = RustExpr::Try(Box::new(ok_or_else_java_exception_projection(
        old_value,
        out_of_bounds.clone(),
    )));
    let remove_value =
        receiver_method_call_projection_exact(receiver, "remove", vec![index_usize_var]);
    RustExpr::Block {
        stmts: vec![RustStmt::Let {
            mutable: false,
            name: index_name.to_string(),
            ty: None,
            value: Some(index),
        }],
        tail: Some(Box::new(RustExpr::If {
            condition: Box::new(RustExpr::Binary {
                op: "<".to_string(),
                left: Box::new(index_var.clone()),
                right: Box::new(RustExpr::Literal("0".to_string())),
            }),
            then_expr: Box::new(function_call_projection(
                RustExpr::Var("Err".to_string()),
                vec![out_of_bounds],
            )),
            else_expr: Box::new(RustExpr::Block {
                stmts: vec![
                    RustStmt::Let {
                        mutable: false,
                        name: index_usize_name.to_string(),
                        ty: None,
                        value: Some(RustExpr::Cast {
                            expr: Box::new(index_var),
                            ty: "usize".to_string(),
                        }),
                    },
                    RustStmt::Let {
                        mutable: false,
                        name: old_name.to_string(),
                        ty: None,
                        value: Some(old_value),
                    },
                    RustStmt::Expr(remove_value),
                ],
                tail: Some(Box::new(function_call_projection(
                    RustExpr::Var("Ok".to_string()),
                    vec![RustExpr::Var(old_name.to_string())],
                ))),
            }),
        })),
    }
}

fn queue_remove_projection(receiver: RustExpr) -> RustExpr {
    unwrap_result_or_panic_uncaught_java_exception(queue_remove_result_projection(receiver))
}

fn queue_element_projection(receiver: RustExpr) -> RustExpr {
    unwrap_result_or_panic_uncaught_java_exception(queue_element_result_projection(receiver))
}

pub(crate) fn queue_element_result_projection(receiver: RustExpr) -> RustExpr {
    let value = receiver_method_call_projection_exact(
        receiver_method_call_projection_exact(
            receiver,
            "get",
            vec![RustExpr::Literal("0".to_string())],
        ),
        "cloned",
        Vec::new(),
    );
    ok_or_else_java_exception_projection(value, queue_no_such_element_exception_expr("element"))
}

pub(crate) fn queue_remove_result_projection(receiver: RustExpr) -> RustExpr {
    let no_such_element = queue_no_such_element_exception_expr("remove");
    RustExpr::If {
        condition: Box::new(receiver_method_call_projection_exact(
            receiver.clone(),
            "is_empty",
            Vec::new(),
        )),
        then_expr: Box::new(function_call_projection(
            RustExpr::Var("Err".to_string()),
            vec![no_such_element],
        )),
        else_expr: Box::new(function_call_projection(
            RustExpr::Var("Ok".to_string()),
            vec![receiver_method_call_projection_exact(
                receiver,
                "remove",
                vec![RustExpr::Literal("0".to_string())],
            )],
        )),
    }
}

fn queue_no_such_element_exception_expr(method: &str) -> RustExpr {
    crate::exception_lowering::new_exception_expr(
        "NoSuchElementException",
        rust_string_literal_projection(&format!("Java Queue.{method} on empty queue")),
    )
}

fn apply_list_remove_object(receiver: RustExpr, mut args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 1 {
        return compile_error_call("Java List.remove(Object) requires one argument");
    }
    let needle = args.pop().expect("len checked");
    let needle_name = "__java_list_remove_needle";
    RustExpr::Block {
        stmts: vec![RustStmt::Let {
            mutable: false,
            name: needle_name.to_string(),
            ty: None,
            value: Some(needle),
        }],
        tail: Some(Box::new(RustExpr::Match {
            expr: Box::new(receiver_method_call_projection_exact(
                receiver_method_call_projection_exact(receiver.clone(), "iter", Vec::new()),
                "position",
                vec![RustExpr::Closure {
                    is_move: false,
                    return_type: None,
                    params: vec![RustClosureParam {
                        pattern: "item".to_string(),
                        ty: None,
                    }],
                    body: RustClosureBody::Expr(Box::new(RustExpr::Binary {
                        op: "==".to_string(),
                        left: Box::new(RustExpr::Var("item".to_string())),
                        right: Box::new(RustExpr::Var(needle_name.to_string())),
                    })),
                }],
            )),
            arms: vec![
                crate::rust_ir::RustMatchArm {
                    pattern: "Some(__java_list_remove_index)".to_string(),
                    body: RustExpr::Block {
                        stmts: vec![RustStmt::Expr(receiver_method_call_projection_exact(
                            receiver,
                            "remove",
                            vec![RustExpr::Var("__java_list_remove_index".to_string())],
                        ))],
                        tail: Some(Box::new(RustExpr::Literal("true".to_string()))),
                    },
                },
                crate::rust_ir::RustMatchArm {
                    pattern: "None".to_string(),
                    body: RustExpr::Literal("false".to_string()),
                },
            ],
        })),
    }
}

fn apply_map_contains_value(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    let Some(needle) = args.into_iter().next() else {
        return compile_error_call("Java Map.containsValue requires one argument");
    };
    receiver_method_call_projection_exact(
        receiver_method_call_projection_exact(receiver, "values", Vec::new()),
        "any",
        vec![RustExpr::Closure {
            is_move: false,
            return_type: None,
            params: vec![RustClosureParam {
                pattern: "value".to_string(),
                ty: None,
            }],
            body: RustClosureBody::Expr(Box::new(RustExpr::Binary {
                op: "==".to_string(),
                left: Box::new(RustExpr::Var("value".to_string())),
                right: Box::new(needle),
            })),
        }],
    )
}

fn apply_map_get_or_default(receiver: RustExpr, mut args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 2 {
        return compile_error_call("Java Map.getOrDefault requires two arguments");
    }
    let default_value = args.pop().expect("len checked");
    let key = args.pop().expect("len checked");
    receiver_method_call_projection_exact(
        receiver_method_call_projection_exact(
            receiver_method_call_projection_exact(receiver, "get", vec![key]),
            "cloned",
            Vec::new(),
        ),
        "unwrap_or",
        vec![default_value],
    )
}

fn apply_map_put_if_absent(receiver: RustExpr, mut args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 2 {
        return compile_error_call("Java Map.putIfAbsent requires two arguments");
    }
    let value = args.pop().expect("len checked");
    let key = args.pop().expect("len checked");
    let key_name = "__java_put_if_absent_key";
    RustExpr::Block {
        stmts: vec![RustStmt::Let {
            mutable: false,
            name: key_name.to_string(),
            ty: None,
            value: Some(key),
        }],
        tail: Some(Box::new(RustExpr::If {
            condition: Box::new(receiver_method_call_projection_exact(
                receiver.clone(),
                "contains_key",
                vec![RustExpr::Unary {
                    op: "&".to_string(),
                    expr: Box::new(RustExpr::Var(key_name.to_string())),
                }],
            )),
            then_expr: Box::new(receiver_method_call_projection_exact(
                receiver_method_call_projection_exact(
                    receiver.clone(),
                    "get",
                    vec![RustExpr::Unary {
                        op: "&".to_string(),
                        expr: Box::new(RustExpr::Var(key_name.to_string())),
                    }],
                ),
                "cloned",
                Vec::new(),
            )),
            else_expr: Box::new(RustExpr::Block {
                stmts: vec![RustStmt::Expr(receiver_method_call_projection_exact(
                    receiver,
                    "insert",
                    vec![RustExpr::Var(key_name.to_string()), value],
                ))],
                tail: Some(Box::new(RustExpr::Var("None".to_string()))),
            }),
        })),
    }
}

pub(crate) fn apply_native_collection_extend(receiver: RustExpr, arg: RustExpr) -> RustExpr {
    receiver_method_call_projection_exact(receiver, "extend", vec![arg])
}

fn apply_native_collection_add_all(receiver: RustExpr, mut args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 1 {
        return compile_error_call("Java Collection.addAll(source) requires one source argument");
    }
    let source = args.remove(0);
    let before = "__java_add_all_before";
    RustExpr::Block {
        stmts: vec![
            RustStmt::Let {
                mutable: false,
                name: before.to_string(),
                ty: None,
                value: Some(receiver_method_call_projection_exact(
                    receiver.clone(),
                    "len",
                    Vec::new(),
                )),
            },
            RustStmt::Expr(apply_native_collection_extend(receiver.clone(), source)),
        ],
        tail: Some(Box::new(RustExpr::Binary {
            op: "!=".to_string(),
            left: Box::new(receiver_method_call_projection_exact(
                receiver,
                "len",
                Vec::new(),
            )),
            right: Box::new(RustExpr::Var(before.to_string())),
        })),
    }
}

fn apply_native_collection_remove_all(receiver: RustExpr, mut args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 1 {
        return compile_error_call(
            "Java Collection.removeAll(source) requires one source argument",
        );
    }
    let source = args.remove(0);
    let values = "__java_remove_all_values";
    let before = "__java_remove_all_before";
    if collection_bulk_source_aliases_receiver(&source, &receiver) {
        return RustExpr::Block {
            stmts: vec![
                RustStmt::Let {
                    mutable: false,
                    name: before.to_string(),
                    ty: None,
                    value: Some(receiver_method_call_projection_exact(
                        receiver.clone(),
                        "len",
                        Vec::new(),
                    )),
                },
                RustStmt::Expr(receiver_method_call_projection_exact(
                    receiver.clone(),
                    "clear",
                    Vec::new(),
                )),
            ],
            tail: Some(Box::new(RustExpr::Binary {
                op: "!=".to_string(),
                left: Box::new(RustExpr::Var(before.to_string())),
                right: Box::new(RustExpr::Literal("0".to_string())),
            })),
        };
    }
    RustExpr::Block {
        stmts: vec![
            RustStmt::Let {
                mutable: false,
                name: values.to_string(),
                ty: None,
                value: Some(source),
            },
            RustStmt::Let {
                mutable: false,
                name: before.to_string(),
                ty: None,
                value: Some(receiver_method_call_projection_exact(
                    receiver.clone(),
                    "len",
                    Vec::new(),
                )),
            },
            RustStmt::Expr(receiver_method_call_projection_exact(
                receiver.clone(),
                "retain",
                vec![RustExpr::Closure {
                    is_move: false,
                    params: vec![RustClosureParam {
                        pattern: "value".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: RustClosureBody::Expr(Box::new(RustExpr::Unary {
                        op: "!".to_string(),
                        expr: Box::new(receiver_method_call_projection_exact(
                            RustExpr::Var(values.to_string()),
                            "contains",
                            vec![RustExpr::Var("value".to_string())],
                        )),
                    })),
                }],
            )),
        ],
        tail: Some(Box::new(RustExpr::Binary {
            op: "!=".to_string(),
            left: Box::new(receiver_method_call_projection_exact(
                receiver,
                "len",
                Vec::new(),
            )),
            right: Box::new(RustExpr::Var(before.to_string())),
        })),
    }
}

fn apply_native_collection_contains_all(receiver: RustExpr, mut args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 1 {
        return compile_error_call(
            "Java Collection.containsAll(source) requires one source argument",
        );
    }
    let source = args.remove(0);
    if collection_bulk_source_aliases_receiver(&source, &receiver) {
        return RustExpr::Literal("true".to_string());
    }
    receiver_method_call_projection_exact(
        receiver_method_call_projection_exact(source, "iter", Vec::new()),
        "all",
        vec![RustExpr::Closure {
            is_move: false,
            params: vec![RustClosureParam {
                pattern: "value".to_string(),
                ty: None,
            }],
            return_type: None,
            body: RustClosureBody::Expr(Box::new(receiver_method_call_projection_exact(
                receiver,
                "contains",
                vec![RustExpr::Var("value".to_string())],
            ))),
        }],
    )
}

fn apply_native_collection_retain_all(receiver: RustExpr, mut args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 1 {
        return compile_error_call(
            "Java Collection.retainAll(source) requires one source argument",
        );
    }
    let source = args.remove(0);
    if collection_bulk_source_aliases_receiver(&source, &receiver) {
        return RustExpr::Literal("false".to_string());
    }
    let values = "__java_retain_all_values";
    let before = "__java_retain_all_before";
    RustExpr::Block {
        stmts: vec![
            RustStmt::Let {
                mutable: false,
                name: values.to_string(),
                ty: None,
                value: Some(source),
            },
            RustStmt::Let {
                mutable: false,
                name: before.to_string(),
                ty: None,
                value: Some(receiver_method_call_projection_exact(
                    receiver.clone(),
                    "len",
                    Vec::new(),
                )),
            },
            RustStmt::Expr(receiver_method_call_projection_exact(
                receiver.clone(),
                "retain",
                vec![RustExpr::Closure {
                    is_move: false,
                    params: vec![RustClosureParam {
                        pattern: "value".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: RustClosureBody::Expr(Box::new(receiver_method_call_projection_exact(
                        RustExpr::Var(values.to_string()),
                        "contains",
                        vec![RustExpr::Var("value".to_string())],
                    ))),
                }],
            )),
        ],
        tail: Some(Box::new(RustExpr::Binary {
            op: "!=".to_string(),
            left: Box::new(receiver_method_call_projection_exact(
                receiver,
                "len",
                Vec::new(),
            )),
            right: Box::new(RustExpr::Var(before.to_string())),
        })),
    }
}

fn collection_bulk_source_aliases_receiver(source: &RustExpr, receiver: &RustExpr) -> bool {
    if source == receiver {
        return true;
    }
    match source {
        RustExpr::Shaped { expr, .. } | RustExpr::PostfixComment { expr, .. } => {
            collection_bulk_source_aliases_receiver(expr, receiver)
        }
        RustExpr::MethodCall {
            receiver: clone_receiver,
            method,
            args,
        } if method == "clone" && args.is_empty() => clone_receiver.as_ref() == receiver,
        _ => false,
    }
}

pub(crate) fn apply_class_literal_token(ty: &JavaType, facts: &FactSet) -> RustExpr {
    let plan = crate::stdlib::class_literal_token_plan(ty, facts);
    apply_class_literal_token_with_rust_type(ty, plan.rust_type)
}

pub(crate) fn apply_class_literal_token_with_rust_type(
    ty: &JavaType,
    rust_type: RustType,
) -> RustExpr {
    function_call_projection(
        java_class_literal_path_projection(rust_type),
        vec![rust_string_literal_projection(
            &crate::stdlib::class_literal_java_name(ty),
        )],
    )
}

pub fn apply_new_target_with_facts(
    target: &NewTargetPlan,
    args: Vec<RustExpr>,
    facts: &FactSet,
) -> RustExpr {
    apply_new_target_with_optional_representations(target, args, facts, None)
}

pub fn apply_new_target_with_facts_and_representations(
    target: &NewTargetPlan,
    args: Vec<RustExpr>,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> RustExpr {
    apply_new_target_with_optional_representations(target, args, facts, Some(representations))
}

fn apply_new_target_with_optional_representations(
    target: &NewTargetPlan,
    args: Vec<RustExpr>,
    facts: &FactSet,
    representations: Option<&RepresentationPlan>,
) -> RustExpr {
    if let Some(native_collection) = target.native_collection {
        return apply_native_collection_new_target(
            target,
            native_collection,
            args,
            facts,
            representations,
        );
    }
    if let Some(native_string) = target.native_string {
        return apply_native_string_new_target(native_string, args);
    }
    if let Some(external_crate) = &target.external_crate {
        return function_call_projection(
            external_crate_associated_path_projection(
                &external_crate.crate_name,
                &external_crate.type_name,
                &external_crate.rust_name,
            ),
            args,
        );
    }
    if target.rust_name == "new"
        && args.len() == 1
        && boxed_primitive_constructor_owner(&target.owner_shape.java_ty)
    {
        return args
            .into_iter()
            .next()
            .expect("checked boxed primitive constructor arity");
    }
    if matches!(
        &target.owner_shape.java_ty,
        JavaType::Named { name, .. } | JavaType::Unknown(name)
            if name == "Object" || name == "java.lang.Object"
    ) {
        return java_plain_object_constructor_projection();
    }
    if shape_is_erased_object_boundary(&target.owner_shape) {
        return erased_object_constructor_projection(args);
    }
    function_call_projection(
        constructor_owner_associated_path_projection(&target.owner_shape, &target.rust_name, facts),
        args,
    )
}

fn constructor_owner_associated_path_projection(
    owner_shape: &crate::slot_shape::RustSlotShape,
    member: &str,
    facts: &FactSet,
) -> RustExpr {
    if let JavaType::Named { name, .. } | JavaType::Unknown(name) = &owner_shape.java_ty {
        let owner = crate::slot_render::rust_name_for_java_named_type(name);
        return owner_associated_path_projection(&owner, member);
    }
    rust_type_associated_path_projection(rust_type_for_slot_with_facts(owner_shape, facts), member)
}

fn boxed_primitive_constructor_owner(java_ty: &JavaType) -> bool {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            crate::stdlib::boxed_primitive_type(name.rsplit('.').next().unwrap_or(name)).is_some()
        }
        JavaType::Primitive(_)
        | JavaType::String
        | JavaType::Void
        | JavaType::Array(_)
        | JavaType::Wildcard(_) => false,
    }
}

fn apply_native_string_new_target(target: NativeStringNewTarget, args: Vec<RustExpr>) -> RustExpr {
    match target {
        NativeStringNewTarget::FromString => string_from_string_constructor_expr(args),
        NativeStringNewTarget::FromBytes => string_from_bytes_constructor_expr(args),
        NativeStringNewTarget::FromByteRange => string_from_byte_range_constructor_expr(args),
    }
}

fn string_from_string_constructor_expr(args: Vec<RustExpr>) -> RustExpr {
    let value = args
        .into_iter()
        .next()
        .expect("checked Java String constructor arity");
    receiver_method_call_projection_exact(value, "clone", Vec::new())
}

fn string_from_byte_range_constructor_expr(args: Vec<RustExpr>) -> RustExpr {
    let mut args = args.into_iter();
    let bytes = args.next().expect("checked Java String constructor arity");
    let offset = args.next().expect("checked Java String constructor arity");
    let len = args.next().expect("checked Java String constructor arity");
    string_from_byte_range_projection(bytes, offset, len)
}

fn string_from_bytes_constructor_expr(args: Vec<RustExpr>) -> RustExpr {
    let bytes = args
        .into_iter()
        .next()
        .expect("checked Java String constructor arity");
    string_from_bytes_projection(bytes)
}

fn apply_native_collection_new_target(
    target: &NewTargetPlan,
    native_collection: NativeCollectionNewTarget,
    mut args: Vec<RustExpr>,
    facts: &FactSet,
    representations: Option<&RepresentationPlan>,
) -> RustExpr {
    if matches!(
        native_collection_new_target_kind(native_collection),
        crate::stdlib::JavaCollectionKind::Set
    ) && representations
        .map(|representations| {
            crate::slot_render::java_set_uses_hash_free_carrier_with_representations(
                &target.owner_shape.java_ty,
                facts,
                representations,
            )
        })
        .unwrap_or_else(|| {
            crate::slot_render::java_set_uses_hash_free_carrier(&target.owner_shape.java_ty, facts)
        })
    {
        return match native_collection {
            NativeCollectionNewTarget::Empty { .. } => {
                native_collection_empty_new_projection(crate::stdlib::JavaCollectionKind::List)
            }
            NativeCollectionNewTarget::WithCapacity { .. } => {
                let capacity = args
                    .pop()
                    .unwrap_or_else(|| RustExpr::Literal("0".to_string()));
                native_collection_with_capacity_projection(
                    crate::stdlib::JavaCollectionKind::List,
                    capacity,
                )
            }
            NativeCollectionNewTarget::FromSource { .. } => args.pop().unwrap_or_else(|| {
                native_collection_default_constructor_projection(
                    crate::stdlib::JavaCollectionKind::List,
                )
            }),
            NativeCollectionNewTarget::FromSingleArg { .. } => args.pop().unwrap_or_else(|| {
                native_collection_default_constructor_projection(
                    crate::stdlib::JavaCollectionKind::List,
                )
            }),
        };
    }
    match native_collection {
        NativeCollectionNewTarget::Empty { kind } => {
            sequence_like_empty_native_collection_projection(target, facts)
                .unwrap_or_else(|| native_collection_empty_new_projection(kind))
        }
        NativeCollectionNewTarget::WithCapacity { kind } => {
            let capacity = args
                .pop()
                .unwrap_or_else(|| RustExpr::Literal("0".to_string()));
            native_collection_with_capacity_projection(kind, capacity)
        }
        NativeCollectionNewTarget::FromSource { kind } => args
            .pop()
            .map(|source| native_collection_from_source_projection(kind, source))
            .unwrap_or_else(|| native_collection_default_constructor_projection(kind)),
        NativeCollectionNewTarget::FromSingleArg { kind } => args
            .pop()
            .unwrap_or_else(|| native_collection_default_constructor_projection(kind)),
    }
}

fn sequence_like_empty_native_collection_projection(
    target: &NewTargetPlan,
    facts: &FactSet,
) -> Option<RustExpr> {
    let element_ty = facts.sequence_element_type_for(&target.owner_shape.java_ty)?;
    Some(function_call_projection(
        RustExpr::GenericPath {
            base: vec!["Vec".to_string()],
            args: vec![rust_type_for_java_type_with_facts(&element_ty, facts)],
            member: Some("new".to_string()),
        },
        Vec::new(),
    ))
}

fn native_collection_new_target_kind(
    target: NativeCollectionNewTarget,
) -> crate::stdlib::JavaCollectionKind {
    match target {
        NativeCollectionNewTarget::Empty { kind }
        | NativeCollectionNewTarget::WithCapacity { kind }
        | NativeCollectionNewTarget::FromSource { kind }
        | NativeCollectionNewTarget::FromSingleArg { kind } => kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit;

    #[test]
    fn class_literal_token_application_stays_structured() {
        assert_eq!(
            apply_class_literal_token(&JavaType::String, &FactSet::default()),
            RustExpr::Call {
                function: Box::new(RustExpr::GenericPath {
                    base: vec!["JavaClass".to_string()],
                    args: vec![RustType::String],
                    member: Some("literal".to_string()),
                }),
                args: vec![RustExpr::Literal("\"java.lang.String\"".to_string())],
            }
        );
    }

    #[test]
    fn optional_get_native_fallback_uses_uncaught_java_exception_adapter() {
        let receiver = Expr::Var("value".to_string());
        let target = CallTargetPlan::NativeJavaInstance {
            receiver: &receiver,
            method: NativeJavaInstanceMethod::OptionalGet,
        };

        let lowered = apply_call_target(
            &target,
            Some(RustExpr::Var("value".to_string())),
            Vec::new(),
        );

        assert_eq!(
            emit::emit_expr(&lowered),
            "value.as_ref().map(|value| (*value).clone()).ok_or_else(|| JavaException::new(\"NoSuchElementException\", \"Java Optional.get on empty value\".to_string())).unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex))"
        );
    }

    #[test]
    fn tree_map_last_key_native_fallback_uses_uncaught_java_exception_adapter() {
        let receiver = Expr::Var("values".to_string());
        let target = CallTargetPlan::NativeCollectionMethod {
            receiver: &receiver,
            receiver_ty: None,
            method: NativeCollectionMethod::MapLastKey,
        };

        let lowered = apply_call_target(
            &target,
            Some(RustExpr::Var("values".to_string())),
            Vec::new(),
        );

        assert_eq!(
            emit::emit_expr(&lowered),
            "values.keys().next_back().ok_or_else(|| JavaException::new(\"NoSuchElementException\", \"Java TreeMap.lastKey on empty map\".to_string())).map(|value| value.clone()).unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex))"
        );
    }

    #[test]
    fn string_char_at_native_fallback_uses_uncaught_java_exception_adapter() {
        let receiver = Expr::Var("value".to_string());
        let target = CallTargetPlan::NativeJavaInstance {
            receiver: &receiver,
            method: NativeJavaInstanceMethod::StringCharAt,
        };

        let lowered = apply_call_target(
            &target,
            Some(RustExpr::Var("value".to_string())),
            vec![RustExpr::Var("index".to_string())],
        );

        assert_eq!(
            emit::emit_expr(&lowered),
            "value.chars().nth((index) as usize).ok_or_else(|| JavaException::new(\"StringIndexOutOfBoundsException\", \"Java string index out of bounds\".to_string())).unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex))"
        );
    }

    #[test]
    fn integral_parse_native_fallback_maps_optional_string_arg_to_number_format_input() {
        let target = CallTargetPlan::NativeJavaStatic {
            method: NativeJavaStaticMethod::IntegerParseInt,
        };
        let lowered = apply_call_target(
            &target,
            None,
            vec![RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("s".to_string())),
                method: "as_ref".to_string(),
                args: Vec::new(),
            }],
        );

        assert_eq!(
            emit::emit_expr(&lowered),
            "s.as_ref().map(|value| std::convert::AsRef::<str>::as_ref(value)).unwrap_or(\"null\").parse::<i32>().map_err(|err| JavaException::from_display(\"NumberFormatException\", err)).unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex))"
        );
    }

    #[test]
    fn list_insert_result_projection_checks_java_index_bounds() {
        let lowered = list_insert_result_projection(
            RustExpr::Var("values".to_string()),
            RustExpr::Var("index".to_string()),
            RustExpr::Var("value".to_string()),
        );
        let rendered = emit::emit_expr(&lowered);

        assert!(
            rendered.contains("__java_list_insert_index < 0")
                && rendered.contains("__java_list_insert_index_usize > values.len()")
                && rendered.contains("IndexOutOfBoundsException")
                && rendered.contains("values.insert(__java_list_insert_index_usize, value);")
                && rendered.contains("Ok(())"),
            "{rendered}"
        );
    }

    #[test]
    fn constructor_owner_path_uses_source_type_not_storage_type() {
        let owner_shape = crate::slot_shape::RustSlotShape::for_storage(JavaType::Named {
            name: "Wrapper".to_string(),
            args: Vec::new(),
        });

        let path =
            constructor_owner_associated_path_projection(&owner_shape, "new", &FactSet::default());

        assert_eq!(emit::emit_expr(&path), "Wrapper::new");
    }
}
