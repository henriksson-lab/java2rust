use crate::java_ir::{
    CallKind, ConstructorDecl, JavaType, MethodDecl, Modifier, Param, Program, TypeDecl,
};

/// Map an actual argument to the selected formal's physical storage domain.
/// Resolution retains formal array types and per-actual conversion targets;
/// their difference proves variable-arity packing without guessing from syntax.
pub(crate) fn resolved_argument_storage_projection(
    call: &crate::java_ir::ResolvedCall,
    actual_index: usize,
) -> Option<(usize, Vec<usize>, JavaType, JavaType)> {
    let last = call.arg_target_types.len().checked_sub(1)?;
    if actual_index >= last
        && let JavaType::Array(element) = &call.arg_target_types[last]
        && crate::call_plan::resolved_call_arg_target_type(call, actual_index)
            == Some(element.as_ref())
    {
        return Some((
            last,
            vec![0],
            *element.clone(),
            call.arg_target_types[last].clone(),
        ));
    }
    let ty = call.arg_target_types.get(actual_index)?.clone();
    Some((actual_index, vec![], ty.clone(), ty))
}

/// Recover the exact source declaration behind a resolved Rust member name.
/// Shared by invocation graphs and source callback contracts: lexical owner
/// lookup must not select a same-leaf declaration from another package.
pub(crate) fn resolved_source_method_decl<'a>(
    program: &'a Program,
    current_owner: &str,
    call: &crate::java_ir::ResolvedCall,
) -> Option<(&'a TypeDecl, &'a MethodDecl)> {
    let owner = crate::inherited_members::find_type_decl_for_owner_context(
        program,
        current_owner,
        &call.declaring_type,
    )?;
    let mut methods = owner.members.iter().filter_map(|member| match member {
        crate::java_ir::Member::Method(method)
            if source_method_rust_name(method) == call.rust_name =>
        {
            Some(method)
        }
        _ => None,
    });
    let method = methods.next()?;
    methods.next().is_none().then_some((owner, method))
}

/// Return the Rust member name used to emit a source constructor.
///
/// Resolved primary declarations normally cache this name on the IR, while
/// emitted context declarations may not. Derive the latter from the same
/// overload rule instead of treating every uncached constructor as `new`.
pub(crate) fn source_constructor_rust_name(
    owner: &TypeDecl,
    constructor: &ConstructorDecl,
) -> String {
    let constructor_count = owner
        .members
        .iter()
        .filter(|member| matches!(member, crate::java_ir::Member::Constructor(_)))
        .count();
    source_constructor_rust_name_with_count(constructor, constructor_count)
}

pub(crate) fn source_constructor_rust_name_with_count(
    constructor: &ConstructorDecl,
    constructor_count: usize,
) -> String {
    if let Some(rust_name) = &constructor.rust_name
        && !(rust_name == "new" && constructor_count > 1 && !constructor.params.is_empty())
    {
        return rust_name.clone();
    }
    if constructor.params.is_empty() || constructor_count == 1 {
        return "new".to_string();
    }
    let suffix = constructor
        .params
        .iter()
        .map(|param| crate::parser::type_suffix(&param.ty, param.is_varargs))
        .collect::<Vec<_>>()
        .join("_");
    format!("new_{suffix}")
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JavaOwnerKey(String);

#[cfg(test)]
mod owner_identity_tests {
    #[test]
    fn merged_program_owner_uses_retained_compilation_unit_identity() {
        let mut parsed = crate::parser::parse_java_source("package origin; class Value<T> {}");
        parsed.program.package = Some("unrelated".into());
        let ty = &parsed.program.types[0];
        assert_eq!(
            super::JavaOwnerKey::source_type_in_program(&parsed.program, ty).as_str(),
            "origin.Value"
        );
        let mut renamed = ty.clone();
        renamed.name = "origin.JavaType.Value".into();
        assert_eq!(
            super::JavaOwnerKey::source_type_in_program(&parsed.program, &renamed).as_str(),
            "origin.JavaType.Value"
        );
    }
}

impl JavaOwnerKey {
    /// The canonical source owner includes the compilation unit package.
    ///
    /// `TypeDecl` intentionally does not duplicate `Program::package`, so
    /// callers that have both must use this constructor rather than silently
    /// collapsing `a.Item` and `b.Item` to the same simple name.
    pub(crate) fn source_type_in_program(program: &Program, ty: &TypeDecl) -> Self {
        // Batch construction qualifies colliding peers in `TypeDecl::name`
        // before merging their compilation units.  That spelling is already
        // canonical and must not acquire the package of the first merged unit
        // (which would turn `b.Item` into `a.b.Item`).
        if ty.name.contains('.') {
            return Self(ty.name.clone());
        }
        // A merged program retains just one package field. Non-colliding
        // declarations still keep their original per-file source identity.
        if let Some(names) = &ty.source_names {
            return Self(names.canonical.clone());
        }
        match &program.package {
            Some(package) if !package.is_empty() => Self(format!("{package}.{}", ty.name)),
            _ => Self(ty.name.clone()),
        }
    }

    /// Compatibility constructor for contexts that genuinely do not retain a
    /// compilation unit. New source-program key construction should use
    /// [`Self::source_type_in_program`].
    pub(crate) fn source_type(ty: &TypeDecl) -> Self {
        Self(ty.name.clone())
    }

    pub(crate) fn resolved_source(owner: impl Into<String>) -> Self {
        Self(owner.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct JavaCallableKey {
    owner: JavaOwnerKey,
    java_name: String,
    erased_param_types: Vec<String>,
    kind: JavaCallableKind,
    staticness: JavaCallableStaticness,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct JavaCallableSlotKey {
    java_name: String,
    erased_param_types: Vec<String>,
    kind: JavaCallableKind,
    staticness: JavaCallableStaticness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum JavaCallableKind {
    Method,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum JavaCallableStaticness {
    Static,
    Instance,
}

impl JavaCallableKey {
    pub(crate) fn source_method_in_program(
        program: &Program,
        ty: &TypeDecl,
        method: &MethodDecl,
    ) -> Self {
        Self::source_method_for_owner(JavaOwnerKey::source_type_in_program(program, ty), method)
    }

    pub(crate) fn source_method_for_type(ty: &TypeDecl, method: &MethodDecl) -> Self {
        Self::source_method_for_owner(JavaOwnerKey::source_type(ty), method)
    }

    pub(crate) fn source_method_for_owner(owner: JavaOwnerKey, method: &MethodDecl) -> Self {
        Self::source_method(owner, method)
    }

    pub(crate) fn resolved_source_method(
        owner: JavaOwnerKey,
        java_name: impl Into<String>,
        arg_target_types: &[JavaType],
        kind: CallKind,
    ) -> Self {
        Self {
            owner,
            java_name: java_name.into(),
            erased_param_types: arg_target_types
                .iter()
                .map(|ty| erased_type_name(ty, false))
                .collect(),
            kind: JavaCallableKind::Method,
            staticness: match kind {
                CallKind::Static => JavaCallableStaticness::Static,
                CallKind::Instance => JavaCallableStaticness::Instance,
            },
        }
    }

    fn source_method(owner: JavaOwnerKey, method: &MethodDecl) -> Self {
        Self {
            owner,
            java_name: method.name.clone(),
            erased_param_types: erased_param_types(&method.params),
            kind: JavaCallableKind::Method,
            staticness: method_staticness(method),
        }
    }

    pub(crate) fn slot(&self) -> JavaCallableSlotKey {
        JavaCallableSlotKey {
            java_name: self.java_name.clone(),
            erased_param_types: self.erased_param_types.clone(),
            kind: self.kind,
            staticness: self.staticness,
        }
    }

    pub(crate) fn owner(&self) -> &str {
        self.owner.as_str()
    }

    pub(crate) fn java_name(&self) -> &str {
        &self.java_name
    }

    pub(crate) fn is_instance(&self) -> bool {
        matches!(self.staticness, JavaCallableStaticness::Instance)
    }
}

impl JavaCallableSlotKey {
    pub(crate) fn source_method(method: &MethodDecl) -> Self {
        JavaCallableKey::source_method(JavaOwnerKey::resolved_source(""), method).slot()
    }

    /// Compare an inherited member in its target owner's generic frame while
    /// retaining the original declaration identity for resolution/emission.
    pub(crate) fn source_method_with_substitutions(
        method: &MethodDecl,
        substitutions: &std::collections::HashMap<String, JavaType>,
    ) -> Self {
        let substitutions = crate::java_type_substitution::without_shadowed_binders(
            substitutions,
            &method.type_params,
        );
        let mut key = Self::source_method(method);
        if !substitutions.is_empty() {
            key.erased_param_types = method
                .params
                .iter()
                .map(|param| {
                    erased_type_name(
                        &crate::resolve::substitute_method_type_variables(
                            &param.ty,
                            &substitutions,
                        ),
                        param.is_varargs,
                    )
                })
                .collect();
        }
        key
    }

    pub(crate) fn matches_with_type_aliases(&self, other: &Self) -> bool {
        source_callable_names_match(&self.java_name, &other.java_name)
            && self.kind == other.kind
            && self.staticness == other.staticness
            && self.erased_param_types.len() == other.erased_param_types.len()
            && self
                .erased_param_types
                .iter()
                .zip(&other.erased_param_types)
                .all(|(left, right)| erased_type_names_match_with_aliases(left, right))
    }

    /// Whether two source callables are in the same overload family and have
    /// the same arity.  This is deliberately weaker than type matching and is
    /// only safe when the caller proves that the candidate is unique.
    pub(crate) fn matches_header_and_arity(&self, other: &Self) -> bool {
        source_callable_names_match(&self.java_name, &other.java_name)
            && self.kind == other.kind
            && self.staticness == other.staticness
            && self.erased_param_types.len() == other.erased_param_types.len()
    }

    /// Reject unrelated declarations before computing ancestor substitutions
    /// and erased parameter types. This is only a prefilter: callers must
    /// still compare the complete specialized slot for overload selection.
    pub(crate) fn has_matching_source_method_header(&self, method: &MethodDecl) -> bool {
        source_callable_names_match(&self.java_name, &method.name)
            && self.kind == JavaCallableKind::Method
            && self.staticness == method_staticness(method)
            && self.erased_param_types.len() == method.params.len()
    }

    pub(crate) fn arity(&self) -> usize {
        self.erased_param_types.len()
    }
}

fn source_callable_names_match(left: &str, right: &str) -> bool {
    left == right || crate::parser::to_snake_case(left) == crate::parser::to_snake_case(right)
}

pub(crate) fn erased_type_names_match_with_aliases(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    let (left_base, left_suffix) = split_erased_array_suffix(left);
    let (right_base, right_suffix) = split_erased_array_suffix(right);
    if left_suffix != right_suffix {
        return false;
    }
    // A simple-name compatibility check is useful when one declaration was
    // flattened for Rust emission, but it must never erase a disagreement
    // between two source-level package qualifications.
    if source_name_has_package_prefix(left_base) && source_name_has_package_prefix(right_base) {
        return false;
    }
    // External API aliases intentionally include the leaf of a nested name.
    // Source slots must retain both explicit owners: A.Builder and B.Builder
    // cannot become one declaration merely because each has a Builder leaf.
    // Reuse the same source suffix normalization for dotted, binary and
    // already-flattened nested identities.
    let left_source = flattened_source_type_suffix(left_base);
    let right_source = flattened_source_type_suffix(right_base);
    if left_source.contains('_') && right_source.contains('_') {
        return left_source == right_source;
    }
    crate::facts::external_type_name_matches(left_base, right_base)
        || nested_source_type_suffix_alias_matches(left_base, right_base)
}

pub(crate) fn nested_source_type_suffix_alias_matches(left: &str, right: &str) -> bool {
    if source_name_has_package_prefix(left)
        && source_name_has_package_prefix(right)
        && source_type_package_prefix(left) != source_type_package_prefix(right)
    {
        return false;
    }
    flattened_source_type_suffix(left) == flattened_source_type_suffix(right)
}

fn source_name_has_package_prefix(name: &str) -> bool {
    name.split(['.', '$'])
        .next()
        .and_then(|segment| segment.chars().next())
        .is_some_and(char::is_lowercase)
}

fn source_type_package_prefix(name: &str) -> String {
    name.split(['.', '$'])
        .take_while(|segment| !segment.chars().next().is_some_and(char::is_uppercase))
        .collect::<Vec<_>>()
        .join(".")
}

fn flattened_source_type_suffix(name: &str) -> String {
    let segments = name.split(['.', '$']).collect::<Vec<_>>();
    let first_type = segments
        .iter()
        .position(|segment| segment.chars().next().is_some_and(char::is_uppercase))
        .unwrap_or(0);
    segments[first_type..].join("_")
}

fn split_erased_array_suffix(name: &str) -> (&str, &str) {
    let suffix_start = name
        .find('[')
        .or_else(|| name.find("..."))
        .unwrap_or(name.len());
    name.split_at(suffix_start)
}

#[cfg(test)]
mod erased_alias_tests {
    use super::*;

    #[test]
    fn cheap_source_header_filter_matches_the_existing_slot_header_policy() {
        let parsed = crate::parser::parse_java_source(
            "class Header<T> { void runNow(T value) {} void run_now(String value) {} \
             void runNow() {} static void runNow(int value) {} void other(T value) {} \
             void runNow(T... values) {} }",
        );
        let methods = parsed.program.types[0]
            .members
            .iter()
            .filter_map(|member| match member {
                crate::java_ir::Member::Method(method) => Some(method),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(methods.len(), 6);
        let substitutions = std::collections::HashMap::from([("T".to_string(), JavaType::String)]);
        for left in &methods {
            let slot = JavaCallableSlotKey::source_method_with_substitutions(left, &substitutions);
            for right in &methods {
                assert_eq!(
                    slot.has_matching_source_method_header(right),
                    slot.matches_header_and_arity(&JavaCallableSlotKey::source_method(right)),
                );
            }
        }
    }

    #[test]
    fn nested_source_aliases_preserve_owners_and_array_rank() {
        for (left, right) in [
            ("First.Builder", "Second.Builder"),
            ("First_Builder", "Second.Builder"),
            ("p.First.Builder[]", "Second$Builder[]"),
            ("First.Builder[]", "First_Builder[][]"),
        ] {
            assert!(
                !erased_type_names_match_with_aliases(left, right),
                "{left} vs {right}"
            );
        }
        for (left, right) in [
            ("First.Builder", "First_Builder"),
            ("p.First$Builder[]", "First_Builder[]"),
            ("p.First.Builder", "First.Builder"),
            ("First.Builder", "Builder"),
        ] {
            assert!(
                erased_type_names_match_with_aliases(left, right),
                "{left} vs {right}"
            );
        }
    }

    #[test]
    fn flattened_nested_source_name_matches_qualified_java_name() {
        assert!(crate::facts::external_type_name_matches(
            "org.biojava.nbio.alignment.routines.AlignerHelper",
            "AlignerHelper"
        ));
        assert!(erased_type_names_match_with_aliases(
            "AlignerHelper_Subproblem",
            "org.biojava.nbio.alignment.routines.AlignerHelper.Subproblem"
        ));
        assert!(!erased_type_names_match_with_aliases(
            "a.AlignerHelper.Subproblem",
            "b.AlignerHelper.Subproblem"
        ));
    }
}

pub(crate) fn erased_param_types(params: &[Param]) -> Vec<String> {
    params
        .iter()
        .map(|param| erased_type_name(&param.ty, param.is_varargs))
        .collect()
}

/// Erase scoped binders through their declared upper bounds. The caller's
/// map already applies method-over-owner shadowing; F-bounds and cycles must
/// not turn binder spelling into an overload identity.
pub(crate) fn erased_type_name_with_bounds(
    ty: &JavaType,
    bounds: &std::collections::HashMap<String, JavaType>,
) -> String {
    fn erase(
        ty: &JavaType,
        bounds: &std::collections::HashMap<String, JavaType>,
        visiting: &mut std::collections::HashSet<String>,
    ) -> String {
        match ty {
            JavaType::Named { name, args } if args.is_empty() && bounds.contains_key(name) => {
                if !visiting.insert(name.clone()) {
                    return "java.lang.Object".into();
                }
                let out = erase(&bounds[name], bounds, visiting);
                visiting.remove(name);
                out
            }
            JavaType::Array(inner) => format!("{}[]", erase(inner, bounds, visiting)),
            JavaType::Intersection(items) => items
                .first()
                .map(|ty| erase(ty, bounds, visiting))
                .unwrap_or_else(|| "java.lang.Object".into()),
            _ => erased_type_name(ty, false),
        }
    }
    erase(ty, bounds, &mut std::collections::HashSet::new())
}

pub(crate) fn erased_type_name(ty: &JavaType, is_varargs: bool) -> String {
    let mut erased = match ty {
        JavaType::Void => "void".to_string(),
        JavaType::Primitive(primitive) => format!("{primitive:?}").to_ascii_lowercase(),
        JavaType::String => "java.lang.String".to_string(),
        JavaType::Array(element) => format!("{}[]", erased_type_name(element, false)),
        JavaType::Named { name, .. } => name.clone(),
        JavaType::Intersection(bounds) => bounds
            .first()
            .map(|bound| erased_type_name(bound, false))
            .unwrap_or_else(|| "java.lang.Object".to_string()),
        JavaType::Wildcard(bound) => bound
            .as_ref()
            .map(|bound| match bound {
                crate::java_ir::WildcardBound::Extends(ty)
                | crate::java_ir::WildcardBound::Super(ty) => erased_type_name(ty, false),
            })
            .unwrap_or_else(|| "java.lang.Object".to_string()),
        JavaType::Unknown(raw) => raw.clone(),
    };
    if is_varargs {
        erased.push_str("...");
    }
    erased
}

pub(crate) fn method_staticness(method: &MethodDecl) -> JavaCallableStaticness {
    if method.modifiers.contains(&Modifier::Static) {
        JavaCallableStaticness::Static
    } else {
        JavaCallableStaticness::Instance
    }
}

pub(crate) fn source_method_rust_name(method: &MethodDecl) -> String {
    method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name))
}

pub(crate) fn source_method_overload_rust_name(method: &MethodDecl) -> String {
    let base = crate::parser::to_snake_case(&method.name);
    if method.params.is_empty() {
        return base;
    }
    let suffix = method
        .params
        .iter()
        .map(|param| crate::parser::type_suffix(&param.ty, param.is_varargs))
        .collect::<Vec<_>>()
        .join("_");
    format!("{base}_{suffix}")
}

#[cfg(test)]
mod bound_erasure_tests {
    use super::*;
    use std::collections::HashMap;
    fn named(name: &str) -> JavaType {
        JavaType::Named {
            name: name.into(),
            args: Vec::new(),
        }
    }
    #[test]
    fn bound_erasure_handles_arrays_alpha_binders_and_cycles() {
        let mut bounds = HashMap::from([
            ("T".into(), named("IntConsumer")),
            ("U".into(), named("IntConsumer")),
            ("A".into(), named("B")),
            ("B".into(), named("A")),
        ]);
        assert_eq!(
            erased_type_name_with_bounds(&named("T"), &bounds),
            erased_type_name_with_bounds(&named("U"), &bounds)
        );
        assert_eq!(
            erased_type_name_with_bounds(&JavaType::Array(Box::new(named("T"))), &bounds),
            "IntConsumer[]"
        );
        assert_eq!(
            erased_type_name_with_bounds(&named("A"), &bounds),
            "java.lang.Object"
        );
        bounds.insert(
            "T".into(),
            JavaType::Named {
                name: "Comparable".into(),
                args: vec![named("T")],
            },
        );
        assert_eq!(
            erased_type_name_with_bounds(&named("T"), &bounds),
            "Comparable"
        );
    }
}
