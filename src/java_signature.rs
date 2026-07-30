use crate::java_ir::{JavaType, MethodDecl, Modifier, Param};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JavaCallableKey {
    pub owner: String,
    pub java_name: String,
    pub erased_param_types: Vec<String>,
    pub kind: JavaCallableKind,
    pub staticness: JavaCallableStaticness,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JavaCallableSlotKey {
    pub java_name: String,
    pub erased_param_types: Vec<String>,
    pub kind: JavaCallableKind,
    pub staticness: JavaCallableStaticness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JavaCallableKind {
    Method,
    Constructor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JavaCallableStaticness {
    Static,
    Instance,
}

impl JavaCallableKey {
    pub fn source_method(owner: impl Into<String>, method: &MethodDecl) -> Self {
        Self {
            owner: owner.into(),
            java_name: method.name.clone(),
            erased_param_types: erased_param_types(&method.params),
            kind: JavaCallableKind::Method,
            staticness: method_staticness(method),
        }
    }

    pub fn slot(&self) -> JavaCallableSlotKey {
        JavaCallableSlotKey {
            java_name: self.java_name.clone(),
            erased_param_types: self.erased_param_types.clone(),
            kind: self.kind,
            staticness: self.staticness,
        }
    }
}

impl JavaCallableSlotKey {
    pub fn source_method(method: &MethodDecl) -> Self {
        JavaCallableKey::source_method("", method).slot()
    }
}

pub fn erased_param_types(params: &[Param]) -> Vec<String> {
    params
        .iter()
        .map(|param| erased_type_name(&param.ty, param.is_varargs))
        .collect()
}

pub fn erased_type_name(ty: &JavaType, is_varargs: bool) -> String {
    let mut erased = match ty {
        JavaType::Void => "void".to_string(),
        JavaType::Primitive(primitive) => format!("{primitive:?}").to_ascii_lowercase(),
        JavaType::String => "java.lang.String".to_string(),
        JavaType::Array(element) => format!("{}[]", erased_type_name(element, false)),
        JavaType::Named { name, .. } => name.clone(),
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

pub fn method_staticness(method: &MethodDecl) -> JavaCallableStaticness {
    if method.modifiers.contains(&Modifier::Static) {
        JavaCallableStaticness::Static
    } else {
        JavaCallableStaticness::Instance
    }
}

pub fn source_method_rust_name(method: &MethodDecl) -> String {
    method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name))
}

pub fn source_method_overload_rust_name(method: &MethodDecl) -> String {
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
