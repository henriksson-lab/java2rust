//! Invocation-local capture conversion. The names never escape resolution;
//! stored call contracts recover the source wildcard after applicability has
//! been checked using distinct identities and their upper/lower bounds.
use super::*;

pub(super) struct CapturedArguments {
    pub types: Vec<JavaType>,
    pub scope: TypeEnv,
    originals: HashMap<String, JavaType>,
}

impl CapturedArguments {
    pub fn restore(&self, ty: &JavaType) -> JavaType {
        substitute_method_type_variables(ty, &self.originals)
    }

    pub fn restore_value_type(&self, ty: &JavaType) -> JavaType {
        if let JavaType::Named { name, args } = ty
            && args.is_empty()
            && self.scope.is_inference_capture(name)
            && let Some(upper) = self.scope.type_param_bound(name)
        {
            // Reading a captured value exposes its upper bound, including
            // the declaration's bound for Holder<?>. A wildcard itself is
            // a type argument, not the type of a Java expression value.
            return self.restore(upper);
        }
        self.restore(ty)
    }
}

pub(super) fn capture_arguments(
    arguments: &[JavaType],
    scope: &TypeEnv,
    symbols: &ProgramSymbols,
    candidates: &[&MethodSymbol],
) -> Option<CapturedArguments> {
    // Capture conversion is shallow (JLS 5.1.10), not a recursive widening of
    // every wildcard appearing somewhere inside an argument's type.
    if !arguments.iter().any(|ty| {
        matches!(ty, JavaType::Named { args, .. }
        if args.iter().any(|arg| matches!(arg, JavaType::Wildcard(_))))
    }) {
        return None;
    }
    let mut result = CapturedArguments {
        types: arguments.to_vec(),
        scope: scope.clone(),
        originals: HashMap::new(),
    };
    let reserved = candidates
        .iter()
        .flat_map(|method| {
            method
                .method_type_params
                .iter()
                .chain(&method.owner_type_params)
                .cloned()
        })
        .collect::<HashSet<_>>();
    let mut serial = 0;
    for ty in &mut result.types {
        let JavaType::Named { name, args } = ty else {
            continue;
        };
        let original_args = args.clone();
        for arg in args.iter_mut() {
            if !matches!(arg, JavaType::Wildcard(_)) {
                continue;
            }
            let capture = loop {
                // Uppercase spelling lets existing overload ranking recognize
                // a type variable. Scoped checking, not that ranking heuristic,
                // decides whether a conversion involving it is actually legal.
                let name = format!("JAVA_INFERENCE_CAPTURE{serial}");
                serial += 1;
                if !scope.is_type_parameter_name(&name)
                    && symbols.source_type_kind(&name).is_none()
                    && !reserved.contains(&name)
                {
                    break name;
                }
            };
            result.originals.insert(capture.clone(), arg.clone());
            *arg = JavaType::Named {
                name: capture,
                args: vec![],
            };
        }
        let owner_substitutions = symbols
            .type_params_for(name)
            .unwrap_or(&[])
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect::<HashMap<_, _>>();
        let declared_bounds = symbols.type_param_bounds_for(name).unwrap_or(&[]);
        for (index, (original, captured)) in original_args.iter().zip(args.iter()).enumerate() {
            let JavaType::Wildcard(bound) = original else {
                continue;
            };
            let JavaType::Named { name, .. } = captured else {
                unreachable!()
            };
            let declared = declared_bounds
                .get(index)
                .and_then(Option::as_ref)
                .map(|bound| substitute_method_type_variables(bound, &owner_substitutions))
                .unwrap_or_else(|| JavaType::Named {
                    name: "java.lang.Object".into(),
                    args: vec![],
                });
            let (upper, lower) = match bound {
                Some(WildcardBound::Extends(upper)) => {
                    let object_bound = matches!(&declared, JavaType::Named { name, args }
                        if args.is_empty() && matches!(name.as_str(), "Object" | "java.lang.Object"));
                    let upper = if object_bound || upper.as_ref() == &declared {
                        upper.as_ref().clone()
                    } else {
                        JavaType::Intersection(vec![upper.as_ref().clone(), declared])
                    };
                    (upper, None)
                }
                Some(WildcardBound::Super(lower)) => (declared, Some(lower.as_ref().clone())),
                None => (declared, None),
            };
            result
                .scope
                .insert_inference_capture(name.clone(), upper, lower);
        }
    }
    Some(result)
}

pub(super) fn contains_capture(ty: &JavaType, scope: &TypeEnv) -> bool {
    match ty {
        JavaType::Named { name, args } => {
            scope.is_inference_capture(name) || args.iter().any(|arg| contains_capture(arg, scope))
        }
        JavaType::Array(inner) => contains_capture(inner, scope),
        JavaType::Intersection(bounds) => bounds.iter().any(|bound| contains_capture(bound, scope)),
        JavaType::Wildcard(Some(WildcardBound::Extends(inner) | WildcardBound::Super(inner))) => {
            contains_capture(inner, scope)
        }
        _ => false,
    }
}

/// Invariant positions supply equality constraints. Process them before
/// wildcard inequalities and ordinary argument lower bounds so an earlier
/// `Consumer<? super T>` cannot choose T before `List<T>` supplies its capture.
pub(super) fn collect_invariant_substitutions(
    formal: &JavaType,
    actual: &JavaType,
    invariant: bool,
    substitutions: &mut HashMap<String, JavaType>,
    symbols: &ProgramSymbols,
    binders: &[String],
) {
    match (formal, actual) {
        (JavaType::Named { name, args }, _)
            if invariant && args.is_empty() && binders.contains(name) =>
        {
            collect_method_type_substitutions(
                formal,
                actual,
                substitutions,
                symbols,
                Some(binders),
            );
        }
        (
            JavaType::Named { name, args },
            JavaType::Named {
                name: actual_name,
                args: actual_args,
            },
        ) => {
            let projected = symbols.source_supertype_args(actual, name);
            let actual_args = if let Some(projected) = projected.as_deref() {
                projected
            } else if same_constructor_name(name, actual_name)
                || (symbols.source_type_kind(name).is_none()
                    && symbols.source_type_kind(actual_name).is_none()
                    && crate::stdlib::is_known_java_platform_subtype(actual_name, name))
            {
                actual_args
            } else {
                return;
            };
            if args.len() != actual_args.len() {
                return;
            }
            for (formal, actual) in args.iter().zip(actual_args) {
                collect_invariant_substitutions(
                    formal,
                    actual,
                    true,
                    substitutions,
                    symbols,
                    binders,
                );
            }
        }
        (JavaType::Array(formal), JavaType::Array(actual)) => {
            collect_invariant_substitutions(
                formal,
                actual,
                invariant,
                substitutions,
                symbols,
                binders,
            );
        }
        _ => {}
    }
}
