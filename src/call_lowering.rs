use crate::call_plan::{
    CallTargetPlan, NativeCollectionMethod, NativeCollectionNewTarget,
    NativeCollectionStaticMethod, NativeJavaInstanceMethod, NativeJavaStaticMethod, NewTargetPlan,
};
use crate::conversion_plan::{BorrowedElementRecoveryPolicy, borrowed_element_recovery_policy};
use crate::emit;
use crate::facts::FactSet;
use crate::facts::ReceiverAbi;
use crate::java_ir::{Expr, JavaType, LambdaBody};
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
        NativeJavaInstanceMethod::StringToString => RustExpr::MethodCall {
            receiver: Box::new(receiver),
            method: "to_string".to_string(),
            args,
        },
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
        NativeJavaInstanceMethod::StringSplit => apply_string_split(receiver, args),
        NativeJavaInstanceMethod::StringSubstring => apply_string_substring(receiver, args),
        NativeJavaInstanceMethod::StringHashCode => RustExpr::Raw(format!(
            "java_string_hash_code({}.as_str())",
            emit::emit_expr(&receiver)
        )),
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

fn apply_string_substring(receiver: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    if args.is_empty() || args.len() > 2 {
        return RustExpr::Raw(
            "compile_error!(\"Java String.substring requires one or two arguments\")".to_string(),
        );
    }
    let mut args = args.into_iter();
    let begin = args.next().unwrap();
    let end = args
        .next()
        .unwrap_or_else(|| RustExpr::Raw(format!("{}.len() as i32", emit::emit_expr(&receiver))));
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("java_string_substring".to_string())),
        args: vec![
            RustExpr::Raw(format!("{}.as_str()", emit::emit_expr(&receiver))),
            begin,
            end,
        ],
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
                crate::emit::emit_expr(&arg)
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
                crate::emit::emit_expr(&arg)
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
    if args.next().is_some() {
        return RustExpr::Raw(
            "compile_error!(\"String.join varargs are not supported yet\")".to_string(),
        );
    }
    RustExpr::MethodCall {
        receiver: Box::new(values),
        method: "join".to_string(),
        args: vec![string_join_delimiter_arg(delimiter)],
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
    if owner_type == "Box<dyn std::any::Any>"
        || matches!(
            &target.owner_shape.java_ty,
            JavaType::Named { name, .. } | JavaType::Unknown(name)
                if name == "Object" || name == "java.lang.Object"
        )
    {
        return erased_object_new_expr(args);
    }
    RustExpr::Call {
        function: Box::new(RustExpr::Raw(format!(
            "{}::{}",
            owner_type, target.rust_name
        ))),
        args,
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
