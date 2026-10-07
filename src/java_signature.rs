use crate::java_ir::{CallKind, JavaType, MethodDecl, Modifier, Param, Program, TypeDecl};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JavaOwnerKey(String);

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
}

impl JavaCallableSlotKey {
    pub(crate) fn source_method(method: &MethodDecl) -> Self {
        JavaCallableKey::source_method(JavaOwnerKey::resolved_source(""), method).slot()
    }
}

pub(crate) fn erased_param_types(params: &[Param]) -> Vec<String> {
    params
        .iter()
        .map(|param| erased_type_name(&param.ty, param.is_varargs))
        .collect()
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
