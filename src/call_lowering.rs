use crate::call_plan::{
    CallTargetPlan, NativeCollectionMethod, NativeCollectionNewTarget,
    NativeCollectionStaticMethod, NativeJavaInstanceMethod, NativeJavaStaticMethod, NewTargetPlan,
};
use crate::conversion_plan::{BorrowedElementRecoveryPolicy, borrowed_element_recovery_policy};
use crate::emit;
use crate::facts::FactSet;
use crate::facts::ReceiverAbi;
use crate::java_ir::{Expr, JavaType, LambdaBody, PrimitiveType};
use crate::rust_ir::RustExpr;
use crate::slot_render::rust_type_for_slot_with_facts;
use crate::slot_shape::RustSlotShape;
use crate::stream_lowering::{StreamLambdaValue, emit_stream_lambda_body};

pub fn apply_call_target(
    target: &CallTargetPlan<'_>,
    lowered_receiver: Option<RustExpr>,
    args: Vec<RustExpr>,
) -> RustExpr {
    match target {
        CallTargetPlan::StaticSelf { rust_name } => RustExpr::Call {
            function: Box::new(RustExpr::Raw(format!("Self::{rust_name}"))),
            args,
        },
        CallTargetPlan::StaticOwner { owner, rust_name }
            if owner == "String"
                && matches!(rust_name.as_str(), "valueOf" | "value_of")
                && args.len() == 1 =>
        {
            args.into_iter()
                .next()
                .expect("String.valueOf target checked one argument")
        }
        CallTargetPlan::StaticOwner { owner, rust_name } => RustExpr::Call {
            function: Box::new(RustExpr::Raw(format!("{owner}::{rust_name}"))),
            args,
        },
        CallTargetPlan::InstanceReceiver { rust_name, .. } => RustExpr::MethodCall {
            receiver: Box::new(
                lowered_receiver.expect("receiver target must provide lowered receiver"),
            ),
            method: rust_name.to_string(),
            args,
        },
        CallTargetPlan::FallbackReceiver { rust_name, .. } => RustExpr::MethodCall {
            receiver: Box::new(
                lowered_receiver.expect("receiver target must provide lowered receiver"),
            ),
            method: rust_name.clone(),
            args,
        },
        CallTargetPlan::InstanceSelf { rust_name } => RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("self".to_string())),
            method: rust_name.to_string(),
            args,
        },
        CallTargetPlan::FallbackSelf { rust_name } => RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("self".to_string())),
            method: rust_name.clone(),
            args,
        },
        CallTargetPlan::FallbackFree { rust_name } => RustExpr::Call {
            function: Box::new(RustExpr::Var(rust_name.clone())),
            args,
        },
        CallTargetPlan::JavaClassStatic { rust_name } => RustExpr::Call {
            function: Box::new(RustExpr::Raw(format!("JavaClass::<()>::{rust_name}"))),
            args,
        },
        CallTargetPlan::NativeCollectionMethod { method, .. } => apply_native_collection_method(
            lowered_receiver.expect("native collection target must provide receiver"),
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
            RustExpr::Call {
                function: Box::new(RustExpr::Raw(format!("{crate_name}::{rust_name}"))),
                args: all_args,
            }
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
            RustExpr::Call {
                function: Box::new(RustExpr::Raw(rust_path.to_string())),
                args: all_args,
            }
        }
        ReceiverAbi::AssociatedFunction => RustExpr::Call {
            function: Box::new(RustExpr::Raw(rust_path.to_string())),
            args,
        },
        ReceiverAbi::BorrowedSelf | ReceiverAbi::MutBorrowedSelf | ReceiverAbi::OwnedSelf => {
            RustExpr::MethodCall {
                receiver: Box::new(receiver),
                method: facade_method_name(rust_path),
                args,
            }
        }
    }
}

fn facade_method_name(rust_path: &str) -> String {
    rust_path
        .rsplit("::")
        .next()
        .unwrap_or(rust_path)
        .to_string()
}

fn apply_super_boundary_call(return_type: &JavaType, propagates_result: bool) -> RustExpr {
    match (return_type, propagates_result) {
        (JavaType::Void, true) => RustExpr::Raw("Ok(())".to_string()),
        (JavaType::Void, false) => RustExpr::Raw("()".to_string()),
        _ => RustExpr::Raw("Default::default()".to_string()),
    }
}

pub fn propagate_result_expr(expr: RustExpr) -> RustExpr {
    if matches!(&expr, RustExpr::Raw(raw) if raw.trim_end().ends_with('?')) {
        return expr;
    }
    RustExpr::Raw(format!("{}?", crate::emit::emit_expr(&expr)))
}

fn apply_native_java_instance_method(
    receiver: RustExpr,
    method: NativeJavaInstanceMethod,
    args: Vec<RustExpr>,
) -> RustExpr {
    match method {
        NativeJavaInstanceMethod::StringLen => RustExpr::Raw(format!(
            "{}.len() as i32",
            crate::emit::emit_expr(&receiver)
        )),
        NativeJavaInstanceMethod::StringIsEmpty => RustExpr::MethodCall {
            receiver: Box::new(receiver),
            method: "is_empty".to_string(),
            args,
        },
        NativeJavaInstanceMethod::StringTrim => RustExpr::MethodCall {
            receiver: Box::new(receiver),
            method: "trim".to_string(),
            args,
        },
        NativeJavaInstanceMethod::StringToLowercase => RustExpr::MethodCall {
            receiver: Box::new(receiver),
            method: "to_lowercase".to_string(),
            args,
        },
        NativeJavaInstanceMethod::StringToUppercase => RustExpr::MethodCall {
            receiver: Box::new(receiver),
            method: "to_uppercase".to_string(),
            args,
        },
        NativeJavaInstanceMethod::StringToString => RustExpr::MethodCall {
            receiver: Box::new(receiver),
            method: "to_string".to_string(),
            args,
        },
        NativeJavaInstanceMethod::ObjectToString => RustExpr::Raw(format!(
            "java_erased_object_to_string(&{})",
            emit::emit_expr(&receiver)
        )),
        NativeJavaInstanceMethod::StringContains => RustExpr::MethodCall {
            receiver: Box::new(receiver),
            method: "contains".to_string(),
            args,
        },
        NativeJavaInstanceMethod::StringStartsWith => RustExpr::MethodCall {
            receiver: Box::new(receiver),
            method: "starts_with".to_string(),
            args,
        },
        NativeJavaInstanceMethod::StringEndsWith => RustExpr::MethodCall {
            receiver: Box::new(receiver),
            method: "ends_with".to_string(),
            args,
        },
        NativeJavaInstanceMethod::BitSetGet => RustExpr::Raw("false".to_string()),
        NativeJavaInstanceMethod::FileExists => RustExpr::Raw(format!(
            "std::path::Path::new({}.get_path().as_str()).exists()",
            emit::emit_expr(&receiver)
        )),
        NativeJavaInstanceMethod::ReaderReady => RustExpr::Raw("true".to_string()),
        NativeJavaInstanceMethod::BufferPut => {
            let Some(arg) = args.into_iter().next() else {
                return RustExpr::Raw("{ }".to_string());
            };
            RustExpr::Raw(format!("{{ let _ = {}; }}", emit::emit_expr(&arg)))
        }
        NativeJavaInstanceMethod::StringReplace => RustExpr::MethodCall {
            receiver: Box::new(receiver),
            method: "replace".to_string(),
            args,
        },
        NativeJavaInstanceMethod::StringReplaceFirst => {
            apply_string_replace_helper("java_string_replace_first", receiver, args)
        }
        NativeJavaInstanceMethod::StringReplaceAll => {
            apply_string_replace_helper("java_string_replace_all", receiver, args)
        }
        NativeJavaInstanceMethod::StringMatches => apply_string_matches_helper(receiver, args),
        NativeJavaInstanceMethod::StringSplit => apply_string_split(receiver, args),
        NativeJavaInstanceMethod::StringSubstring => apply_string_substring(receiver, args),
        NativeJavaInstanceMethod::StringToCharArray => RustExpr::Raw(format!(
            "{}.chars().collect::<Vec<char>>()",
            emit::emit_expr(&receiver)
        )),
        NativeJavaInstanceMethod::StringHashCode => RustExpr::Raw(format!(
            "java_string_hash_code({}.as_str())",
            emit::emit_expr(&receiver)
        )),
        NativeJavaInstanceMethod::StringGetBytes => RustExpr::Raw(format!(
            "{}.as_bytes().iter().map(|byte| *byte as i8).collect::<Vec<i8>>()",
            emit::emit_expr(&receiver)
        )),
        NativeJavaInstanceMethod::StringIndexOf => apply_string_index_of(receiver, args),
        NativeJavaInstanceMethod::StringCharAt => apply_string_char_at(receiver, args),
        NativeJavaInstanceMethod::StringCompareTo => {
            apply_one_arg_string_helper("java_string_compare_to", receiver, args)
        }
        NativeJavaInstanceMethod::StringCompareToIgnoreCase => {
            apply_one_arg_string_helper("java_string_compare_to_ignore_case", receiver, args)
        }
        NativeJavaInstanceMethod::StringEqualsIgnoreCase => {
            apply_one_arg_string_helper("java_string_equals_ignore_case", receiver, args)
        }
        NativeJavaInstanceMethod::OptionalIsPresent => RustExpr::MethodCall {
            receiver: Box::new(receiver),
            method: "is_some".to_string(),
            args: Vec::new(),
        },
        NativeJavaInstanceMethod::OptionalGet => RustExpr::Raw(format!(
            "{}.as_ref().map(|value| (*value).clone()).expect(\"Java null receiver\")",
            emit::emit_expr(&receiver)
        )),
        NativeJavaInstanceMethod::OptionalOrElse => {
            let Some(fallback) = args.into_iter().next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Java Optional.orElse requires one argument\")".to_string(),
                );
            };
            RustExpr::Raw(format!(
                "{}.as_ref().map(|value| (*value).clone()).unwrap_or({})",
                emit::emit_expr(&receiver),
                emit::emit_expr(&fallback)
            ))
        }
        NativeJavaInstanceMethod::PrimitiveNumberValue { target } => {
            apply_primitive_number_value(receiver, target)
        }
        NativeJavaInstanceMethod::PrimitiveHashCode { source } => {
            apply_primitive_hash_code(receiver, source)
        }
        NativeJavaInstanceMethod::PrimitiveCompareTo { source } => {
            apply_primitive_compare_to(receiver, args, source)
        }
        NativeJavaInstanceMethod::DecimalFormatFormat => {
            let Some(value) = args.into_iter().next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Java DecimalFormat.format requires one argument\")"
                        .to_string(),
                );
            };
            let value = strip_arithmetic_operand_display_conversion(&emit::emit_expr(&value));
            RustExpr::Raw(format!("{value}.to_string()"))
        }
        NativeJavaInstanceMethod::MessageFormatFormat => {
            let Some(value) = args.into_iter().next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Java MessageFormat.format requires one argument\")"
                        .to_string(),
                );
            };
            RustExpr::MethodCall {
                receiver: Box::new(receiver),
                method: "format".to_string(),
                args: vec![value],
            }
        }
        NativeJavaInstanceMethod::Equals => {
            let Some(other) = args.into_iter().next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Java equals requires one argument\")".to_string(),
                );
            };
            RustExpr::Binary {
                op: "==".to_string(),
                left: Box::new(receiver),
                right: Box::new(other),
            }
        }
    }
}

fn strip_arithmetic_operand_display_conversion(rendered: &str) -> String {
    if !(rendered.contains(" * ")
        || rendered.contains(" / ")
        || rendered.contains(" - ")
        || rendered.contains(" + "))
    {
        return rendered.to_string();
    }
    rendered.replace(".to_string()", "")
}

fn apply_primitive_number_value(receiver: RustExpr, target: PrimitiveType) -> RustExpr {
    match target {
        PrimitiveType::Byte => RustExpr::Raw(format!("{} as i8", emit::emit_expr(&receiver))),
        PrimitiveType::Short => RustExpr::Raw(format!("{} as i16", emit::emit_expr(&receiver))),
        PrimitiveType::Int => RustExpr::Raw(format!("{} as i32", emit::emit_expr(&receiver))),
        PrimitiveType::Long => RustExpr::Raw(format!("{} as i64", emit::emit_expr(&receiver))),
        PrimitiveType::Float => RustExpr::Raw(format!("{} as f32", emit::emit_expr(&receiver))),
        PrimitiveType::Double => RustExpr::Raw(format!("{} as f64", emit::emit_expr(&receiver))),
        PrimitiveType::Boolean | PrimitiveType::Char => receiver,
    }
}

fn apply_primitive_hash_code(receiver: RustExpr, source: PrimitiveType) -> RustExpr {
    let rendered = emit::emit_expr(&receiver);
    match source {
        PrimitiveType::Boolean => {
            RustExpr::Raw(format!("if {rendered} {{ 1231 }} else {{ 1237 }}"))
        }
        PrimitiveType::Byte | PrimitiveType::Short | PrimitiveType::Char | PrimitiveType::Int => {
            RustExpr::Raw(format!("{rendered} as i32"))
        }
        PrimitiveType::Long => RustExpr::Raw(format!(
            "(({rendered} as i64) ^ (({rendered} as i64) >> 32)) as i32"
        )),
        PrimitiveType::Float => RustExpr::Raw(format!("({rendered}).to_bits() as i32")),
        PrimitiveType::Double => RustExpr::Raw(format!(
            "(({rendered}).to_bits() ^ (({rendered}).to_bits() >> 32)) as i32"
        )),
    }
}

fn apply_primitive_compare_to(
    receiver: RustExpr,
    args: Vec<RustExpr>,
    _source: PrimitiveType,
) -> RustExpr {
    let Some(other) = args.into_iter().next() else {
        return RustExpr::Raw(
            "compile_error!(\"Java boxed primitive compareTo requires one argument\")".to_string(),
        );
    };
    RustExpr::Raw(format!(
        "java_primitive_compare_to({}, {})",
        emit::emit_expr(&receiver),
        emit::emit_expr(&other)
    ))
}

fn apply_string_char_at(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    let Some(index) = args.into_iter().next() else {
        return RustExpr::Raw(
            "compile_error!(\"Java String.charAt requires an index\")".to_string(),
        );
    };
    RustExpr::Raw(format!(
        "{}.chars().nth({}).expect(\"Java string index out of bounds\")",
        emit::emit_expr(&receiver),
        render_string_char_at_index(index)
    ))
}

fn render_string_char_at_index(index: RustExpr) -> String {
    let rendered = emit::emit_expr(&index);
    if rendered.ends_with(" as usize") {
        rendered
    } else if rendered.chars().all(|ch| ch.is_ascii_digit()) {
        format!("{rendered} as usize")
    } else {
        format!("({rendered}) as usize")
    }
}

fn apply_string_index_of(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    let receiver = render_string_view_argument(receiver);
    let mut args = args.into_iter();
    let Some(needle) = args.next() else {
        return RustExpr::Raw(
            "compile_error!(\"Java String.indexOf requires at least one argument\")".to_string(),
        );
    };
    let needle = render_string_view_argument(needle);
    if let Some(from_index) = args.next() {
        return RustExpr::Raw(format!(
            "java_string_index_of_from({receiver}, {needle}, {})",
            emit::emit_expr(&from_index)
        ));
    }
    RustExpr::Raw(format!("java_string_index_of({receiver}, {needle})"))
}

fn render_string_view_argument(expr: RustExpr) -> String {
    let rendered = emit::emit_expr(&expr);
    if rendered.starts_with('&') {
        rendered
    } else {
        format!("&{}", parenthesize_string_view_borrow_operand(&rendered))
    }
}

fn parenthesize_string_view_borrow_operand(rendered: &str) -> String {
    let trimmed = rendered.trim();
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        || trimmed.starts_with("self.")
        || trimmed.starts_with('"')
        || trimmed.starts_with("r#\"")
        || trimmed.starts_with("r\"")
    {
        rendered.to_string()
    } else {
        format!("({rendered})")
    }
}

fn apply_string_replace_helper(
    function: &str,
    receiver: RustExpr,
    args: Vec<RustExpr>,
) -> RustExpr {
    if args.len() != 2 {
        return RustExpr::Raw(format!(
            "compile_error!(\"Java String.{function} requires two arguments\")"
        ));
    }
    let mut args = args.into_iter();
    let regex = args.next().unwrap();
    let replacement = args.next().unwrap();
    RustExpr::Call {
        function: Box::new(RustExpr::Raw(function.to_string())),
        args: vec![
            RustExpr::Raw(format!("{}.as_str()", emit::emit_expr(&receiver))),
            regex,
            replacement,
        ],
    }
}

fn apply_string_matches_helper(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    if args.len() != 1 {
        return RustExpr::Raw(
            "compile_error!(\"Java String.matches requires one argument\")".to_string(),
        );
    }
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("java_string_matches".to_string())),
        args: vec![
            RustExpr::Raw(format!("{}.as_str()", emit::emit_expr(&receiver))),
            args.into_iter().next().unwrap(),
        ],
    }
}

fn apply_string_substring(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    if args.is_empty() || args.len() > 2 {
        return RustExpr::Raw(
            "compile_error!(\"Java String.substring requires one or two arguments\")".to_string(),
        );
    }
    let mut args = args.into_iter();
    let begin = args.next().unwrap();
    let receiver_for_len = emit::emit_expr(&receiver);
    let receiver = render_string_view_argument(receiver);
    let end = args
        .next()
        .unwrap_or_else(|| RustExpr::Raw(format!("({receiver_for_len}).len() as i32")));
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("java_string_substring".to_string())),
        args: vec![RustExpr::Raw(receiver), begin, end],
    }
}

fn apply_one_arg_string_helper(
    function: &str,
    receiver: RustExpr,
    args: Vec<RustExpr>,
) -> RustExpr {
    let Some(other) = args.into_iter().next() else {
        return RustExpr::Raw(format!(
            "compile_error!(\"Java String helper {function} requires one argument\")"
        ));
    };
    RustExpr::Call {
        function: Box::new(RustExpr::Raw(function.to_string())),
        args: vec![RustExpr::Raw(render_string_view_argument(receiver)), other],
    }
}

fn apply_string_split(receiver: RustExpr, mut args: Vec<RustExpr>) -> RustExpr {
    if args.is_empty() || args.len() > 2 {
        return RustExpr::Raw(
            "compile_error!(\"Java String.split requires one or two arguments\")".to_string(),
        );
    }
    let regex = args.remove(0);
    let limit = args
        .pop()
        .unwrap_or_else(|| RustExpr::Literal("0".to_string()));
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("java_string_split".to_string())),
        args: vec![
            RustExpr::Raw(format!("{}.as_str()", crate::emit::emit_expr(&receiver))),
            regex,
            limit,
        ],
    }
}

fn apply_native_java_static_method(
    method: NativeJavaStaticMethod,
    args: Vec<RustExpr>,
) -> RustExpr {
    match method {
        NativeJavaStaticMethod::MathMax => {
            let mut args = args.into_iter();
            let Some(left) = args.next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Math.max requires two arguments\")".to_string(),
                );
            };
            let Some(right) = args.next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Math.max requires two arguments\")".to_string(),
                );
            };
            RustExpr::Call {
                function: Box::new(RustExpr::Raw("std::cmp::max".to_string())),
                args: vec![left, right],
            }
        }
        NativeJavaStaticMethod::MathMin => {
            let mut args = args.into_iter();
            let Some(left) = args.next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Math.min requires two arguments\")".to_string(),
                );
            };
            let Some(right) = args.next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Math.min requires two arguments\")".to_string(),
                );
            };
            RustExpr::Call {
                function: Box::new(RustExpr::Raw("std::cmp::min".to_string())),
                args: vec![left, right],
            }
        }
        NativeJavaStaticMethod::MathLog => {
            let Some(arg) = args.into_iter().next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Math.log requires one argument\")".to_string(),
                );
            };
            RustExpr::Raw(format!("({} as f64).ln()", emit::emit_expr(&arg)))
        }
        NativeJavaStaticMethod::MathExp => {
            let Some(arg) = args.into_iter().next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Math.exp requires one argument\")".to_string(),
                );
            };
            RustExpr::Raw(format!("({} as f64).exp()", emit::emit_expr(&arg)))
        }
        NativeJavaStaticMethod::MathToIntExact => {
            let Some(arg) = args.into_iter().next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Math.toIntExact requires one argument\")".to_string(),
                );
            };
            RustExpr::Raw(format!("{} as i32", crate::emit::emit_expr(&arg)))
        }
        NativeJavaStaticMethod::IntegerParseInt => {
            let Some(arg) = args.into_iter().next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Integer.parseInt requires at least one argument\")"
                        .to_string(),
                );
            };
            RustExpr::Raw(format!(
                "{}.parse::<i32>().unwrap()",
                render_parse_receiver(arg)
            ))
        }
        NativeJavaStaticMethod::FloatParseFloat => {
            let Some(arg) = args.into_iter().next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Float.parseFloat requires one argument\")".to_string(),
                );
            };
            RustExpr::Raw(format!(
                "{}.parse::<f32>().unwrap()",
                render_parse_receiver(arg)
            ))
        }
        NativeJavaStaticMethod::DoubleParseDouble => {
            let Some(arg) = args.into_iter().next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Double.parseDouble requires one argument\")".to_string(),
                );
            };
            RustExpr::Raw(format!(
                "{}.parse::<f64>().unwrap()",
                render_parse_receiver(arg)
            ))
        }
        NativeJavaStaticMethod::FloatingIsNaN => {
            let Some(arg) = args.into_iter().next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Float.isNaN/Double.isNaN requires one argument\")"
                        .to_string(),
                );
            };
            RustExpr::Raw(format!("{}.is_nan()", emit::emit_expr(&arg)))
        }
        NativeJavaStaticMethod::SystemGc => RustExpr::Raw("()".to_string()),
        NativeJavaStaticMethod::SystemExit => {
            let Some(status) = args.into_iter().next() else {
                return RustExpr::Raw(
                    "compile_error!(\"System.exit requires one argument\")".to_string(),
                );
            };
            RustExpr::Raw(format!(
                "std::process::exit({} as i32)",
                emit::emit_expr(&status)
            ))
        }
        NativeJavaStaticMethod::ThreadSleep => {
            let Some(millis) = args.into_iter().next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Thread.sleep requires one argument\")".to_string(),
                );
            };
            RustExpr::Raw(format!(
                "std::thread::sleep(std::time::Duration::from_millis({} as u64))",
                emit::emit_expr(&millis)
            ))
        }
        NativeJavaStaticMethod::StringFormat => apply_string_format(args),
        NativeJavaStaticMethod::StringJoin => apply_string_join(args),
        NativeJavaStaticMethod::ObjectsEquals => {
            let mut args = args.into_iter();
            let Some(left) = args.next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Objects.equals requires two arguments\")".to_string(),
                );
            };
            let Some(right) = args.next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Objects.equals requires two arguments\")".to_string(),
                );
            };
            RustExpr::Binary {
                op: "==".to_string(),
                left: Box::new(left),
                right: Box::new(right),
            }
        }
        NativeJavaStaticMethod::ObjectsNonNull => {
            let Some(arg) = args.into_iter().next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Objects.nonNull requires one argument\")".to_string(),
                );
            };
            RustExpr::MethodCall {
                receiver: Box::new(arg),
                method: "is_some".to_string(),
                args: Vec::new(),
            }
        }
        NativeJavaStaticMethod::ObjectsIsNull => {
            let Some(arg) = args.into_iter().next() else {
                return RustExpr::Raw(
                    "compile_error!(\"Objects.isNull requires one argument\")".to_string(),
                );
            };
            RustExpr::MethodCall {
                receiver: Box::new(arg),
                method: "is_none".to_string(),
                args: Vec::new(),
            }
        }
    }
}

fn render_parse_receiver(arg: RustExpr) -> String {
    let rendered = crate::emit::emit_expr(&arg);
    let Some(stripped) = rendered.trim().strip_prefix('&') else {
        return rendered;
    };
    let stripped = stripped.trim();
    if stripped.starts_with('(') && stripped.ends_with(')') {
        stripped.to_string()
    } else if stripped
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        || stripped.starts_with("self.")
        || stripped.contains(".as_str()")
        || stripped.contains(".next_token()")
    {
        stripped.to_string()
    } else {
        format!("({stripped})")
    }
}

fn apply_string_join(args: Vec<RustExpr>) -> RustExpr {
    let mut args = args.into_iter();
    let Some(delimiter) = args.next() else {
        return RustExpr::Raw("compile_error!(\"String.join requires arguments\")".to_string());
    };
    let Some(values) = args.next() else {
        return RustExpr::Raw("compile_error!(\"String.join requires values\")".to_string());
    };
    let rest = args.collect::<Vec<_>>();
    if !rest.is_empty() {
        let mut varargs = Vec::with_capacity(rest.len() + 1);
        varargs.push(values);
        varargs.extend(rest);
        let values = varargs
            .into_iter()
            .map(string_join_vararg_value)
            .collect::<Vec<_>>()
            .join(", ");
        return RustExpr::Raw(format!(
            "vec![{values}].join({})",
            emit::emit_expr(&string_join_delimiter_arg(delimiter))
        ));
    }
    RustExpr::MethodCall {
        receiver: Box::new(values),
        method: "join".to_string(),
        args: vec![string_join_delimiter_arg(delimiter)],
    }
}

fn string_join_vararg_value(value: RustExpr) -> String {
    if let Some(literal) = string_literal_source(&value) {
        return format!("{literal:?}.to_string()");
    }
    let rendered = emit::emit_expr(&value);
    if rendered.ends_with(".to_string()") {
        rendered
    } else {
        format!("({rendered}).to_string()")
    }
}

fn string_join_delimiter_arg(delimiter: RustExpr) -> RustExpr {
    if let Some(literal) = string_literal_source(&delimiter) {
        RustExpr::Literal(format!("{literal:?}"))
    } else {
        RustExpr::Raw(format!("{}.as_str()", crate::emit::emit_expr(&delimiter)))
    }
}

fn apply_string_format(mut args: Vec<RustExpr>) -> RustExpr {
    if args.is_empty() {
        return RustExpr::Raw("String::new()".to_string());
    }
    let format_arg = args.remove(0);
    let Some(java_format) = string_literal_source(&format_arg) else {
        return RustExpr::Call {
            function: Box::new(RustExpr::Raw("java_string_format".to_string())),
            args: vec![
                format_arg,
                RustExpr::Raw(format!(
                    "vec![{}]",
                    args.into_iter()
                        .map(|arg| format!("{}.to_string()", crate::emit::emit_expr(&arg)))
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            ],
        };
    };
    let rust_format = java_format_to_rust_format(&java_format);
    let mut rendered_args = vec![RustExpr::Literal(format!("{rust_format:?}"))];
    rendered_args.extend(args);
    RustExpr::Raw(format!(
        "format!({})",
        rendered_args
            .iter()
            .map(crate::emit::emit_expr)
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn string_literal_source(expr: &RustExpr) -> Option<String> {
    let raw = crate::emit::emit_expr(expr);
    let raw = raw.strip_suffix(".to_string()").unwrap_or(&raw);
    if !(raw.starts_with('"') && raw.ends_with('"')) {
        return None;
    }
    Some(raw[1..raw.len() - 1].to_string())
}

fn java_format_to_rust_format(format: &str) -> String {
    let mut out = String::new();
    let mut chars = format.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        if matches!(chars.peek(), Some('%')) {
            chars.next();
            out.push('%');
            continue;
        }

        let mut spec = String::new();
        while let Some(next) = chars.peek().copied() {
            if next.is_ascii_alphabetic() {
                chars.next();
                match next {
                    's' | 'd' | 'b' | 'c' => out.push_str("{}"),
                    'f' => out.push_str(&rust_float_format(&spec)),
                    _ => out.push_str("{}"),
                }
                break;
            }
            spec.push(next);
            chars.next();
        }
    }
    out
}

fn rust_float_format(spec: &str) -> String {
    if let Some(dot) = spec.find('.') {
        let width = spec[..dot]
            .chars()
            .filter(|ch| ch.is_ascii_digit())
            .collect::<String>();
        let precision = spec[dot + 1..]
            .chars()
            .filter(|ch| ch.is_ascii_digit())
            .collect::<String>();
        return match (width.is_empty(), precision.is_empty()) {
            (true, true) => "{}".to_string(),
            (true, false) => format!("{{:.{precision}}}"),
            (false, true) => format!("{{:{width}}}"),
            (false, false) => format!("{{:{width}.{precision}}}"),
        };
    }
    let width = spec
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if width.is_empty() {
        "{}".to_string()
    } else {
        format!("{{:{width}}}")
    }
}

fn apply_native_collection_static_method(
    method: NativeCollectionStaticMethod,
    args: Vec<RustExpr>,
) -> RustExpr {
    match method {
        NativeCollectionStaticMethod::ArraysAsList => RustExpr::Raw(format!(
            "vec![{}]",
            args.iter()
                .map(crate::emit::emit_expr)
                .collect::<Vec<_>>()
                .join(", ")
        )),
        NativeCollectionStaticMethod::ArraysEquals => {
            let left = args
                .first()
                .map(crate::emit::emit_expr)
                .unwrap_or_else(|| "()".to_string());
            let right = args
                .get(1)
                .map(crate::emit::emit_expr)
                .unwrap_or_else(|| "()".to_string());
            RustExpr::Raw(format!("{left} == {right}"))
        }
        NativeCollectionStaticMethod::CollectionsEmptyList => {
            RustExpr::Raw("Vec::new()".to_string())
        }
        NativeCollectionStaticMethod::CollectionsEmptySortedSet => {
            RustExpr::Raw("std::collections::BTreeSet::new()".to_string())
        }
        NativeCollectionStaticMethod::CollectionsSingleton => {
            let value = args
                .first()
                .map(crate::emit::emit_expr)
                .unwrap_or_else(|| "()".to_string());
            RustExpr::Raw(format!("std::collections::HashSet::from([{value}])"))
        }
        NativeCollectionStaticMethod::CollectionsSingletonList => {
            let value = args
                .first()
                .map(crate::emit::emit_expr)
                .unwrap_or_else(|| "()".to_string());
            RustExpr::Raw(format!("vec![{value}]"))
        }
        NativeCollectionStaticMethod::CollectionsSingletonMap => {
            let key = args
                .first()
                .map(crate::emit::emit_expr)
                .unwrap_or_else(|| "()".to_string());
            let value = args
                .get(1)
                .map(crate::emit::emit_expr)
                .unwrap_or_else(|| "()".to_string());
            RustExpr::Raw(format!(
                "std::collections::HashMap::from([({key}, {value})])"
            ))
        }
        NativeCollectionStaticMethod::CollectionsNCopies => {
            let count = args
                .first()
                .map(crate::emit::emit_expr)
                .unwrap_or_else(|| "0".to_string());
            let value = args
                .get(1)
                .map(crate::emit::emit_expr)
                .unwrap_or_else(|| "()".to_string());
            RustExpr::Raw(format!("vec![{value}; {count} as usize]"))
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
    Some(RustExpr::Raw(format!(
        "{}.retain(|{}| !({}))",
        crate::emit::emit_expr(&receiver),
        param,
        predicate
    )))
}

fn apply_native_collection_method(
    receiver: RustExpr,
    method: NativeCollectionMethod,
    args: Vec<RustExpr>,
) -> RustExpr {
    if matches!(method, NativeCollectionMethod::Len) {
        return RustExpr::Raw(format!(
            "{}.len() as i32",
            crate::emit::emit_expr(&receiver)
        ));
    }
    if matches!(method, NativeCollectionMethod::MapKeys) {
        return RustExpr::Raw(format!(
            "{}.keys().cloned().collect::<Vec<_>>()",
            crate::emit::emit_expr(&receiver)
        ));
    }
    if matches!(method, NativeCollectionMethod::ToArray) {
        return RustExpr::Raw(format!(
            "{}.iter().cloned().collect::<Vec<_>>()",
            crate::emit::emit_expr(&receiver)
        ));
    }
    if matches!(method, NativeCollectionMethod::Iterator) {
        return RustExpr::Raw(format!(
            "JavaIterator::new({}.iter().cloned())",
            crate::emit::emit_expr(&receiver)
        ));
    }
    let rust_name = match method {
        NativeCollectionMethod::Len => "len",
        NativeCollectionMethod::IsEmpty => "is_empty",
        NativeCollectionMethod::ListPush => "push",
        NativeCollectionMethod::ListGet => "get",
        NativeCollectionMethod::SetInsert => "insert",
        NativeCollectionMethod::Extend => "extend",
        NativeCollectionMethod::MapInsert => "insert",
        NativeCollectionMethod::MapGet => "get",
        NativeCollectionMethod::MapContainsKey => "contains_key",
        NativeCollectionMethod::MapKeys => unreachable!("MapKeys is rendered as an owned key view"),
        NativeCollectionMethod::MapIter => "iter",
        NativeCollectionMethod::Iterator => unreachable!("Iterator is rendered as JavaIterator"),
        NativeCollectionMethod::ToArray => unreachable!("ToArray is rendered as an owned Vec"),
    };
    RustExpr::MethodCall {
        receiver: Box::new(receiver),
        method: rust_name.to_string(),
        args,
    }
}

pub fn apply_native_collection_extend(receiver: RustExpr, arg: RustExpr) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(receiver),
        method: "extend".to_string(),
        args: vec![arg],
    }
}

pub fn borrowed_sequence_extend_arg(
    arg: RustExpr,
    element_shape: &RustSlotShape,
    facts: &FactSet,
) -> RustExpr {
    let arg = emit::emit_expr(&arg);
    match borrowed_element_recovery_policy(element_shape, facts) {
        BorrowedElementRecoveryPolicy::BorrowElement => RustExpr::Raw(format!("{arg}.iter()")),
        BorrowedElementRecoveryPolicy::StringViewToOwned => {
            RustExpr::Raw(format!("{arg}.iter().map(|v| v.to_string())"))
        }
        BorrowedElementRecoveryPolicy::CopyElements => {
            RustExpr::Raw(format!("{arg}.iter().copied()"))
        }
        BorrowedElementRecoveryPolicy::CloneElements { .. } => {
            RustExpr::Raw(format!("{arg}.iter().cloned()"))
        }
        BorrowedElementRecoveryPolicy::Unsupported { .. } => RustExpr::Raw(format!(
            "{arg}.iter().cloned() /* java2rust: unsupported borrowed element recovery for extend */"
        )),
    }
}

pub fn apply_class_literal_token(ty: &JavaType, facts: &FactSet) -> RustExpr {
    let plan = crate::stdlib::class_literal_token_plan(ty, facts);
    RustExpr::Call {
        function: Box::new(RustExpr::Raw(format!(
            "JavaClass::<{}>::literal",
            plan.rust_type
        ))),
        args: vec![RustExpr::Literal(format!("{:?}", plan.java_name))],
    }
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
        return RustExpr::Call {
            function: Box::new(RustExpr::Raw(format!(
                "{}::{}::{}",
                external_crate.crate_name, external_crate.type_name, external_crate.rust_name
            ))),
            args,
        };
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
    if owner_type == "Box<dyn std::any::Any>"
        || matches!(
            &target.owner_shape.java_ty,
            JavaType::Named { name, .. } | JavaType::Unknown(name)
                if name == "Object" || name == "java.lang.Object"
        )
    {
        return erased_object_new_expr(args);
    }
    if owner_type == "String" && target.rust_name == "new" && matches!(args.len(), 1 | 2) {
        return java_string_from_bytes_expr(args);
    }
    if owner_type == "String" && target.rust_name == "new" && matches!(args.len(), 3 | 4) {
        return java_string_from_byte_range_expr(args);
    }
    RustExpr::Call {
        function: Box::new(RustExpr::Raw(format!(
            "{}::{}",
            owner_type, target.rust_name
        ))),
        args,
    }
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

fn java_string_from_byte_range_expr(args: Vec<RustExpr>) -> RustExpr {
    let mut args = args.into_iter();
    let bytes = args.next().expect("checked Java String constructor arity");
    let offset = args.next().expect("checked Java String constructor arity");
    let len = args.next().expect("checked Java String constructor arity");
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("java_string_from_byte_range".to_string())),
        args: vec![
            RustExpr::Raw(format!("&{}", crate::emit::emit_expr(&bytes))),
            offset,
            len,
        ],
    }
}

fn java_string_from_bytes_expr(args: Vec<RustExpr>) -> RustExpr {
    let bytes = args
        .into_iter()
        .next()
        .expect("checked Java String constructor arity");
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("java_string_from_bytes".to_string())),
        args: vec![RustExpr::Raw(format!(
            "&{}",
            crate::emit::emit_expr(&bytes)
        ))],
    }
}

fn erased_object_new_expr(args: Vec<RustExpr>) -> RustExpr {
    let value = match args.as_slice() {
        [] => "()".to_string(),
        [single] => crate::emit::emit_expr(single),
        _ => format!(
            "({})",
            args.iter()
                .map(crate::emit::emit_expr)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    RustExpr::Raw(format!("Box::new({value}) as Box<dyn std::any::Any>"))
}

fn apply_native_collection_new_target(
    _target: &NewTargetPlan,
    native_collection: NativeCollectionNewTarget,
    mut args: Vec<RustExpr>,
    _facts: &FactSet,
) -> RustExpr {
    match native_collection {
        NativeCollectionNewTarget::Empty { kind } => RustExpr::Call {
            function: Box::new(RustExpr::Raw(format!(
                "{}::new",
                native_collection_type_name(kind)
            ))),
            args: Vec::new(),
        },
        NativeCollectionNewTarget::WithCapacity { kind } => {
            let capacity = args
                .pop()
                .unwrap_or_else(|| RustExpr::Literal("0".to_string()));
            RustExpr::Raw(format!(
                "{}::with_capacity({} as usize)",
                native_collection_type_name(kind),
                emit::emit_expr(&capacity)
            ))
        }
        NativeCollectionNewTarget::FromSource { kind } => args.pop().unwrap_or_else(|| {
            RustExpr::Raw(format!("{}::new()", native_collection_type_name(kind)))
        }),
        NativeCollectionNewTarget::FromSingleArg { kind } => args.pop().unwrap_or_else(|| {
            RustExpr::Raw(format!("{}::new()", native_collection_type_name(kind)))
        }),
    }
}

fn native_collection_type_name(kind: crate::stdlib::JavaCollectionKind) -> &'static str {
    match kind {
        crate::stdlib::JavaCollectionKind::List => "Vec",
        crate::stdlib::JavaCollectionKind::Set => "std::collections::HashSet",
        crate::stdlib::JavaCollectionKind::Map => "std::collections::HashMap",
    }
}
