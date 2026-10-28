use std::collections::{HashMap, HashSet};

use crate::java_ir::{BinaryOp, Expr, JavaType, Literal, PrimitiveType, Stmt, WildcardBound};

/// Locate symbolic owner binders structurally before actual-type substitution.
/// Method/constructor binders shadow same-named owner binders.
pub(crate) fn type_parameter_paths(
    declared: &JavaType,
    params: &[String],
    shadows: &[String],
) -> Vec<(usize, Vec<usize>)> {
    fn visit(
        ty: &JavaType,
        prefix: &mut Vec<usize>,
        params: &[String],
        shadows: &[String],
        out: &mut Vec<(usize, Vec<usize>)>,
    ) {
        let ty = crate::types::optional_element_type(ty).unwrap_or_else(|| ty.clone());
        match ty {
            JavaType::Named { name, args }
                if args.is_empty() && !shadows.contains(&name) && params.contains(&name) =>
            {
                out.push((
                    params.iter().position(|param| param == &name).unwrap(),
                    prefix.clone(),
                ));
            }
            JavaType::Named { args, .. } => {
                for (index, arg) in args.iter().enumerate() {
                    prefix.push(index);
                    visit(arg, prefix, params, shadows, out);
                    prefix.pop();
                }
            }
            JavaType::Array(element) => {
                prefix.push(0);
                visit(&element, prefix, params, shadows, out);
                prefix.pop();
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    visit(declared, &mut Vec::new(), params, shadows, &mut out);
    out
}

#[derive(Debug, Clone, Default)]
pub struct TypeEnv {
    locals: HashMap<String, JavaType>,
    params: HashSet<String>,
    callable_locals: HashSet<String>,
    comparator_object_locals: HashSet<String>,
    comparator_object_fields: HashSet<String>,
    fields: HashMap<String, JavaType>,
    this_type: Option<JavaType>,
    // The inline anonymous body's `this` is not the enclosing lexical class.
    // Keep its declared/inherited fields distinct from captured outer fields.
    anonymous_this_fields: Option<HashMap<String, JavaType>>,
    type_fields: HashMap<(String, String), JavaType>,
    type_param_bounds: HashMap<String, Option<JavaType>>,
    // Invocation-local wildcard captures have both an upper bound and an
    // optional lower bound. Ordinary Java type parameters have no lower bound.
    inference_capture_lower_bounds: HashMap<String, Option<JavaType>>,
    static_type_aliases: HashMap<String, String>,
    // Shared across lexical environment clones: emission ownership is a
    // batch invariant, not a per-expression map to rebuild or deep-copy.
    source_emission: Option<std::sync::Arc<crate::source_emission::SourceEmissionPlan>>,
    static_member_imports: HashMap<String, String>,
}

pub(crate) enum MethodReferenceValueBinding<'a> {
    Local(String, &'a JavaType),
    Field(String, &'a JavaType),
}

impl MethodReferenceValueBinding<'_> {
    pub(crate) fn java_type(&self) -> &JavaType {
        match self {
            Self::Local(_, ty) | Self::Field(_, ty) => ty,
        }
    }
}

impl TypeEnv {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_local(&mut self, name: impl Into<String>, ty: JavaType) {
        self.locals.insert(name.into(), ty);
    }

    pub(crate) fn clear_local_scope(&mut self) {
        self.locals.clear();
        self.params.clear();
        self.callable_locals.clear();
        self.comparator_object_locals.clear();
    }

    pub fn mark_callable_local(&mut self, name: impl Into<String>) {
        self.callable_locals.insert(name.into());
    }

    pub fn mark_comparator_object_local(&mut self, name: impl Into<String>) {
        self.comparator_object_locals.insert(name.into());
    }

    pub fn mark_comparator_object_field(&mut self, name: impl Into<String>) {
        self.comparator_object_fields.insert(name.into());
    }

    pub fn insert_param(&mut self, name: impl Into<String>, ty: JavaType) {
        let name = name.into();
        self.locals.insert(name.clone(), ty);
        self.params.insert(name);
    }

    pub fn is_param(&self, name: &str) -> bool {
        self.params.contains(name)
            || name
                .strip_prefix("r#")
                .is_some_and(|plain| self.params.contains(plain))
            || (!name.starts_with("r#") && self.params.contains(&format!("r#{name}")))
    }

    pub fn is_callable_local(&self, name: &str) -> bool {
        self.callable_locals.contains(name)
            || name
                .strip_prefix("r#")
                .is_some_and(|plain| self.callable_locals.contains(plain))
            || (!name.starts_with("r#") && self.callable_locals.contains(&format!("r#{name}")))
    }

    pub fn is_comparator_object_local(&self, name: &str) -> bool {
        self.comparator_object_locals.contains(name)
            || name
                .strip_prefix("r#")
                .is_some_and(|plain| self.comparator_object_locals.contains(plain))
            || (!name.starts_with("r#")
                && self.comparator_object_locals.contains(&format!("r#{name}")))
    }

    pub fn is_comparator_object_field(&self, name: &str) -> bool {
        self.comparator_object_fields.contains(name)
            || name
                .strip_prefix("r#")
                .is_some_and(|plain| self.comparator_object_fields.contains(plain))
            || (!name.starts_with("r#")
                && self.comparator_object_fields.contains(&format!("r#{name}")))
    }

    pub fn insert_field(&mut self, name: impl Into<String>, ty: JavaType) {
        self.fields.insert(name.into(), ty);
    }

    pub(crate) fn set_this_type(&mut self, ty: JavaType) {
        self.this_type = Some(ty);
    }

    pub(crate) fn bind_this_type(&mut self, ty: &crate::java_ir::TypeDecl) {
        self.set_this_type(JavaType::Named {
            name: ty.name.clone(),
            args: ty
                .type_params
                .iter()
                .map(|name| JavaType::Named {
                    name: name.clone(),
                    args: vec![],
                })
                .collect(),
        });
    }

    pub(crate) fn enter_anonymous_field_scope(
        &mut self,
        fields: impl IntoIterator<Item = (String, JavaType)>,
    ) {
        self.this_type = None;
        let mut own = HashMap::new();
        for (name, ty) in fields {
            let alias = crate::parser::to_snake_case(&name);
            for spelling in [name, alias] {
                // A member of the anonymous class shadows an enclosing local;
                // parameters/locals declared inside its methods are added later.
                self.locals.remove(&spelling);
                self.params.remove(&spelling);
                self.callable_locals.remove(&spelling);
                self.comparator_object_locals.remove(&spelling);
                self.fields.insert(spelling.clone(), ty.clone());
                own.insert(spelling, ty.clone());
            }
        }
        self.anonymous_this_fields = Some(own);
    }

    pub(crate) fn anonymous_this_field_type(&self, name: &str) -> Option<&JavaType> {
        let fields = self.anonymous_this_fields.as_ref()?;
        fields
            .get(name)
            .or_else(|| fields.get(&crate::parser::to_snake_case(name)))
    }

    pub fn insert_type_field(
        &mut self,
        type_name: impl Into<String>,
        field_name: impl Into<String>,
        ty: JavaType,
    ) {
        self.type_fields
            .insert((type_name.into(), field_name.into()), ty);
    }

    pub fn insert_type_param_bound(&mut self, name: impl Into<String>, bound: JavaType) {
        self.insert_type_param_declaration(name, Some(bound));
    }

    pub(crate) fn insert_type_param_declaration(
        &mut self,
        name: impl Into<String>,
        bound: Option<JavaType>,
    ) {
        let name = name.into();
        self.inference_capture_lower_bounds.remove(&name);
        self.type_param_bounds.insert(name, bound);
    }

    pub(crate) fn insert_inference_capture(
        &mut self,
        name: String,
        upper: JavaType,
        lower: Option<JavaType>,
    ) {
        self.type_param_bounds.insert(name.clone(), Some(upper));
        self.inference_capture_lower_bounds.insert(name, lower);
    }

    pub(crate) fn is_inference_capture(&self, name: &str) -> bool {
        self.inference_capture_lower_bounds.contains_key(name)
    }

    pub(crate) fn inference_capture_lower_bound(&self, name: &str) -> Option<&JavaType> {
        self.inference_capture_lower_bounds
            .get(name)
            .and_then(Option::as_ref)
    }

    pub fn insert_static_type_alias(
        &mut self,
        alias: impl Into<String>,
        type_name: impl Into<String>,
    ) {
        self.static_type_aliases
            .insert(alias.into(), type_name.into());
    }

    pub fn insert_static_type_alias_if_absent(
        &mut self,
        alias: impl Into<String>,
        type_name: impl Into<String>,
    ) {
        self.static_type_aliases
            .entry(alias.into())
            .or_insert_with(|| type_name.into());
    }

    pub fn insert_static_member_import(
        &mut self,
        member: impl Into<String>,
        owner_type: impl Into<String>,
    ) {
        self.static_member_imports
            .insert(member.into(), owner_type.into());
    }

    pub fn local_type(&self, name: &str) -> Option<&JavaType> {
        self.locals
            .get(name)
            .or_else(|| {
                name.strip_prefix("r#")
                    .and_then(|plain| self.locals.get(plain))
            })
            .or_else(|| {
                if name.starts_with("r#") {
                    None
                } else {
                    self.locals.get(&format!("r#{name}"))
                }
            })
    }

    /// Exact Java value bindings shadow types. A normalized compatibility
    /// spelling must never manufacture that shadowing for a known type owner.
    /// Share the same namespace decision between selection, effects and emit.
    pub(crate) fn method_reference_value_binding(
        &self,
        target: &str,
        known_type_owner: bool,
    ) -> Option<MethodReferenceValueBinding<'_>> {
        let lookup = |name: &str| {
            self.local_type(name)
                .map(|ty| MethodReferenceValueBinding::Local(name.to_string(), ty))
                .or_else(|| {
                    self.field_type_exact(name)
                        .map(|ty| MethodReferenceValueBinding::Field(name.to_string(), ty))
                })
        };
        if let Some(binding) = lookup(target) {
            return Some(binding);
        }
        if known_type_owner || self.static_type_alias(target).is_some() {
            return None;
        }
        let normalized = crate::parser::to_snake_case(target);
        (normalized != target)
            .then(|| lookup(&normalized))
            .flatten()
    }

    pub fn field_type(&self, name: &str) -> Option<&JavaType> {
        self.field_type_exact(name).or_else(|| {
            let plain = name.strip_prefix("r#").unwrap_or(name);
            let alias = crate::parser::to_snake_case(plain);
            (alias != plain).then(|| self.fields.get(&alias)).flatten()
        })
    }

    pub(crate) fn field_type_exact(&self, name: &str) -> Option<&JavaType> {
        self.fields
            .get(name)
            .or_else(|| {
                name.strip_prefix("r#")
                    .and_then(|plain| self.fields.get(plain))
            })
            .or_else(|| {
                if name.starts_with("r#") {
                    None
                } else {
                    self.fields.get(&format!("r#{name}"))
                }
            })
    }

    pub fn type_field_type(&self, type_name: &str, field_name: &str) -> Option<&JavaType> {
        self.type_fields
            .get(&(type_name.to_string(), field_name.to_string()))
    }

    pub fn type_param_bound(&self, name: &str) -> Option<&JavaType> {
        self.type_param_bounds.get(name).and_then(Option::as_ref)
    }

    /// Resolve a scoped type-parameter receiver to its declared class bound.
    /// Keep the bound's arguments: field ABI specialization needs them. An
    /// unbounded/shadowing binder or cyclic bound is not a source type alias.
    pub(crate) fn type_parameter_receiver_bound(&self, ty: &JavaType) -> Option<JavaType> {
        let mut current = optional_element_type(ty).unwrap_or_else(|| ty.clone());
        let mut seen = HashSet::new();
        let mut followed_bound = false;
        loop {
            let name = match &current {
                JavaType::Named { name, args } if args.is_empty() => name,
                JavaType::Unknown(name) => name,
                _ => return followed_bound.then_some(current),
            };
            let Some(bound) = self.type_param_bounds.get(name) else {
                return followed_bound.then_some(current);
            };
            if !seen.insert(name.clone()) {
                return None;
            }
            current = bound.as_ref()?.clone();
            followed_bound = true;
        }
    }

    pub(crate) fn type_parameter_names(&self) -> Vec<String> {
        self.type_param_bounds.keys().cloned().collect()
    }

    pub(crate) fn is_type_parameter_name(&self, name: &str) -> bool {
        self.type_param_bounds.contains_key(name)
    }

    pub(crate) fn is_unbounded_object_type_parameter(&self, ty: &JavaType) -> bool {
        let ty = optional_element_type(ty).unwrap_or_else(|| ty.clone());
        let name = match &ty {
            JavaType::Named { name, args } if args.is_empty() => name,
            JavaType::Unknown(name) => name,
            _ => return false,
        };
        self.type_param_bounds.get(name).is_some_and(|bound| {
            bound.as_ref().is_none_or(|bound| matches!(bound,
                JavaType::Named { name, args } if args.is_empty() && matches!(name.as_str(), "Object" | "java.lang.Object")
            ))
        })
    }

    pub fn static_type_alias(&self, name: &str) -> Option<&str> {
        self.static_type_aliases.get(name).map(String::as_str)
    }

    pub(crate) fn set_source_emission(
        &mut self,
        plan: std::sync::Arc<crate::source_emission::SourceEmissionPlan>,
    ) {
        self.source_emission = Some(plan);
    }

    pub(crate) fn is_emitted_source_owner(&self, owner: &str) -> bool {
        self.source_emission
            .as_ref()
            .is_some_and(|plan| plan.rust_owner(owner).is_some())
    }

    pub fn static_member_import_owner(&self, member: &str) -> Option<&str> {
        self.static_member_imports.get(member).map(String::as_str)
    }

    pub fn learn_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl {
                ty,
                name,
                source_name,
                ..
            } => {
                self.insert_local(name.clone(), ty.clone());
                // Constant-style references retain Java spelling in the IR,
                // even when the declared Rust binding name is normalized.
                self.insert_local(source_name.clone(), ty.clone());
            }
            Stmt::For { init, .. } => {
                for stmt in init {
                    self.learn_stmt(stmt);
                }
            }
            Stmt::EnhancedFor {
                var_type, var_name, ..
            } => self.insert_local(var_name.clone(), var_type.clone()),
            Stmt::Try {
                resources, body, ..
            } => {
                for resource in resources {
                    self.learn_stmt(resource);
                }
                for stmt in &body.statements {
                    self.learn_stmt(stmt);
                }
            }
            _ => {}
        }
    }
}

/// Substitute an identifier in generated Rust type/header text without
/// replacing occurrences embedded in a longer identifier.
pub fn replace_rust_identifier(source: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(offset) = rest.find(from) {
        let before = &rest[..offset];
        let after = &rest[offset + from.len()..];
        let left_boundary = before
            .chars()
            .next_back()
            .is_none_or(|ch| !(ch.is_ascii_alphanumeric() || ch == '_'));
        let right_boundary = after
            .chars()
            .next()
            .is_none_or(|ch| !(ch.is_ascii_alphanumeric() || ch == '_'));
        out.push_str(before);
        if left_boundary && right_boundary {
            out.push_str(to);
        } else {
            out.push_str(from);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

pub fn infer_expr_type(expr: &Expr, env: &TypeEnv) -> JavaType {
    match expr {
        Expr::Literal(lit) => literal_type(lit),
        Expr::Var(name) => env
            .local_type(name)
            .cloned()
            .or_else(|| env.field_type(name).cloned())
            .unwrap_or_else(|| JavaType::Unknown(name.clone())),
        Expr::This => env
            .this_type
            .clone()
            .unwrap_or_else(|| JavaType::Unknown("this".to_string())),
        Expr::Super => JavaType::Unknown("super".to_string()),
        Expr::FieldAccess(receiver, field) => {
            if let Some(java_ty) = infer_member_access_type(receiver, field, env) {
                java_ty
            } else if matches!(receiver.as_ref(), Expr::This) {
                env.field_type(field)
                    .cloned()
                    .unwrap_or_else(|| JavaType::Unknown(field.clone()))
            } else {
                JavaType::Unknown(field.clone())
            }
        }
        Expr::ArrayAccess(array, _) => {
            sequence_element_type_after_optional(&infer_expr_type(array, env))
                .unwrap_or_else(|| JavaType::Unknown("array_access".to_string()))
        }
        Expr::ArrayCreation {
            element_type,
            dimensions,
            initializer,
        } => (0..initializer.as_ref().map_or(dimensions.len(), |_| 1).max(1))
            .fold(element_type.clone(), |ty, _| JavaType::Array(Box::new(ty))),
        Expr::ArrayInitializer(elements) => elements
            .first()
            .map(|element| JavaType::Array(Box::new(infer_expr_type(element, env))))
            .unwrap_or_else(|| {
                JavaType::Array(Box::new(JavaType::Unknown("array_initializer".to_string())))
            }),
        Expr::Binary { op, left, right } => infer_binary_type(*op, left, right, env),
        Expr::Unary {
            op: crate::java_ir::UnaryOp::BitNot,
            expr,
        } => match infer_expr_type(expr, env) {
            JavaType::Primitive(PrimitiveType::Long) => JavaType::Primitive(PrimitiveType::Long),
            JavaType::Primitive(
                PrimitiveType::Byte
                | PrimitiveType::Short
                | PrimitiveType::Char
                | PrimitiveType::Int,
            ) => JavaType::Primitive(PrimitiveType::Int),
            other => other,
        },
        Expr::Unary { expr, .. } => infer_expr_type(expr, env),
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => common_type(
            &infer_expr_type(then_expr, env),
            &infer_expr_type(else_expr, env),
        ),
        Expr::Switch { cases, .. } => cases
            .iter()
            .map(|case| infer_expr_type(&case.value, env))
            .reduce(|left, right| common_type(&left, &right))
            .unwrap_or_else(|| JavaType::Unknown("switch_expression".to_string())),
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
            ..
        } => {
            if let Some(call) = resolved {
                call.return_type.clone()
            } else if name == "nonNull" && !args.is_empty() {
                infer_expr_type(&args[0], env)
            } else {
                let receiver_type = receiver
                    .as_deref()
                    .map(|receiver| infer_expr_type(receiver, env));
                let arg_types = args
                    .iter()
                    .map(|arg| infer_expr_type(arg, env))
                    .collect::<Vec<_>>();
                crate::stdlib::known_external_method_return_type_for_call(
                    receiver_type.as_ref(),
                    name,
                    &arg_types,
                )
                .or_else(|| {
                    external_pack_method_return_type(receiver_type.as_ref(), name, args.len())
                })
                .unwrap_or_else(|| JavaType::Unknown(name.clone()))
            }
        }
        Expr::New {
            resolved: Some(resolved),
            ..
        } => resolved.constructed_type.clone(),
        Expr::New { ty, .. } => ty.clone(),
        Expr::Cast { ty, .. } => ty.clone(),
        Expr::InstanceOf { .. } => JavaType::Primitive(PrimitiveType::Boolean),
        Expr::Lambda { .. } => JavaType::Unknown("lambda".to_string()),
        Expr::MethodReference { .. } => JavaType::Unknown("method_reference".to_string()),
        Expr::ClassLiteral(ty) => class_literal_type(ty),
        Expr::Unsupported { reason, .. } => JavaType::Unknown(reason.clone()),
        Expr::Raw(raw) => JavaType::Unknown(raw.clone()),
    }
}

fn external_pack_method_return_type(
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_count: usize,
) -> Option<JavaType> {
    if let Some(receiver_type) = receiver_type
        && let Some(element_ty) = optional_element_type(receiver_type)
    {
        return external_pack_method_return_type(Some(&element_ty), name, arg_count);
    }
    let receiver_name = match receiver_type {
        Some(JavaType::Named { name, .. }) | Some(JavaType::Unknown(name)) => Some(name.as_str()),
        _ => None,
    };
    crate::external_packs::external_api_effect_for_receiver_with_arity(
        receiver_name,
        name,
        Some(arg_count),
    )
    .and_then(|fact| {
        fact.return_type
            .map(|return_ty| receiver_generic_return_type(receiver_type, return_ty))
    })
}

pub fn receiver_generic_return_type(
    receiver_type: Option<&JavaType>,
    return_ty: JavaType,
) -> JavaType {
    let Some(receiver_type) = receiver_type else {
        return return_ty;
    };
    let receiver_type =
        optional_element_type(receiver_type).unwrap_or_else(|| receiver_type.clone());
    let JavaType::Named {
        args: receiver_args,
        ..
    } = receiver_type
    else {
        return return_ty;
    };
    substitute_receiver_generic_placeholders(return_ty, &receiver_args)
}

fn substitute_receiver_generic_placeholders(
    return_ty: JavaType,
    receiver_args: &[JavaType],
) -> JavaType {
    if let Some(index) = receiver_generic_placeholder_index(&return_ty) {
        return receiver_args.get(index).cloned().unwrap_or(return_ty);
    }
    match return_ty {
        JavaType::Named { name, args } if !args.is_empty() => JavaType::Named {
            name,
            args: args
                .into_iter()
                .map(|arg| substitute_receiver_generic_placeholders(arg, receiver_args))
                .collect(),
        },
        other => other,
    }
}

fn receiver_generic_placeholder_index(return_ty: &JavaType) -> Option<usize> {
    let JavaType::Named { name, args } = return_ty else {
        return None;
    };
    if !args.is_empty() {
        return None;
    }
    if name == "T" {
        return Some(0);
    }
    let index = name.strip_prefix('T')?.parse::<usize>().ok()?;
    Some(index)
}

pub fn class_literal_type(ty: &JavaType) -> JavaType {
    JavaType::Named {
        name: "Class".to_string(),
        args: vec![ty.clone()],
    }
}

pub fn literal_type(lit: &Literal) -> JavaType {
    match lit {
        Literal::Null => JavaType::Unknown("null".to_string()),
        Literal::Bool(_) => JavaType::Primitive(PrimitiveType::Boolean),
        Literal::Int(_) => JavaType::Primitive(PrimitiveType::Int),
        Literal::Long(_) => JavaType::Primitive(PrimitiveType::Long),
        Literal::Float(_) => JavaType::Primitive(PrimitiveType::Float),
        Literal::Double(_) => JavaType::Primitive(PrimitiveType::Double),
        Literal::String(_) | Literal::StringUtf16(_) => JavaType::String,
        Literal::Char(_) | Literal::CharUtf16(_) => JavaType::Primitive(PrimitiveType::Char),
    }
}

pub fn sequence_element_type(java_ty: &JavaType) -> Option<JavaType> {
    match java_ty {
        JavaType::Array(inner) => Some((**inner).clone()),
        JavaType::Named { name, args }
            if crate::stdlib::canonical_external_receiver_type_key(name) == "Iterable"
                && args.len() == 1 =>
        {
            Some(args[0].clone())
        }
        JavaType::Named { name, args }
            if crate::stdlib::java_collection_kind_for_name(name)
                .is_some_and(|kind| !kind.is_map_like())
                && args.len() == 1 =>
        {
            Some(args[0].clone())
        }
        JavaType::Named { .. } => crate::javalibs::facade_sequence_element_type(java_ty),
        _ => None,
    }
}

pub fn sequence_mutation_element_type(java_ty: &JavaType) -> Option<JavaType> {
    sequence_element_type(java_ty).map(|element_ty| match element_ty {
        JavaType::Wildcard(Some(WildcardBound::Super(bound))) => *bound,
        other => other,
    })
}

/// Projects the value type produced when a Java generic container is read.
///
/// A value read through `? extends T` has the usable static type `T`; retaining
/// the existential wildcard at that boundary incorrectly turns concrete
/// native carriers (for example `List<T>`) into erased objects.  Contravariant
/// `? super T` does not have that property and therefore remains unchanged.
pub fn generic_read_type(ty: &JavaType) -> JavaType {
    generic_read_type_ref(ty).clone()
}

/// Borrowed form of the same read projection for representation decisions.
/// This projects one value, never reinterprets an invariant container.
pub(crate) fn generic_read_type_ref(mut ty: &JavaType) -> &JavaType {
    while let JavaType::Wildcard(Some(WildcardBound::Extends(bound))) = ty {
        ty = bound;
    }
    ty
}

/// Read a generic reference slot without discarding its solved null carrier.
/// Shared by ordinary iteration, retained cursors and native entry projections.
pub(crate) fn generic_read_type_preserving_optional(ty: &JavaType) -> JavaType {
    if let Some(element_ty) = optional_element_type(ty) {
        optional_java_type(generic_read_type(&element_ty))
    } else {
        generic_read_type(ty)
    }
}

/// A value supplied to an input-only `? super T` contract may have type T.
/// Do not apply the dual extends projection here: `? extends T` does not
/// permit arbitrary T values to be written through its existential slot.
pub fn generic_input_type(ty: &JavaType) -> JavaType {
    match ty {
        JavaType::Wildcard(Some(WildcardBound::Super(bound))) => bound.as_ref().clone(),
        _ => ty.clone(),
    }
}

pub fn sequence_element_type_after_optional(java_ty: &JavaType) -> Option<JavaType> {
    sequence_element_type(java_ty).or_else(|| {
        optional_element_type(java_ty)
            .as_ref()
            .and_then(sequence_element_type)
    })
}

pub fn two_dimensional_array_element_type(java_ty: &JavaType) -> Option<JavaType> {
    match java_ty {
        JavaType::Array(row_ty) => match row_ty.as_ref() {
            JavaType::Array(element_ty) if !matches!(element_ty.as_ref(), JavaType::Array(_)) => {
                Some((**element_ty).clone())
            }
            _ => None,
        },
        _ => None,
    }
}

pub fn ndarray_array2_element_type(java_ty: &JavaType) -> Option<JavaType> {
    let element_ty = two_dimensional_array_element_type(java_ty)?;
    matches!(element_ty, JavaType::Primitive(_)).then_some(element_ty)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayLengthDimension {
    Rows,
    Columns,
    Linear,
}

pub fn array_length_dimension(receiver: &Expr, env: &TypeEnv) -> ArrayLengthDimension {
    if two_dimensional_array_element_type_after_optional(&infer_expr_type(receiver, env)).is_some()
    {
        return ArrayLengthDimension::Rows;
    }
    if let Expr::ArrayAccess(array, _) = receiver
        && two_dimensional_array_element_type_after_optional(&infer_expr_type(array, env)).is_some()
    {
        return ArrayLengthDimension::Columns;
    }
    ArrayLengthDimension::Linear
}

fn two_dimensional_array_element_type_after_optional(java_ty: &JavaType) -> Option<JavaType> {
    two_dimensional_array_element_type(java_ty).or_else(|| {
        optional_element_type(java_ty)
            .as_ref()
            .and_then(two_dimensional_array_element_type)
    })
}

pub fn optional_element_type(java_ty: &JavaType) -> Option<JavaType> {
    match java_ty {
        JavaType::Named { name, args } if args.is_empty() => {
            crate::stdlib::primitive_optional_payload(name).map(JavaType::Primitive)
        }
        JavaType::Named { name, args }
            if matches!(name.as_str(), "Optional" | "java.util.Optional") && args.len() == 1 =>
        {
            Some(args[0].clone())
        }
        _ => None,
    }
}

pub fn is_numeric_primitive_type(primitive: PrimitiveType) -> bool {
    !matches!(primitive, PrimitiveType::Boolean)
}

pub fn is_named_java_type(java_ty: &JavaType, expected: &str) -> bool {
    matches!(java_ty, JavaType::Named { name, .. } if name == expected)
}

pub fn is_optional_or_result_type(java_ty: &JavaType) -> bool {
    is_named_java_type(java_ty, "Optional") || is_named_java_type(java_ty, "Result")
}

pub fn contains_wildcard_type(java_ty: &JavaType) -> bool {
    match java_ty {
        JavaType::Wildcard(_) => true,
        JavaType::Array(inner) => contains_wildcard_type(inner),
        JavaType::Named { args, .. } => args.iter().any(contains_wildcard_type),
        _ => false,
    }
}

pub(crate) fn is_raw_generic_counterpart(actual: &JavaType, target: &JavaType) -> bool {
    match (actual, target) {
        (
            JavaType::Named {
                name: actual_name,
                args: actual_args,
            },
            JavaType::Named {
                name: target_name,
                args: target_args,
            },
        ) => {
            actual_name == target_name
                && ((actual_args.is_empty() && !target_args.is_empty())
                    || (!actual_args.is_empty() && target_args.is_empty()))
        }
        _ => false,
    }
}

pub(crate) fn raw_rust_generic_args<'a>(raw: &'a str, head: &str) -> Option<Vec<&'a str>> {
    let raw = raw.trim();
    let rest = raw.strip_prefix(head)?.strip_prefix('<')?;
    let inner = rest.strip_suffix('>')?;
    Some(split_top_level_rust_type_args(inner))
}

pub(crate) fn raw_generic_type_head(raw: &str) -> Option<&str> {
    let raw = raw.trim();
    let (head, rest) = raw.split_once('<')?;
    rest.ends_with('>').then_some(head.trim())
}

pub fn rendered_rust_type_is_result(rendered: &str) -> bool {
    raw_rust_type_is_result(rendered)
}

pub fn rendered_rust_type_is_option(rendered: &str) -> bool {
    raw_rust_type_is_option(rendered)
}

pub fn rendered_rust_result_args(rendered: &str) -> Option<Vec<&str>> {
    raw_rust_result_args(rendered)
}

pub fn rendered_rust_option_inner(rendered: &str) -> Option<&str> {
    raw_rust_option_inner(rendered)
}

pub(crate) fn raw_rust_type_is_result(raw: &str) -> bool {
    raw_rust_result_args(raw).is_some()
}

pub(crate) fn raw_rust_type_is_option(raw: &str) -> bool {
    raw_rust_option_inner(raw).is_some()
}

pub(crate) fn raw_rust_result_args(raw: &str) -> Option<Vec<&str>> {
    let raw = raw.trim();
    let generic_start = raw.find('<')?;
    let head = raw[..generic_start].trim();
    if head != "Result" && head != "std::result::Result" && !head.ends_with("::Result") {
        return None;
    }
    let inner = raw[generic_start + 1..].strip_suffix('>')?;
    Some(split_top_level_rust_type_args(inner))
}

pub(crate) fn raw_rust_option_inner(raw: &str) -> Option<&str> {
    raw_rust_generic_args(raw, "Option")
        .or_else(|| raw_rust_generic_args(raw, "std::option::Option"))
        .and_then(|args| {
            if args.len() == 1 {
                args.first().copied()
            } else {
                None
            }
        })
}

pub(crate) fn raw_rust_vec_inner(raw: &str) -> Option<&str> {
    raw_rust_generic_args(raw, "Vec")
        .or_else(|| raw_rust_generic_args(raw, "std::vec::Vec"))
        .and_then(|args| {
            if args.len() == 1 {
                args.first().copied()
            } else {
                None
            }
        })
}

pub(crate) fn raw_rust_box_dyn_trait(raw: &str) -> Option<&str> {
    raw_rust_box_arg(raw).and_then(|arg| arg.strip_prefix("dyn ").map(str::trim))
}

/// The pointee of a `Box<T>` where `T` is a sized type — the complement of
/// [`raw_rust_box_dyn_trait`]. Recursive Java object graphs (`Node next`) lower
/// to `Option<Box<Node>>`, and a boxed pointee is transparent to most type
/// properties (`Clone`, `PartialEq`, …), so callers need to see through it.
pub(crate) fn raw_rust_box_sized_inner(raw: &str) -> Option<&str> {
    raw_rust_box_arg(raw).filter(|arg| !arg.starts_with("dyn "))
}

fn raw_rust_box_arg(raw: &str) -> Option<&str> {
    raw_rust_generic_args(raw, "Box")
        .or_else(|| raw_rust_generic_args(raw, "std::boxed::Box"))
        .and_then(|args| {
            if args.len() != 1 {
                return None;
            }
            args.first().copied().map(str::trim)
        })
}

fn split_top_level_rust_type_args(args: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut angle_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut cursor = 0usize;
    while cursor < args.len() {
        if let Some(end) = rust_type_raw_string_literal_end_at(args, cursor) {
            cursor = end;
            continue;
        }
        if let Some(end) = rust_type_comment_end_at(args, cursor) {
            cursor = end;
            continue;
        }
        let ch = args[cursor..]
            .chars()
            .next()
            .expect("cursor is always on a char boundary");
        if ch == '"' {
            cursor = rust_type_string_literal_end_at(args, cursor).unwrap_or(args.len());
            continue;
        }
        match ch {
            '<' => angle_depth += 1,
            '>' => angle_depth = angle_depth.saturating_sub(1),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            ',' if angle_depth == 0
                && paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0 =>
            {
                parts.push(args[start..cursor].trim());
                start = cursor + ch.len_utf8();
            }
            _ => {}
        }
        cursor += ch.len_utf8();
    }
    parts.push(args[start..].trim());
    parts
}

fn rust_type_comment_end_at(raw: &str, idx: usize) -> Option<usize> {
    if raw[idx..].starts_with("//") {
        return Some(
            raw[idx..]
                .find('\n')
                .map(|offset| idx + offset)
                .unwrap_or(raw.len()),
        );
    }
    if raw[idx..].starts_with("/*") {
        return Some(
            raw[idx + 2..]
                .find("*/")
                .map(|offset| idx + 2 + offset + 2)
                .unwrap_or(raw.len()),
        );
    }
    None
}

fn rust_type_raw_string_literal_end_at(raw: &str, idx: usize) -> Option<usize> {
    let rest = raw.get(idx..)?;
    let mut chars = rest.char_indices();
    let (_, first) = chars.next()?;
    if first != 'r' {
        return None;
    }
    let mut hashes = 0usize;
    let mut quote_start = None;
    for (offset, ch) in chars {
        match ch {
            '#' if quote_start.is_none() => hashes += 1,
            '"' if quote_start.is_none() => {
                quote_start = Some(offset);
                break;
            }
            _ => return None,
        }
    }
    let quote_start = quote_start?;
    let body_start = idx + quote_start + 1;
    let terminator = format!("\"{}", "#".repeat(hashes));
    raw[body_start..]
        .find(&terminator)
        .map(|offset| body_start + offset + terminator.len())
}

fn rust_type_string_literal_end_at(raw: &str, idx: usize) -> Option<usize> {
    let rest = raw.get(idx..)?;
    if !rest.starts_with('"') {
        return None;
    }
    let mut escaped = false;
    for (offset, ch) in rest.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(idx + offset + ch.len_utf8());
        }
    }
    None
}

pub(crate) fn is_known_raw_generic_type(java_ty: &JavaType) -> bool {
    matches!(
        java_ty,
        JavaType::Named { name, args }
            if args.is_empty()
                && matches!(
                    name.as_str(),
                    "List"
                        | "Set"
                        | "Map"
                        | "Optional"
                        | "Iterator"
                        | "ListIterator"
                        | "Iterable"
                        | "Comparator"
                        | "Comparable"
                        | "Class"
                )
    )
}

pub(crate) fn is_unchecked_generic_counterpart(actual: &JavaType, target: &JavaType) -> bool {
    match (actual, target) {
        (
            JavaType::Named {
                name: actual_name,
                args: actual_args,
            },
            JavaType::Named {
                name: target_name,
                args: target_args,
            },
        ) => {
            actual_name == target_name
                && !actual_args.is_empty()
                && !target_args.is_empty()
                && actual_args != target_args
        }
        _ => false,
    }
}

pub fn list_java_type(element_ty: JavaType) -> JavaType {
    JavaType::Named {
        name: "List".to_string(),
        args: vec![element_ty],
    }
}

pub fn set_java_type(element_ty: JavaType) -> JavaType {
    JavaType::Named {
        name: "Set".to_string(),
        args: vec![element_ty],
    }
}

pub fn optional_java_type(element_ty: JavaType) -> JavaType {
    JavaType::Named {
        name: "Optional".to_string(),
        args: vec![element_ty],
    }
}

pub fn stream_element_type(source_ty: &JavaType) -> Option<JavaType> {
    let element = match source_ty {
        JavaType::Named { name, args }
            if crate::stdlib::canonical_external_receiver_type_key(name) == "Stream"
                && args.len() == 1 =>
        {
            args.first().cloned()
        }
        _ => sequence_element_type(source_ty),
    };
    element.map(normalize_stream_element_type)
}

fn normalize_stream_element_type(ty: JavaType) -> JavaType {
    match ty {
        JavaType::Named { name, args } if name == "Integer" && args.is_empty() => {
            JavaType::Primitive(PrimitiveType::Int)
        }
        other => other,
    }
}

pub fn is_final_by_default(java_ty: &JavaType) -> bool {
    matches!(java_ty, JavaType::Primitive(_) | JavaType::String)
        || matches!(
            java_ty,
            JavaType::Named { name, .. } | JavaType::Unknown(name)
                if matches!(
                    name.rsplit('.').next().unwrap_or(name),
                    "BufferedImage" | "Graphics"
                )
        )
}

pub fn infer_member_access_type(receiver: &Expr, member: &str, env: &TypeEnv) -> Option<JavaType> {
    if matches!(receiver, Expr::This)
        && let Some(ty) = env.anonymous_this_field_type(member)
    {
        return Some(ty.clone());
    }
    let receiver_ty = infer_expr_type(receiver, env);
    if member == "length" && is_java_length_receiver(&receiver_ty) {
        Some(JavaType::Primitive(PrimitiveType::Int))
    } else if let Some(java_ty) = external_pack_field_type(&receiver_ty, member) {
        Some(java_ty)
    } else if let Some(java_ty) = source_type_field_type(&receiver_ty, member, env) {
        Some(java_ty)
    } else if is_static_type_qualifier(receiver, env)
        && let Some(java_ty) =
            crate::stdlib::known_external_field_type_for_receiver(&receiver_ty, member)
    {
        Some(java_ty)
    } else {
        None
    }
}

fn is_static_type_qualifier(receiver: &Expr, env: &TypeEnv) -> bool {
    let Expr::Var(name) = receiver else {
        return false;
    };
    if env.local_type(name).is_some() || env.field_type(name).is_some() {
        return false;
    }
    name.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
        || name.contains('_')
        || name.chars().all(|ch| ch.is_ascii_lowercase())
}

fn external_pack_field_type(receiver_ty: &JavaType, member: &str) -> Option<JavaType> {
    let receiver_name = match receiver_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        _ => return None,
    };
    crate::external_packs::external_field_for(receiver_name, member)?
        .field_type
        .clone()
}

fn source_type_field_type(receiver_ty: &JavaType, member: &str, env: &TypeEnv) -> Option<JavaType> {
    let receiver_name = match receiver_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        _ => return None,
    };
    let member_alias = crate::parser::to_snake_case(member);
    env.type_field_type(receiver_name, member)
        .or_else(|| env.type_field_type(receiver_name, &member_alias))
        .or_else(|| {
            env.static_type_alias(receiver_name)
                .and_then(|owner| env.type_field_type(owner, member))
        })
        .or_else(|| {
            env.static_type_alias(receiver_name)
                .and_then(|owner| env.type_field_type(owner, &member_alias))
        })
        .cloned()
}

fn is_java_length_receiver(ty: &JavaType) -> bool {
    matches!(ty, JavaType::Array(_) | JavaType::String)
        || optional_element_type(ty).is_some_and(|inner| is_java_length_receiver(&inner))
}

fn infer_binary_type(op: BinaryOp, left: &Expr, right: &Expr, env: &TypeEnv) -> JavaType {
    match op {
        BinaryOp::Assign => {
            assignment_target_type(left, env).unwrap_or_else(|| infer_expr_type(right, env))
        }
        BinaryOp::Eq
        | BinaryOp::Ne
        | BinaryOp::Lt
        | BinaryOp::Le
        | BinaryOp::Gt
        | BinaryOp::Ge
        | BinaryOp::And
        | BinaryOp::Or => JavaType::Primitive(PrimitiveType::Boolean),
        // Ask about the operands directly. Rebuilding the `Expr::Binary` node
        // here — deep-cloning both subtrees — made this exponential in the
        // length of a `+` chain; see the note on
        // `binary_add_operands_make_string_concat`.
        BinaryOp::Add
            if crate::string_concat::binary_add_operands_make_string_concat(left, right, env) =>
        {
            JavaType::String
        }
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
            integral_binary_type(&infer_expr_type(left, env), &infer_expr_type(right, env))
        }
        BinaryOp::Shl | BinaryOp::Shr | BinaryOp::UShr => {
            shift_binary_type(&infer_expr_type(left, env))
        }
        _ => numeric_binary_type(&infer_expr_type(left, env), &infer_expr_type(right, env)),
    }
}

fn assignment_target_type(target: &Expr, env: &TypeEnv) -> Option<JavaType> {
    match target {
        Expr::Var(name) => env
            .local_type(name)
            .cloned()
            .or_else(|| env.field_type(name).cloned()),
        Expr::FieldAccess(receiver, field) => {
            if matches!(receiver.as_ref(), Expr::This) {
                env.field_type(field).cloned()
            } else {
                None
            }
        }
        Expr::ArrayAccess(array, _) => {
            sequence_element_type_after_optional(&infer_expr_type(array, env))
        }
        _ => None,
    }
}

fn integral_binary_type(left: &JavaType, right: &JavaType) -> JavaType {
    use PrimitiveType::*;
    if matches!(left, JavaType::Primitive(Long)) || matches!(right, JavaType::Primitive(Long)) {
        JavaType::Primitive(Long)
    } else {
        JavaType::Primitive(Int)
    }
}

fn shift_binary_type(left: &JavaType) -> JavaType {
    use PrimitiveType::*;
    match left {
        JavaType::Primitive(Long) => JavaType::Primitive(Long),
        JavaType::Primitive(Byte | Short | Char | Int) => JavaType::Primitive(Int),
        other => other.clone(),
    }
}

fn numeric_binary_type(left: &JavaType, right: &JavaType) -> JavaType {
    use PrimitiveType::*;
    let rank = |ty: &JavaType| match ty {
        JavaType::Primitive(Double) => 5,
        JavaType::Primitive(Float) => 4,
        JavaType::Primitive(Long) => 3,
        JavaType::Primitive(Int | Short | Byte | Char) => 2,
        JavaType::Named { name, args } if args.is_empty() => {
            match crate::stdlib::boxed_primitive_type(name) {
                Some(Double) => 5,
                Some(Float) => 4,
                Some(Long) => 3,
                Some(Int | Short | Byte | Char) => 2,
                _ => 0,
            }
        }
        _ => 0,
    };
    match rank(left).max(rank(right)) {
        5 => JavaType::Primitive(Double),
        4 => JavaType::Primitive(Float),
        3 => JavaType::Primitive(Long),
        _ => JavaType::Primitive(Int),
    }
}

fn common_type(left: &JavaType, right: &JavaType) -> JavaType {
    if left == right {
        left.clone()
    } else if matches!(left, JavaType::Unknown(name) if name == "null")
        && !matches!(right, JavaType::Primitive(_) | JavaType::Void)
    {
        right.clone()
    } else if matches!(right, JavaType::Unknown(name) if name == "null")
        && !matches!(left, JavaType::Primitive(_) | JavaType::Void)
    {
        left.clone()
    } else if matches!(left, JavaType::Primitive(_)) && matches!(right, JavaType::Primitive(_)) {
        numeric_binary_type(left, right)
    } else {
        JavaType::Unknown("common_type".to_string())
    }
}

/// JLS §5.1.2 widening primitive conversion: may `from` become `to` without a
/// cast?
///
/// The single definition of a relation that was encoded four times — an explicit
/// 19-pair table in `lower.rs` (correct) and three *rank scales* in
/// `resolve.rs`, `conversion_plan.rs` and `operator_lowering.rs`. All three
/// scales placed `char` beside `short` on one chain, which the JLS does not:
///
///   - **nothing widens to `char`.** The rank form made `short`→`char` and
///     `char`→`short` distance 0, i.e. scored as *exact* matches in overload
///     ranking, and made `byte`→`char` a widening that emitted a conversion
///     Java rejects.
///   - **`char` widens only to `int` and above.** It is a separate root, not a
///     point on the numeric chain.
///   - **`boolean` participates in nothing.** `resolve.rs` guarded this;
///     `conversion_plan.rs` did not, so `boolean`→numeric reported `widens`.
///
/// Written as the rule rather than the extension so those three facts are
/// stated once. `primitive_widening_pairs_match_the_jls` checks the extension
/// is exactly the 19 pairs.
pub fn primitive_widens_to(from: PrimitiveType, to: PrimitiveType) -> bool {
    use PrimitiveType::*;
    if from == to {
        return false;
    }
    match (from, to) {
        (Boolean, _) | (_, Boolean) => false,
        (_, Char) => false,
        (Char, Int | Long | Float | Double) => true,
        (Char, _) => false,
        _ => numeric_widening_chain_position(from) < numeric_widening_chain_position(to),
    }
}

/// Position on the numeric widening chain `byte < short < int < long < float <
/// double`. `char` and `boolean` are not on it and are handled before this is
/// consulted.
fn numeric_widening_chain_position(primitive: PrimitiveType) -> u8 {
    use PrimitiveType::*;
    match primitive {
        Byte => 0,
        Short => 1,
        Int => 2,
        Long => 3,
        Float => 4,
        Double => 5,
        Char | Boolean => u8::MAX,
    }
}

/// How far `from` has to widen to reach `to`, for ranking overload candidates.
///
/// `Some(0)` means the types are identical. Only defined where the conversion is
/// legal, so a candidate needing an illegal conversion can never outrank one
/// that does not — which is what the rank-subtraction form got wrong.
pub fn primitive_widening_steps(from: PrimitiveType, to: PrimitiveType) -> Option<u8> {
    if from == to {
        return Some(0);
    }
    if !primitive_widens_to(from, to) {
        return None;
    }
    let from_position = numeric_widening_chain_position(from);
    let to_position = numeric_widening_chain_position(to);
    // `char` is off-chain, so its distance is measured from `int`, its nearest
    // supertype, plus one for the step onto the chain.
    if from == PrimitiveType::Char {
        return Some(1 + to_position - numeric_widening_chain_position(PrimitiveType::Int));
    }
    Some(to_position - from_position)
}

#[cfg(test)]
mod raw_rust_type_tests {
    #[test]
    fn mapped_local_source_spelling_keeps_its_declared_type() {
        let parsed = crate::parser::parse_java_source(
            "class Probe { void run() { String CURRENT = \"x\"; } }",
        );
        let crate::java_ir::Member::Method(method) = &parsed.program.types[0].members[0] else {
            panic!("missing method");
        };
        let declaration = &method.body.as_ref().unwrap().statements[0];
        let crate::java_ir::Stmt::VarDecl {
            name, source_name, ..
        } = declaration
        else {
            panic!("missing declaration");
        };
        assert_ne!(name, source_name);
        let mut env = super::TypeEnv::new();
        env.learn_stmt(declaration);
        assert_eq!(
            env.local_type(name),
            Some(&crate::java_ir::JavaType::String)
        );
        assert_eq!(
            env.local_type(source_name),
            Some(&crate::java_ir::JavaType::String)
        );
        assert_eq!(
            super::infer_expr_type(&crate::java_ir::Expr::Var(source_name.clone()), &env),
            crate::java_ir::JavaType::String
        );
    }

    use super::*;

    #[test]
    fn field_type_accepts_java_spelling_for_normalized_field_binding() {
        let mut env = TypeEnv::new();
        env.insert_field("generic_fields", JavaType::String);

        assert_eq!(env.field_type("genericFields"), Some(&JavaType::String));
    }

    #[test]
    fn raw_rust_type_args_ignore_nested_rust_delimiters_and_comments() {
        assert_eq!(
            raw_generic_type_head("ObjectAssert<Box<dyn Animal>>"),
            Some("ObjectAssert")
        );
        assert_eq!(
            raw_rust_result_args("Result<fn(i32, i32), JavaException>"),
            Some(vec!["fn(i32, i32)", "JavaException"])
        );
        assert_eq!(
            raw_rust_result_args("std::io::Result<String>")
                .expect("path-qualified Result should be parsed"),
            vec!["String"]
        );
        assert_eq!(
            raw_rust_option_inner("std::option::Option<&Schema>"),
            Some("&Schema")
        );
        assert_eq!(
            raw_rust_vec_inner("std::vec::Vec<Box<dyn Animal>>"),
            Some("Box<dyn Animal>")
        );
        assert_eq!(
            raw_rust_box_dyn_trait("std::boxed::Box<dyn Animal>"),
            Some("Animal")
        );
        assert_eq!(
            raw_rust_result_args("Result<T /*,*/, JavaException>"),
            Some(vec!["T /*,*/", "JavaException"])
        );
        assert_eq!(
            raw_rust_result_args("Result<&'a T, JavaException>"),
            Some(vec!["&'a T", "JavaException"])
        );
        assert_eq!(
            split_top_level_rust_type_args("Vec<[u8; 2]>, Result<{ const N: usize }, E>"),
            vec!["Vec<[u8; 2]>", "Result<{ const N: usize }, E>"]
        );
    }
}

#[cfg(test)]
mod primitive_widening_tests {
    use super::*;

    const ALL: [PrimitiveType; 8] = [
        PrimitiveType::Boolean,
        PrimitiveType::Byte,
        PrimitiveType::Short,
        PrimitiveType::Char,
        PrimitiveType::Int,
        PrimitiveType::Long,
        PrimitiveType::Float,
        PrimitiveType::Double,
    ];

    /// The rule's extension must be exactly the 19 pairs of JLS §5.1.2.
    ///
    /// This is the table that used to live in `lower.rs`, kept as the *check* on
    /// the rule instead of as a fourth definition of it. If the rule and the
    /// table ever disagree, one of them is wrong and this says which pair.
    #[test]
    fn primitive_widening_pairs_match_the_jls() {
        use PrimitiveType::*;
        let jls: [(PrimitiveType, PrimitiveType); 19] = [
            (Byte, Short),
            (Byte, Int),
            (Byte, Long),
            (Byte, Float),
            (Byte, Double),
            (Short, Int),
            (Short, Long),
            (Short, Float),
            (Short, Double),
            (Char, Int),
            (Char, Long),
            (Char, Float),
            (Char, Double),
            (Int, Long),
            (Int, Float),
            (Int, Double),
            (Long, Float),
            (Long, Double),
            (Float, Double),
        ];
        for from in ALL {
            for to in ALL {
                let expected = jls.contains(&(from, to));
                assert_eq!(
                    primitive_widens_to(from, to),
                    expected,
                    "{from:?} -> {to:?}"
                );
            }
        }
    }

    /// The three facts the rank scales got wrong, named individually so a
    /// regression says which one came back.
    #[test]
    fn nothing_widens_to_char() {
        for from in ALL {
            assert!(
                !primitive_widens_to(from, PrimitiveType::Char),
                "{from:?} must not widen to char"
            );
        }
    }

    #[test]
    fn char_widens_only_to_int_and_above() {
        use PrimitiveType::*;
        for to in [Int, Long, Float, Double] {
            assert!(primitive_widens_to(Char, to), "char -> {to:?}");
        }
        for to in [Boolean, Byte, Short, Char] {
            assert!(!primitive_widens_to(Char, to), "char -> {to:?}");
        }
    }

    #[test]
    fn boolean_participates_in_no_widening() {
        for other in ALL {
            assert!(!primitive_widens_to(PrimitiveType::Boolean, other));
            assert!(!primitive_widens_to(other, PrimitiveType::Boolean));
        }
    }

    /// Widening is a strict partial order: irreflexive, antisymmetric and
    /// transitive. Asserting the laws catches a wrong pair that the 19-pair
    /// check would only catch if someone also fixed the list.
    #[test]
    fn widening_is_a_strict_partial_order() {
        for a in ALL {
            assert!(!primitive_widens_to(a, a), "irreflexive: {a:?}");
            for b in ALL {
                if primitive_widens_to(a, b) {
                    assert!(!primitive_widens_to(b, a), "antisymmetric: {a:?} <-> {b:?}");
                }
                for c in ALL {
                    if primitive_widens_to(a, b) && primitive_widens_to(b, c) {
                        assert!(
                            primitive_widens_to(a, c),
                            "transitive: {a:?} -> {b:?} -> {c:?}"
                        );
                    }
                }
            }
        }
    }

    /// Distance is only defined where the conversion is legal, so a candidate
    /// needing an illegal conversion can never outrank one that does not —
    /// which is exactly what rank subtraction got wrong for `char`/`short`.
    #[test]
    fn widening_distance_is_defined_exactly_where_widening_is() {
        for from in ALL {
            for to in ALL {
                let steps = primitive_widening_steps(from, to);
                if from == to {
                    assert_eq!(steps, Some(0), "{from:?} -> {to:?}");
                } else {
                    assert_eq!(
                        steps.is_some(),
                        primitive_widens_to(from, to),
                        "{from:?} -> {to:?}"
                    );
                }
            }
        }
        // The specific regression: neither direction is an exact match.
        assert_eq!(
            primitive_widening_steps(PrimitiveType::Char, PrimitiveType::Short),
            None
        );
        assert_eq!(
            primitive_widening_steps(PrimitiveType::Short, PrimitiveType::Char),
            None
        );
    }

    /// A nearer widening ranks better than a further one, which is the whole
    /// point of the distance.
    #[test]
    fn distance_grows_along_the_chain() {
        use PrimitiveType::*;
        let byte_to_short = primitive_widening_steps(Byte, Short).unwrap();
        let byte_to_double = primitive_widening_steps(Byte, Double).unwrap();
        assert!(byte_to_short < byte_to_double);
        assert!(
            primitive_widening_steps(Char, Int).unwrap()
                < primitive_widening_steps(Char, Double).unwrap()
        );
    }

    #[test]
    fn bound_field_receiver_resolution_preserves_arguments_and_rejects_cycles() {
        let mut env = TypeEnv::new();
        let bound = crate::parser::parse_hierarchy_type("Holder<Vertex>");
        env.insert_type_param_bound("E", JavaType::Unknown("F".into()));
        env.insert_type_param_bound("F", bound.clone());
        assert_eq!(
            env.type_parameter_receiver_bound(&optional_java_type(JavaType::Unknown("E".into()))),
            Some(bound),
        );
        assert_eq!(
            env.type_parameter_receiver_bound(&JavaType::Unknown("Holder".into())),
            None
        );
        env.insert_type_param_bound("F", JavaType::Unknown("E".into()));
        assert_eq!(
            env.type_parameter_receiver_bound(&JavaType::Unknown("E".into())),
            None
        );
        env.insert_type_param_declaration("F", None);
        assert_eq!(
            env.type_parameter_receiver_bound(&JavaType::Unknown("E".into())),
            None
        );
        // The binder has lexical precedence even if a same-name source field
        // could otherwise make a lookup appear successful.
        env.insert_type_field("F", "item", JavaType::String);
        assert_eq!(
            env.type_parameter_receiver_bound(&JavaType::Unknown("F".into())),
            None
        );
    }

    #[test]
    fn unbounded_binder_registration_shadows_bounds_without_guessing_type_names() {
        let mut outer = TypeEnv::new();
        let ty = crate::parser::parse_hierarchy_type("T");
        assert!(!outer.is_unbounded_object_type_parameter(&ty));
        outer.insert_type_param_bound("T", crate::parser::parse_hierarchy_type("Runnable"));
        assert!(!outer.is_unbounded_object_type_parameter(&ty));
        let mut method_scope = outer.clone();
        method_scope.insert_type_param_declaration("T", None);
        assert!(method_scope.is_unbounded_object_type_parameter(&ty));
        assert!(method_scope.is_unbounded_object_type_parameter(&optional_java_type(ty.clone())));
        assert!(outer.type_param_bound("T").is_some());
        assert!(method_scope.type_param_bound("T").is_none());
        assert!(
            !method_scope.is_unbounded_object_type_parameter(&JavaType::Named {
                name: "pkg.T".into(),
                args: Vec::new(),
            })
        );
        method_scope
            .insert_type_param_bound("U", crate::parser::parse_hierarchy_type("java.lang.Object"));
        assert!(
            method_scope
                .is_unbounded_object_type_parameter(&crate::parser::parse_hierarchy_type("U"))
        );
    }

    #[test]
    fn this_type_is_lexical_and_anonymous_scopes_do_not_inherit_it() {
        let mut outer = TypeEnv::new();
        let ty = crate::parser::parse_hierarchy_type("Owner<T>");
        outer.set_this_type(ty.clone());
        assert_eq!(infer_expr_type(&Expr::This, &outer), ty);
        let mut anonymous = outer.clone();
        anonymous.enter_anonymous_field_scope([]);
        assert_eq!(
            infer_expr_type(&Expr::This, &anonymous),
            JavaType::Unknown("this".into())
        );
        assert_eq!(infer_expr_type(&Expr::This, &outer), ty);
    }
}
