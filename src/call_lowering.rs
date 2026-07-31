use crate::call_plan::{
    CallTargetPlan, NativeCollectionMethod, NativeCollectionNewTarget,
    NativeCollectionStaticMethod, NativeJavaInstanceMethod, NativeJavaStaticMethod, NewTargetPlan,
};
use crate::coercion::{
    JavaDisplayPlan, apply_java_display_plan, arrays_as_list_projection,
    arrays_binary_search_projection, arrays_binary_search_range_projection,
    arrays_compare_projection, arrays_copy_of_projection, arrays_copy_of_range_projection,
    arrays_equals_projection, arrays_hash_code_projection, arrays_to_string_projection,
    collection_remove_if_projection, collections_empty_list_projection,
    collections_empty_map_projection, collections_empty_set_projection,
    collections_empty_sorted_set_projection, collections_n_copies_projection,
    collections_singleton_list_projection, collections_singleton_map_projection,
    collections_singleton_projection, collections_unmodifiable_projection,
    discard_value_projection, erased_object_constructor_projection,
    external_crate_associated_path_projection, external_crate_path_projection,
    facade_receiver_method_call_projection, file_exists_projection, function_call_projection,
    java_class_literal_path_projection, java_class_static_path_projection,
    java_iterator_from_collection_projection, java_list_hash_code_projection,
    java_map_hash_code_projection, java_set_hash_code_projection, java_value_hash_code_projection,
    len_i32_projection, math_abs_projection, math_floor_div_projection, math_floor_mod_projection,
    math_max_projection, math_min_projection, math_pow_projection, math_round_projection,
    native_collection_default_constructor_projection, native_collection_empty_new_projection,
    native_collection_from_source_projection, native_collection_with_capacity_projection,
    objects_equals_projection, objects_is_null_projection, objects_non_null_projection,
    optional_get_owned_value_projection, optional_or_else_owned_value_projection,
    owned_collection_to_vec_projection_for_receiver_type, owned_map_keys_vec_projection,
    owned_map_values_vec_projection, owner_associated_path_projection,
    primitive_compare_to_projection, primitive_f64_binary_method_projection,
    primitive_f64_method_projection, primitive_hash_code_projection, primitive_is_nan_projection,
    primitive_number_value_projection, primitive_parse_projection, receiver_method_call_projection,
    receiver_method_call_projection_exact, self_associated_path_projection,
    sequence_index_of_projection, string_char_at_projection,
    string_compare_to_ignore_case_projection, string_compare_to_projection,
    string_equals_ignore_case_projection, string_format_projection,
    string_from_byte_range_projection, string_from_bytes_projection, string_get_bytes_projection,
    string_hash_code_projection, string_index_of_projection, string_join_delimiter_projection,
    string_join_varargs_projection, string_last_index_of_projection, string_matches_projection,
    string_replace_all_projection, string_replace_first_projection, string_split_projection,
    string_substring_projection, string_to_char_array_projection, system_exit_projection,
    thread_sleep_millis_projection,
};
use crate::facts::FactSet;
use crate::facts::ReceiverAbi;
use crate::java_ir::{Expr, JavaType, LambdaBody, PrimitiveType};
use crate::rust_ir::RustExpr;
use crate::shape_projection::shape_is_erased_object_boundary;
use crate::slot_render::rust_type_for_slot_with_facts;
use crate::stream_lowering::{StreamLambdaValue, emit_stream_lambda_body};

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
            if owner == "String"
                && matches!(rust_name.as_str(), "valueOf" | "value_of")
                && args.len() == 1 =>
        {
            args.into_iter()
                .next()
                .expect("String.valueOf target checked one argument")
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
            function_call_projection(RustExpr::Var(rust_name.clone()), args)
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
            ..
        } => apply_external_facade_receiver_call(
            lowered_receiver.expect("external facade target must provide receiver"),
            rust_path,
            *receiver_abi,
            args,
        ),
        CallTargetPlan::SuperBoundary {
            return_type,
            propagates_result,
            ..
        } => apply_super_boundary_call(return_type, *propagates_result),
    }
}

fn apply_external_facade_receiver_call(
    receiver: RustExpr,
    rust_path: &str,
    receiver_abi: ReceiverAbi,
    args: Vec<RustExpr>,
) -> RustExpr {
    match receiver_abi {
        ReceiverAbi::FreeFunction => {
            let mut all_args = Vec::with_capacity(args.len() + 1);
            all_args.push(receiver);
            all_args.extend(args);
            function_call_projection(RustExpr::Raw(rust_path.to_string()), all_args)
        }
        ReceiverAbi::AssociatedFunction => {
            function_call_projection(RustExpr::Raw(rust_path.to_string()), args)
        }
        ReceiverAbi::BorrowedSelf | ReceiverAbi::MutBorrowedSelf | ReceiverAbi::OwnedSelf => {
            facade_receiver_method_call_projection(receiver, rust_path, args)
        }
    }
}

fn apply_super_boundary_call(return_type: &JavaType, propagates_result: bool) -> RustExpr {
    match (return_type, propagates_result) {
        (JavaType::Void, true) => RustExpr::Raw("Ok(())".to_string()),
        (JavaType::Void, false) => RustExpr::Raw("()".to_string()),
        (JavaType::Primitive(PrimitiveType::Boolean), _) => RustExpr::Literal("false".to_string()),
        (JavaType::Primitive(PrimitiveType::Char), _) => RustExpr::Literal("'\\0'".to_string()),
        (JavaType::Primitive(PrimitiveType::Byte), _)
        | (JavaType::Primitive(PrimitiveType::Short), _)
        | (JavaType::Primitive(PrimitiveType::Int), _) => RustExpr::Literal("0".to_string()),
        (JavaType::Primitive(PrimitiveType::Long), _) => RustExpr::Literal("0i64".to_string()),
        (JavaType::Primitive(PrimitiveType::Float), _) => RustExpr::Literal("0.0f32".to_string()),
        (JavaType::Primitive(PrimitiveType::Double), _) => RustExpr::Literal("0.0f64".to_string()),
        _ => RustExpr::Raw("Default::default()".to_string()),
    }
}

fn compile_error_call(message: &str) -> RustExpr {
    crate::unsupported::compile_error_expr(message)
}

fn apply_native_java_instance_method(
    receiver: RustExpr,
    method: NativeJavaInstanceMethod,
    args: Vec<RustExpr>,
) -> RustExpr {
    match method {
        NativeJavaInstanceMethod::StringLen => len_i32_projection(receiver),
        NativeJavaInstanceMethod::StringIsEmpty => {
            receiver_method_call_projection(receiver, "is_empty", args)
        }
        NativeJavaInstanceMethod::StringTrim => {
            receiver_method_call_projection(receiver, "trim", args)
        }
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
        NativeJavaInstanceMethod::BitSetGet => RustExpr::Raw("false".to_string()),
        NativeJavaInstanceMethod::FileExists => file_exists_projection(receiver),
        NativeJavaInstanceMethod::ReaderReady => RustExpr::Raw("true".to_string()),
        NativeJavaInstanceMethod::BufferPut => {
            let Some(arg) = args.into_iter().next() else {
                return RustExpr::Raw("{ }".to_string());
            };
            discard_value_projection(arg)
        }
        NativeJavaInstanceMethod::StringReplace => {
            receiver_method_call_projection(receiver, "replace", args)
        }
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
        NativeJavaInstanceMethod::OptionalGet => optional_get_owned_value_projection(receiver),
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
            RustExpr::Binary {
                op: "==".to_string(),
                left: Box::new(receiver),
                right: Box::new(other),
            }
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
    string_char_at_projection(receiver, index)
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
    if args.is_empty() || args.len() > 2 {
        return compile_error_call("Java String.substring requires one or two arguments");
    }
    let mut args = args.into_iter();
    let begin = args.next().unwrap();
    let receiver_for_len = len_i32_projection(receiver.clone());
    let end = args.next().unwrap_or(receiver_for_len);
    string_substring_projection(receiver, begin, end)
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
                return compile_error_call(&format!("Math.{java_name} requires one argument"));
            };
            primitive_f64_method_projection(arg, rust_method)
        }
        NativeJavaStaticMethod::MathBinaryF64 {
            java_name,
            rust_method,
        } => {
            let mut args = args.into_iter();
            let Some(left) = args.next() else {
                return compile_error_call(&format!("Math.{java_name} requires two arguments"));
            };
            let Some(right) = args.next() else {
                return compile_error_call(&format!("Math.{java_name} requires two arguments"));
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
            primitive_number_value_projection(arg, PrimitiveType::Int)
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
        NativeJavaStaticMethod::MathFloorMod { primitive } => {
            let mut args = args.into_iter();
            let Some(left) = args.next() else {
                return compile_error_call("Math.floorMod requires two arguments");
            };
            let Some(right) = args.next() else {
                return compile_error_call("Math.floorMod requires two arguments");
            };
            math_floor_mod_projection(left, right, primitive)
        }
        NativeJavaStaticMethod::IntegerParseInt => {
            let Some(arg) = args.into_iter().next() else {
                return compile_error_call("Integer.parseInt requires at least one argument");
            };
            primitive_parse_projection(arg, PrimitiveType::Int)
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
        NativeJavaStaticMethod::SystemGc => RustExpr::Raw("()".to_string()),
        NativeJavaStaticMethod::SystemExit => {
            let Some(status) = args.into_iter().next() else {
                return compile_error_call("System.exit requires one argument");
            };
            system_exit_projection(status)
        }
        NativeJavaStaticMethod::ThreadSleep => {
            let Some(millis) = args.into_iter().next() else {
                return compile_error_call("Thread.sleep requires one argument");
            };
            thread_sleep_millis_projection(millis)
        }
        NativeJavaStaticMethod::StringFormat => string_format_projection(args),
        NativeJavaStaticMethod::StringJoin => apply_string_join(args),
        NativeJavaStaticMethod::StringValueOf => {
            let Some(value) = args.into_iter().next() else {
                return compile_error_call("String.valueOf requires one argument");
            };
            apply_java_display_plan(value, JavaDisplayPlan::FormatValue)
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
    receiver_method_call_projection_exact(
        values,
        "join",
        vec![string_join_delimiter_projection(delimiter)],
    )
}

fn apply_native_collection_static_method(
    method: NativeCollectionStaticMethod,
    args: Vec<RustExpr>,
) -> RustExpr {
    match method {
        NativeCollectionStaticMethod::ArraysAsList => arrays_as_list_projection(args),
        NativeCollectionStaticMethod::ArraysEquals => {
            let left = args
                .first()
                .cloned()
                .unwrap_or_else(|| RustExpr::Raw("()".to_string()));
            let right = args
                .get(1)
                .cloned()
                .unwrap_or_else(|| RustExpr::Raw("()".to_string()));
            arrays_equals_projection(left, right)
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
                _ => arrays_binary_search_projection(source, RustExpr::Raw("()".to_string())),
            }
        }
        NativeCollectionStaticMethod::ArraysCompare => {
            let left = args
                .first()
                .cloned()
                .unwrap_or_else(collections_empty_list_projection);
            let right = args
                .get(1)
                .cloned()
                .unwrap_or_else(collections_empty_list_projection);
            arrays_compare_projection(left, right)
        }
        NativeCollectionStaticMethod::ArraysHashCode => args
            .first()
            .cloned()
            .map(arrays_hash_code_projection)
            .unwrap_or_else(|| RustExpr::Literal("0".to_string())),
        NativeCollectionStaticMethod::ArraysCopyOf => {
            let source = args
                .first()
                .cloned()
                .unwrap_or_else(collections_empty_list_projection);
            let len = args
                .get(1)
                .cloned()
                .unwrap_or_else(|| RustExpr::Raw("0".to_string()));
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
                .unwrap_or_else(|| RustExpr::Raw("0".to_string()));
            let to = args
                .get(2)
                .cloned()
                .unwrap_or_else(|| RustExpr::Raw("0".to_string()));
            arrays_copy_of_range_projection(source, from, to)
        }
        NativeCollectionStaticMethod::ArraysToString => args
            .first()
            .cloned()
            .map(arrays_to_string_projection)
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
            .unwrap_or_else(|| RustExpr::Raw("Default::default()".to_string())),
        NativeCollectionStaticMethod::CollectionsSingleton => {
            let value = args
                .first()
                .cloned()
                .unwrap_or_else(|| RustExpr::Raw("()".to_string()));
            collections_singleton_projection(value)
        }
        NativeCollectionStaticMethod::CollectionsSingletonList => {
            let value = args
                .first()
                .cloned()
                .unwrap_or_else(|| RustExpr::Raw("()".to_string()));
            collections_singleton_list_projection(value)
        }
        NativeCollectionStaticMethod::CollectionsSingletonMap => {
            let key = args
                .first()
                .cloned()
                .unwrap_or_else(|| RustExpr::Raw("()".to_string()));
            let value = args
                .get(1)
                .cloned()
                .unwrap_or_else(|| RustExpr::Raw("()".to_string()));
            collections_singleton_map_projection(key, value)
        }
        NativeCollectionStaticMethod::CollectionsNCopies => {
            let count = args
                .first()
                .cloned()
                .unwrap_or_else(|| RustExpr::Raw("0".to_string()));
            let value = args
                .get(1)
                .cloned()
                .unwrap_or_else(|| RustExpr::Raw("()".to_string()));
            collections_n_copies_projection(count, value)
        }
    }
}

pub fn apply_collection_remove_if(receiver: RustExpr, predicate: &Expr) -> Option<RustExpr> {
    let Expr::Lambda { params, body } = predicate else {
        return None;
    };
    let param = &params.first()?.name;
    let LambdaBody::Expr(body) = body else {
        return None;
    };
    let predicate = emit_stream_lambda_body(body, param, StreamLambdaValue::BorrowedItem)?;
    Some(collection_remove_if_projection(receiver, param, &predicate))
}

fn apply_native_collection_method(
    receiver: RustExpr,
    receiver_ty: Option<&JavaType>,
    method: NativeCollectionMethod,
    args: Vec<RustExpr>,
) -> RustExpr {
    if matches!(method, NativeCollectionMethod::Len) {
        return len_i32_projection(receiver);
    }
    if matches!(method, NativeCollectionMethod::MapKeys) {
        return owned_map_keys_vec_projection(receiver);
    }
    if matches!(method, NativeCollectionMethod::MapValues) {
        return owned_map_values_vec_projection(receiver);
    }
    if matches!(method, NativeCollectionMethod::ToArray) {
        return owned_collection_to_vec_projection_for_receiver_type(receiver, receiver_ty);
    }
    if matches!(method, NativeCollectionMethod::Iterator) {
        return java_iterator_from_collection_projection(receiver);
    }
    let rust_name = match method {
        NativeCollectionMethod::Len => "len",
        NativeCollectionMethod::IsEmpty => "is_empty",
        NativeCollectionMethod::ListHashCode => {
            return java_list_hash_code_projection(receiver);
        }
        NativeCollectionMethod::ListPush => "push",
        NativeCollectionMethod::ListGet => "get",
        NativeCollectionMethod::ListIndexOf => {
            return apply_list_index_of(receiver, args);
        }
        NativeCollectionMethod::SetHashCode => {
            return java_set_hash_code_projection(receiver);
        }
        NativeCollectionMethod::SetInsert => "insert",
        NativeCollectionMethod::Extend => "extend",
        NativeCollectionMethod::MapHashCode => {
            return java_map_hash_code_projection(receiver);
        }
        NativeCollectionMethod::MapInsert => "insert",
        NativeCollectionMethod::MapGet => "get",
        NativeCollectionMethod::MapContainsKey => "contains_key",
        NativeCollectionMethod::MapKeys => unreachable!("MapKeys is rendered as an owned key view"),
        NativeCollectionMethod::MapValues => {
            unreachable!("MapValues is rendered as an owned value view")
        }
        NativeCollectionMethod::MapIter => "iter",
        NativeCollectionMethod::Iterator => unreachable!("Iterator is rendered as JavaIterator"),
        NativeCollectionMethod::ToArray => unreachable!("ToArray is rendered as an owned Vec"),
    };
    receiver_method_call_projection_exact(receiver, rust_name, args)
}

fn apply_list_index_of(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    let Some(needle) = args.into_iter().next() else {
        return compile_error_call("Java List.indexOf requires one argument");
    };
    sequence_index_of_projection(receiver, needle)
}

pub fn apply_native_collection_extend(receiver: RustExpr, arg: RustExpr) -> RustExpr {
    receiver_method_call_projection_exact(receiver, "extend", vec![arg])
}

pub fn apply_class_literal_token(ty: &JavaType, facts: &FactSet) -> RustExpr {
    let plan = crate::stdlib::class_literal_token_plan(ty, facts);
    function_call_projection(
        java_class_literal_path_projection(&plan.rust_type),
        vec![RustExpr::Literal(format!("{:?}", plan.java_name))],
    )
}

pub fn apply_new_target(target: &NewTargetPlan, args: Vec<RustExpr>) -> RustExpr {
    apply_new_target_with_facts(target, args, &FactSet::default())
}

pub fn apply_new_target_with_facts(
    target: &NewTargetPlan,
    args: Vec<RustExpr>,
    facts: &FactSet,
) -> RustExpr {
    if let Some(native_collection) = target.native_collection {
        return apply_native_collection_new_target(target, native_collection, args, facts);
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
    let owner_type =
        crate::emit::emit_type(&rust_type_for_slot_with_facts(&target.owner_shape, facts));
    if target.rust_name == "new"
        && args.len() == 1
        && boxed_primitive_constructor_owner(&target.owner_shape.java_ty)
    {
        return args
            .into_iter()
            .next()
            .expect("checked boxed primitive constructor arity");
    }
    if shape_is_erased_object_boundary(&target.owner_shape)
        || matches!(
            &target.owner_shape.java_ty,
            JavaType::Named { name, .. } | JavaType::Unknown(name)
                if name == "Object" || name == "java.lang.Object"
        )
    {
        return erased_object_constructor_projection(args);
    }
    if owner_type == "String" && target.rust_name == "new" && matches!(args.len(), 1 | 2) {
        return string_from_bytes_constructor_expr(args);
    }
    if owner_type == "String" && target.rust_name == "new" && matches!(args.len(), 3 | 4) {
        return string_from_byte_range_constructor_expr(args);
    }
    function_call_projection(
        owner_associated_path_projection(&owner_type, &target.rust_name),
        args,
    )
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
    _target: &NewTargetPlan,
    native_collection: NativeCollectionNewTarget,
    mut args: Vec<RustExpr>,
    _facts: &FactSet,
) -> RustExpr {
    match native_collection {
        NativeCollectionNewTarget::Empty { kind } => native_collection_empty_new_projection(kind),
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
