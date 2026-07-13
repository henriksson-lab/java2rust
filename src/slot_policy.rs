use crate::facts::FactSet;
use crate::java_ir::{ConstructorDecl, JavaType, MethodDecl, Modifier, Param, TypeDecl};
use crate::representation::RepresentationPlan;
use crate::shape_projection::{
    apply_borrowed_sequence_content_projection, apply_borrowed_string_content_projection,
    value_shape_for_java_type,
};
use crate::slot_shape::RustSlotShape;
use crate::usage::{
    BindingUse, CallArgumentBoundary, MethodUsageFacts, UsageConstructorKey, UsageFacts,
    UsageMethodKey,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowedContentKind {
    StringLike,
    SequenceLike,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowedContentPolicy {
    ContentView { kind: BorrowedContentKind },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowedContentCapabilityPolicy {
    pub java_ty: JavaType,
    pub string_content: bool,
    pub sequence_element: Option<JavaType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowedContentTypePolicy {
    pub java_ty: JavaType,
    pub policy: Option<BorrowedContentPolicy>,
}

impl BorrowedContentPolicy {
    pub const fn string_like_view() -> Self {
        Self::ContentView {
            kind: BorrowedContentKind::StringLike,
        }
    }

    pub const fn sequence_like_view() -> Self {
        Self::ContentView {
            kind: BorrowedContentKind::SequenceLike,
        }
    }
}

pub fn borrowed_content_capability_policy(
    java_ty: &JavaType,
    facts: &FactSet,
) -> BorrowedContentCapabilityPolicy {
    BorrowedContentCapabilityPolicy {
        java_ty: java_ty.clone(),
        string_content: facts.is_string_like_type(java_ty),
        sequence_element: facts.sequence_element_type_for(java_ty),
    }
}

pub fn method_param_borrowed_content_policy(
    ty: &TypeDecl,
    method: &MethodDecl,
    param: &Param,
    usage: &UsageFacts,
) -> Option<BorrowedContentPolicy> {
    method_param_borrowed_content_policy_with_facts(ty, method, param, usage, &FactSet::default())
}

pub fn method_param_borrowed_content_policy_with_facts(
    ty: &TypeDecl,
    method: &MethodDecl,
    param: &Param,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Option<BorrowedContentPolicy> {
    if method.modifiers.contains(&Modifier::Public) {
        return None;
    }
    if method_param_allows_borrowed_content(ty, method, param, usage, facts)
        || method_return_allows_borrowed_param_content(ty, method, param, usage, facts)
    {
        borrowed_content_type_policy(&param.ty, facts).policy
    } else {
        None
    }
}

pub fn constructor_param_borrowed_content_policy(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    param: &Param,
    usage: &UsageFacts,
) -> Option<BorrowedContentPolicy> {
    constructor_param_borrowed_content_policy_with_facts(
        ty,
        ctor,
        param,
        usage,
        &FactSet::default(),
    )
}

pub fn constructor_param_borrowed_content_policy_with_facts(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    param: &Param,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Option<BorrowedContentPolicy> {
    if ctor.modifiers.contains(&Modifier::Public)
        || borrowed_content_type_policy(&param.ty, facts)
            .policy
            .is_none()
    {
        return None;
    }
    let rust_name = ctor.rust_name.clone().unwrap_or_else(|| "new".to_string());
    let ctor_usage = usage.constructors.get(&UsageConstructorKey {
        type_name: ty.name.clone(),
        rust_name,
    })?;
    if usage_allows_borrowed_content(ctor_usage, &param.name) {
        borrowed_content_type_policy(&param.ty, facts).policy
    } else {
        None
    }
}

pub fn apply_borrowed_content_policy(shape: &mut RustSlotShape, policy: BorrowedContentPolicy) {
    match policy {
        BorrowedContentPolicy::ContentView {
            kind: BorrowedContentKind::StringLike,
        } => {
            apply_borrowed_string_content_projection(shape);
        }
        BorrowedContentPolicy::ContentView {
            kind: BorrowedContentKind::SequenceLike,
        } => {
            apply_borrowed_sequence_content_projection(shape);
        }
    }
}

pub fn borrowed_return_shape_from_param(
    ty: &TypeDecl,
    method: &MethodDecl,
    representations: &RepresentationPlan,
    usage: &UsageFacts,
) -> Option<RustSlotShape> {
    borrowed_return_shape_from_param_with_facts(
        ty,
        method,
        representations,
        usage,
        &FactSet::default(),
    )
}

pub fn borrowed_return_content_policy_from_param(
    ty: &TypeDecl,
    method: &MethodDecl,
    usage: &UsageFacts,
) -> Option<BorrowedContentPolicy> {
    borrowed_return_content_policy_from_param_with_facts(ty, method, usage, &FactSet::default())
}

pub fn borrowed_return_content_policy_from_param_with_facts(
    ty: &TypeDecl,
    method: &MethodDecl,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Option<BorrowedContentPolicy> {
    method.params.iter().find_map(|param| {
        if method_return_allows_borrowed_param_content(ty, method, param, usage, facts) {
            borrowed_content_type_policy(&param.ty, facts).policy
        } else {
            None
        }
    })
}

pub fn borrowed_return_shape_from_param_with_facts(
    ty: &TypeDecl,
    method: &MethodDecl,
    representations: &RepresentationPlan,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Option<RustSlotShape> {
    let param = method.params.iter().find(|param| {
        method_return_allows_borrowed_param_content(ty, method, param, usage, facts)
    })?;
    let mut shape = value_shape_for_java_type(representations, param.ty.clone());
    apply_borrowed_content_policy(
        &mut shape,
        borrowed_return_content_policy_from_param_with_facts(ty, method, usage, facts)?,
    );
    Some(shape)
}

pub fn borrowed_content_type_policy(
    java_ty: &JavaType,
    facts: &FactSet,
) -> BorrowedContentTypePolicy {
    let capability = borrowed_content_capability_policy(java_ty, facts);
    let policy = if capability.string_content {
        Some(BorrowedContentPolicy::string_like_view())
    } else if capability.sequence_element.is_some() {
        Some(BorrowedContentPolicy::sequence_like_view())
    } else {
        None
    };
    BorrowedContentTypePolicy {
        java_ty: capability.java_ty,
        policy,
    }
}

fn method_param_allows_borrowed_content(
    ty: &TypeDecl,
    method: &MethodDecl,
    param: &Param,
    usage: &UsageFacts,
    facts: &FactSet,
) -> bool {
    if borrowed_content_type_policy(&param.ty, facts)
        .policy
        .is_none()
    {
        return false;
    }
    let Some(method_usage) = usage.methods.get(&UsageMethodKey {
        type_name: ty.name.clone(),
        method_name: method.name.clone(),
    }) else {
        return false;
    };
    usage_allows_borrowed_content(method_usage, &param.name)
}

fn method_return_allows_borrowed_param_content(
    ty: &TypeDecl,
    method: &MethodDecl,
    param: &Param,
    usage: &UsageFacts,
    facts: &FactSet,
) -> bool {
    if borrowed_content_type_policy(&param.ty, facts)
        .policy
        .is_none()
        || borrowed_content_type_policy(&method.return_type, facts)
            .policy
            .is_none()
        || method.modifiers.contains(&Modifier::Public)
        || !method.modifiers.contains(&Modifier::Static)
    {
        return false;
    }
    let Some(method_usage) = usage.methods.get(&UsageMethodKey {
        type_name: ty.name.clone(),
        method_name: method.name.clone(),
    }) else {
        return false;
    };
    let Some(binding) = method_usage.bindings.get(&param.name) else {
        return false;
    };
    !binding.uses.is_empty()
        && binding
            .uses
            .iter()
            .all(|usage| matches!(usage, BindingUse::Returned))
}

fn usage_allows_borrowed_content(usage: &MethodUsageFacts, param_name: &str) -> bool {
    let Some(binding) = usage.bindings.get(param_name) else {
        return false;
    };
    !binding.uses.is_empty()
        && binding.uses.iter().all(|usage| {
            matches!(
                usage,
                BindingUse::ContentRead
                    | BindingUse::PassedToCall {
                        boundary: CallArgumentBoundary::KnownContentRead,
                        ..
                    }
            )
        })
}
