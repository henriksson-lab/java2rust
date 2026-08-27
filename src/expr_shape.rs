use std::collections::{HashMap, HashSet};

use crate::binding_id::{BindingId, BindingKind};
use crate::facts::FactSet;
use crate::java_ir::{Expr, JavaType, Member, MethodDecl, Modifier, Param, Stmt, TypeDecl};
use crate::java_signature::{JavaCallableKey, JavaOwnerKey};
use crate::representation::RepresentationPlan;
use crate::shape_projection::{
    borrowed_string_content_shape, indexed_assignment_shape_with_facts,
    indexed_read_shape_for_access_with_facts,
    project_java_value_shape as project_java_value_shape_with_representations,
    shape_is_erased_object_boundary,
};
use crate::signature::{
    ConstructorSignaturePlan, LocalPlan, MethodSignaturePlan, ParamPlan, SignaturePlan,
};
use crate::slot_shape::RustSlotShape;
use crate::types::{TypeEnv, infer_expr_type};

#[derive(Debug, Clone, Default)]
pub struct ShapeEnv {
    types: TypeEnv,
    local_shapes: HashMap<String, RustSlotShape>,
    field_shapes: HashMap<String, RustSlotShape>,
    static_field_names: HashMap<String, String>,
    static_field_access_names: HashMap<String, String>,
    static_field_owners: HashMap<String, String>,
    static_storage_field_names: HashMap<String, String>,
    static_storage_field_access_names: HashMap<String, String>,
    /// Declaring type of each static-storage field, so a reference from a
    /// *nested* class qualifies the accessor with the class that owns it
    /// instead of assuming `Self`
    /// ([0037](../claude_audits/done/0037-nested-class-outer-static-read-is-capture.md)).
    static_storage_field_owners: HashMap<String, String>,
    /// The type whose members this environment was built for, when known.
    current_type: Option<String>,
    type_static_storage_field_names: HashMap<(String, String), String>,
    type_static_storage_field_access_names: HashMap<(String, String), String>,
    instance_field_access_names: HashMap<String, String>,
    instance_field_names: HashSet<String>,
    binding_access_names: HashMap<String, String>,
    binding_ids: HashMap<String, BindingId>,
    binding_occurrences: HashMap<(BindingKind, String), usize>,
    map_key_set_view_receivers: HashMap<String, Expr>,
    map_values_view_receivers: HashMap<String, Expr>,
}

impl ShapeEnv {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_env(&self) -> &TypeEnv {
        &self.types
    }

    pub fn insert_map_key_set_view_receiver(&mut self, name: impl Into<String>, receiver: Expr) {
        self.map_key_set_view_receivers
            .insert(name.into(), receiver);
    }

    pub fn insert_map_values_view_receiver(&mut self, name: impl Into<String>, receiver: Expr) {
        self.map_values_view_receivers.insert(name.into(), receiver);
    }

    fn clear_map_view_receivers(&mut self, name: &str) {
        self.map_key_set_view_receivers.remove(name);
        self.map_values_view_receivers.remove(name);
    }

    pub fn map_key_set_view_receiver(&self, name: &str) -> Option<&Expr> {
        self.map_key_set_view_receivers.get(name)
    }

    pub fn map_values_view_receiver(&self, name: &str) -> Option<&Expr> {
        self.map_values_view_receivers.get(name)
    }

    pub fn insert_local(&mut self, name: impl Into<String>, ty: JavaType) {
        let name = name.into();
        self.clear_map_view_receivers(&name);
        self.local_shapes
            .insert(name.clone(), RustSlotShape::for_storage(ty.clone()));
        self.types.insert_local(name.clone(), ty);
        self.bind_name(BindingKind::Local, name);
    }

    pub fn insert_field(&mut self, name: impl Into<String>, ty: JavaType) {
        let name = name.into();
        self.field_shapes
            .insert(name.clone(), RustSlotShape::for_storage(ty.clone()));
        self.types.insert_field(name.clone(), ty);
        self.instance_field_names
            .insert(crate::parser::to_snake_case(&name));
        self.bind_name(BindingKind::Field, name);
    }

    pub fn insert_local_shape(&mut self, name: impl Into<String>, shape: RustSlotShape) {
        let name = name.into();
        self.clear_map_view_receivers(&name);
        self.types.insert_local(name.clone(), shape.java_ty.clone());
        self.local_shapes.insert(name.clone(), shape);
        self.bind_name(BindingKind::Local, name);
    }

    pub fn insert_local_shape_with_java_type(
        &mut self,
        name: impl Into<String>,
        shape: RustSlotShape,
        java_ty: JavaType,
    ) {
        let name = name.into();
        self.clear_map_view_receivers(&name);
        self.types.insert_local(name.clone(), java_ty);
        self.local_shapes.insert(name.clone(), shape);
        self.bind_name(BindingKind::Local, name);
    }

    pub fn insert_local_shape_with_java_type_and_binding(
        &mut self,
        name: impl Into<String>,
        shape: RustSlotShape,
        java_ty: JavaType,
        binding: Option<BindingId>,
    ) {
        let name = name.into();
        self.clear_map_view_receivers(&name);
        self.types.insert_local(name.clone(), java_ty);
        self.local_shapes.insert(name.clone(), shape);
        if let Some(binding) = binding {
            self.binding_ids.insert(name, binding);
        } else {
            self.bind_name(BindingKind::Local, name);
        }
    }

    pub fn insert_local_shape_with_access_name_and_java_type(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
        java_ty: JavaType,
    ) {
        let name = name.into();
        let access_name = access_name.into();
        self.clear_map_view_receivers(&access_name);
        self.insert_local_shape_with_java_type(name.clone(), shape, java_ty);
        self.binding_access_names
            .insert(name.clone(), access_name.clone());
        self.binding_access_names
            .insert(access_name.clone(), access_name);
    }

    pub fn insert_local_shape_with_access_name_java_type_and_binding(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
        java_ty: JavaType,
        binding: Option<BindingId>,
    ) {
        let name = name.into();
        let access_name = access_name.into();
        self.clear_map_view_receivers(&access_name);
        self.insert_local_shape_with_java_type_and_binding(name.clone(), shape, java_ty, binding);
        self.binding_access_names
            .insert(name.clone(), access_name.clone());
        self.binding_access_names
            .insert(access_name.clone(), access_name);
    }

    pub fn insert_param_shape_with_java_type(
        &mut self,
        name: impl Into<String>,
        shape: RustSlotShape,
        java_ty: JavaType,
    ) {
        let name = name.into();
        self.types.insert_local(name.clone(), java_ty);
        self.local_shapes.insert(name.clone(), shape);
        self.bind_name(BindingKind::Param, name);
    }

    pub fn insert_param_shape_with_java_type_and_binding(
        &mut self,
        name: impl Into<String>,
        shape: RustSlotShape,
        java_ty: JavaType,
        binding: Option<BindingId>,
    ) {
        let name = name.into();
        self.types.insert_local(name.clone(), java_ty);
        self.local_shapes.insert(name.clone(), shape);
        if let Some(binding) = binding {
            self.binding_ids.insert(name, binding);
        } else {
            self.bind_name(BindingKind::Param, name);
        }
    }

    pub fn insert_param_shape_with_access_name_and_java_type(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
        java_ty: JavaType,
    ) {
        let name = name.into();
        let access_name = access_name.into();
        self.insert_param_shape_with_java_type(name.clone(), shape, java_ty);
        self.binding_access_names
            .insert(name.clone(), access_name.clone());
        self.binding_access_names
            .insert(access_name.clone(), access_name);
    }

    pub fn insert_param_shape_with_access_name_java_type_and_binding(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
        java_ty: JavaType,
        binding: Option<BindingId>,
    ) {
        let name = name.into();
        let access_name = access_name.into();
        self.insert_param_shape_with_java_type_and_binding(name.clone(), shape, java_ty, binding);
        self.binding_access_names
            .insert(name.clone(), access_name.clone());
        self.binding_access_names
            .insert(access_name.clone(), access_name);
    }

    pub fn insert_loop_variable_shape(&mut self, name: impl Into<String>, shape: RustSlotShape) {
        let name = name.into();
        self.types.insert_local(name.clone(), shape.java_ty.clone());
        self.local_shapes.insert(name.clone(), shape);
        self.bind_name(BindingKind::LoopVariable, name);
    }

    pub fn insert_loop_variable_shape_with_access_name(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
    ) {
        let name = name.into();
        let access_name = access_name.into();
        self.insert_loop_variable_shape(name.clone(), shape);
        self.binding_access_names
            .insert(name.clone(), access_name.clone());
        self.binding_access_names
            .insert(access_name.clone(), access_name);
    }

    pub fn insert_loop_variable_shape_with_access_name_and_binding(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
        binding: Option<BindingId>,
    ) {
        let name = name.into();
        let access_name = access_name.into();
        self.types.insert_local(name.clone(), shape.java_ty.clone());
        self.local_shapes.insert(name.clone(), shape);
        if let Some(binding) = binding {
            self.binding_ids.insert(name.clone(), binding);
        } else {
            self.bind_name(BindingKind::LoopVariable, name.clone());
        }
        self.binding_access_names
            .insert(name.clone(), access_name.clone());
        self.binding_access_names
            .insert(access_name.clone(), access_name);
    }

    pub fn insert_catch_param(&mut self, name: impl Into<String>, ty: JavaType) {
        let name = name.into();
        self.local_shapes
            .insert(name.clone(), RustSlotShape::for_storage(ty.clone()));
        self.types.insert_local(name.clone(), ty);
        self.bind_name(BindingKind::CatchParam, name);
    }

    pub fn insert_catch_param_with_binding(
        &mut self,
        name: impl Into<String>,
        ty: JavaType,
        binding: Option<BindingId>,
    ) {
        let name = name.into();
        self.local_shapes
            .insert(name.clone(), RustSlotShape::for_storage(ty.clone()));
        self.types.insert_local(name.clone(), ty);
        if let Some(binding) = binding {
            self.binding_ids.insert(name, binding);
        } else {
            self.bind_name(BindingKind::CatchParam, name);
        }
    }

    pub fn insert_catch_param_shape(&mut self, name: impl Into<String>, shape: RustSlotShape) {
        let name = name.into();
        self.types.insert_local(name.clone(), shape.java_ty.clone());
        self.local_shapes.insert(name.clone(), shape);
        self.bind_name(BindingKind::CatchParam, name);
    }

    pub fn insert_static_type_alias(
        &mut self,
        alias: impl Into<String>,
        type_name: impl Into<String>,
    ) {
        self.types.insert_static_type_alias(alias, type_name);
    }

    pub fn insert_static_type_alias_if_absent(
        &mut self,
        alias: impl Into<String>,
        type_name: impl Into<String>,
    ) {
        self.types
            .insert_static_type_alias_if_absent(alias, type_name);
    }

    pub fn insert_static_member_import(
        &mut self,
        member: impl Into<String>,
        owner_type: impl Into<String>,
    ) {
        self.types.insert_static_member_import(member, owner_type);
    }

    pub fn insert_type_param_bound(&mut self, name: impl Into<String>, bound: JavaType) {
        self.types.insert_type_param_bound(name, bound);
    }

    pub fn insert_field_shape(&mut self, name: impl Into<String>, shape: RustSlotShape) {
        let name = name.into();
        let alias = crate::parser::to_snake_case(&name);
        self.types.insert_field(name.clone(), shape.java_ty.clone());
        self.field_shapes.insert(name.clone(), shape.clone());
        self.instance_field_names.insert(name.clone());
        if alias != name {
            if self.types.field_type(&alias).is_none() {
                self.types
                    .insert_field(alias.clone(), shape.java_ty.clone());
            }
            self.field_shapes.entry(alias.clone()).or_insert(shape);
            self.instance_field_names.insert(alias.clone());
        }
        let binding = self.bind_name(BindingKind::Field, name.clone());
        let alias = crate::parser::to_snake_case(&name);
        if alias != name {
            self.binding_ids.entry(alias).or_insert(binding);
        }
    }

    pub fn insert_field_shape_with_access_name(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
    ) {
        let name = name.into();
        let access_name = access_name.into();
        let alias = crate::parser::to_snake_case(&name);
        self.insert_field_shape(name.clone(), shape);
        self.instance_field_access_names
            .insert(name.clone(), access_name.clone());
        if alias != name {
            self.instance_field_access_names
                .entry(alias.clone())
                .or_insert(access_name.clone());
        }
        self.instance_field_access_names
            .insert(access_name.clone(), access_name);
    }

    pub fn insert_field_shape_with_access_name_and_java_type(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
        java_ty: JavaType,
    ) {
        let name = name.into();
        let access_name = access_name.into();
        let alias = crate::parser::to_snake_case(&name);
        self.insert_field_shape_with_access_name(name.clone(), access_name, shape);
        self.types.insert_field(name, java_ty.clone());
        if self.types.field_type(&alias).is_none() {
            self.types.insert_field(alias, java_ty);
        }
    }

    pub fn insert_type_field_shape(
        &mut self,
        type_name: impl Into<String>,
        field_name: impl Into<String>,
        shape: RustSlotShape,
    ) {
        let type_name = type_name.into();
        let field_name = field_name.into();
        let type_name_alias = crate::parser::to_snake_case(&type_name);
        let alias = crate::parser::to_snake_case(&field_name);
        self.types
            .insert_type_field(type_name.clone(), field_name.clone(), shape.java_ty.clone());
        self.types
            .insert_type_field(type_name.clone(), alias.clone(), shape.java_ty.clone());
        if type_name_alias != type_name {
            self.types.insert_type_field(
                type_name_alias.clone(),
                field_name,
                shape.java_ty.clone(),
            );
            self.types
                .insert_type_field(type_name_alias, alias, shape.java_ty.clone());
        }
    }

    pub fn insert_type_field_shape_with_java_type(
        &mut self,
        type_name: impl Into<String>,
        field_name: impl Into<String>,
        shape: RustSlotShape,
        java_ty: JavaType,
    ) {
        let type_name = type_name.into();
        let field_name = field_name.into();
        let type_name_alias = crate::parser::to_snake_case(&type_name);
        let field_alias = crate::parser::to_snake_case(&field_name);
        self.insert_type_field_shape(type_name.clone(), field_name.clone(), shape);
        self.types
            .insert_type_field(type_name.clone(), field_name.clone(), java_ty.clone());
        self.types
            .insert_type_field(type_name.clone(), field_alias.clone(), java_ty.clone());
        if type_name_alias != type_name {
            self.types
                .insert_type_field(type_name_alias.clone(), field_name, java_ty.clone());
            self.types
                .insert_type_field(type_name_alias, field_alias, java_ty);
        }
    }

    pub fn insert_type_static_storage_field_shape_with_access_name(
        &mut self,
        type_name: impl Into<String>,
        field_name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
    ) {
        let type_name = type_name.into();
        let field_name = field_name.into();
        let access_name = access_name.into();
        let type_name_alias = crate::parser::to_snake_case(&type_name);
        let alias = crate::parser::to_snake_case(&field_name);
        self.insert_type_field_shape(type_name.clone(), field_name.clone(), shape);
        self.type_static_storage_field_names
            .insert((type_name.clone(), field_name.clone()), field_name.clone());
        self.type_static_storage_field_names
            .entry((type_name.clone(), alias.clone()))
            .or_insert_with(|| field_name.clone());
        if type_name_alias != type_name {
            self.type_static_storage_field_names.insert(
                (type_name_alias.clone(), field_name.clone()),
                field_name.clone(),
            );
            self.type_static_storage_field_names
                .entry((type_name_alias, alias))
                .or_insert_with(|| field_name.clone());
        }
        self.type_static_storage_field_access_names
            .insert((type_name.clone(), field_name.clone()), access_name.clone());
        self.type_static_storage_field_access_names.insert(
            (crate::parser::to_snake_case(&type_name), field_name.clone()),
            access_name.clone(),
        );
        for key in [
            (type_name.clone(), crate::parser::to_snake_case(&field_name)),
            (
                crate::parser::to_snake_case(&type_name),
                crate::parser::to_snake_case(&field_name),
            ),
        ] {
            self.type_static_storage_field_access_names
                .entry(key)
                .or_insert_with(|| access_name.clone());
        }
    }

    pub fn insert_type_static_storage_field_shape_with_access_name_and_java_type(
        &mut self,
        type_name: impl Into<String>,
        field_name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
        java_ty: JavaType,
    ) {
        let type_name = type_name.into();
        let field_name = field_name.into();
        let type_name_alias = crate::parser::to_snake_case(&type_name);
        let field_alias = crate::parser::to_snake_case(&field_name);
        self.insert_type_static_storage_field_shape_with_access_name(
            type_name.clone(),
            field_name.clone(),
            access_name,
            shape,
        );
        self.types
            .insert_type_field(type_name.clone(), field_name.clone(), java_ty.clone());
        self.types
            .insert_type_field(type_name.clone(), field_alias.clone(), java_ty.clone());
        if type_name_alias != type_name {
            self.types
                .insert_type_field(type_name_alias.clone(), field_name, java_ty.clone());
            self.types
                .insert_type_field(type_name_alias, field_alias, java_ty);
        }
    }

    pub fn insert_static_field_shape(&mut self, name: impl Into<String>, shape: RustSlotShape) {
        let name = name.into();
        self.insert_static_field_shape_with_access_name(name.clone(), name, shape);
    }

    pub fn insert_static_field_shape_with_access_name(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
    ) {
        let name = name.into();
        let access_name = access_name.into();
        let shape = if matches!(shape.java_ty, JavaType::String) {
            borrowed_string_content_shape()
        } else {
            shape
        };
        let alias = crate::parser::to_snake_case(&name);
        self.types.insert_field(name.clone(), shape.java_ty.clone());
        self.field_shapes.insert(name.clone(), shape.clone());
        self.types
            .insert_field(alias.clone(), shape.java_ty.clone());
        self.field_shapes.insert(alias.clone(), shape);
        let binding = self.bind_name(BindingKind::StaticField, name.clone());
        self.binding_ids.insert(alias.clone(), binding);
        self.static_field_names.insert(name.clone(), name.clone());
        self.static_field_names
            .entry(alias.clone())
            .or_insert_with(|| name.clone());
        self.static_field_access_names
            .insert(name.clone(), access_name.clone());
        self.static_field_access_names
            .entry(alias)
            .or_insert(access_name);
    }

    pub fn insert_static_field_shape_with_access_name_and_java_type(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
        java_ty: JavaType,
    ) {
        let name = name.into();
        let alias = crate::parser::to_snake_case(&name);
        self.insert_static_field_shape_with_access_name(name.clone(), access_name, shape);
        self.types.insert_field(name, java_ty.clone());
        self.types.insert_field(alias, java_ty);
    }

    pub fn insert_static_field_shape_for_owner_with_access_name(
        &mut self,
        owner: impl Into<String>,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
    ) {
        let owner = owner.into();
        let name = name.into();
        let access_name = access_name.into();
        let alias = crate::parser::to_snake_case(&name);
        self.insert_static_field_shape_with_access_name(name.clone(), access_name.clone(), shape);
        self.static_field_owners.insert(name.clone(), owner.clone());
        self.static_field_owners
            .insert(access_name.clone(), owner.clone());
        self.static_field_owners.entry(alias).or_insert(owner);
    }

    pub fn insert_static_storage_field_shape_with_access_name(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
    ) {
        let name = name.into();
        let access_name = access_name.into();
        let alias = crate::parser::to_snake_case(&name);
        self.types.insert_field(name.clone(), shape.java_ty.clone());
        self.field_shapes.insert(name.clone(), shape.clone());
        self.types
            .insert_field(alias.clone(), shape.java_ty.clone());
        self.field_shapes.insert(alias.clone(), shape);
        let binding = self.bind_name(BindingKind::StaticField, name.clone());
        self.binding_ids.insert(alias.clone(), binding);
        self.static_storage_field_names
            .insert(name.clone(), name.clone());
        self.static_storage_field_names
            .entry(alias.clone())
            .or_insert_with(|| name.clone());
        self.static_storage_field_access_names
            .insert(name.clone(), access_name.clone());
        self.static_storage_field_access_names
            .entry(alias)
            .or_insert(access_name);
    }

    pub fn insert_static_storage_field_shape_owned_by_with_access_name(
        &mut self,
        owner: &str,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
    ) {
        let name = name.into();
        let access_name = access_name.into();
        let alias = crate::parser::to_snake_case(&name);
        self.insert_static_storage_field_shape_with_access_name(
            name.clone(),
            access_name.clone(),
            shape,
        );
        self.static_storage_field_owners
            .insert(name.clone(), owner.to_string());
        self.static_storage_field_owners
            .insert(access_name, owner.to_string());
        self.static_storage_field_owners
            .entry(alias)
            .or_insert_with(|| owner.to_string());
    }

    /// The type declaring the static-storage field `name`, if known.
    pub fn static_storage_field_owner(&self, name: &str) -> Option<&str> {
        self.static_storage_field_owners
            .get(name)
            .map(String::as_str)
    }

    /// Records the type this environment lowers members of.
    pub fn set_current_type(&mut self, name: impl Into<String>) {
        self.current_type = Some(name.into());
    }

    /// The Rust type to qualify an unqualified static-storage read with, or
    /// `None` when `Self` is right.
    ///
    /// A nested class's environment also carries the enclosing class's statics
    /// (they are in scope in Java), so `Self` names the wrong type whenever the
    /// declaring type is not the one being lowered
    /// ([0037](../claude_audits/done/0037-nested-class-outer-static-read-is-capture.md)).
    pub fn static_storage_accessor_qualifier(&self, name: &str) -> Option<String> {
        let owner = self.static_storage_field_owners.get(name)?;
        let current = self.current_type.as_ref()?;
        if owner == current {
            return None;
        }
        // The current type declaring it under its own key wins over a stale
        // cross-type record, whichever order the two seedings ran in.
        if self
            .type_static_storage_field_names
            .contains_key(&(current.clone(), name.to_string()))
        {
            return None;
        }
        Some(crate::slot_render::rust_name_for_java_named_type(owner))
    }

    pub fn insert_static_storage_field_shape_with_access_name_and_java_type(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
        java_ty: JavaType,
    ) {
        let name = name.into();
        let alias = crate::parser::to_snake_case(&name);
        self.insert_static_storage_field_shape_with_access_name(name.clone(), access_name, shape);
        self.types.insert_field(name, java_ty.clone());
        self.types.insert_field(alias, java_ty);
    }

    pub fn binding_id(&self, name: &str) -> Option<&BindingId> {
        self.binding_ids.get(name)
    }

    fn bind_name(&mut self, kind: BindingKind, name: String) -> BindingId {
        let key = (kind, name.clone());
        let occurrence = self.binding_occurrences.entry(key).or_insert(0);
        let id = BindingId::new(kind, name.clone(), *occurrence);
        *occurrence += 1;
        self.binding_ids.insert(name, id.clone());
        id
    }

    pub fn local_type(&self, name: &str) -> Option<&JavaType> {
        self.types.local_type(name)
    }

    pub fn insert_binding_access_name(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
    ) {
        self.binding_access_names
            .insert(name.into(), access_name.into());
    }

    pub fn binding_access_name(&self, name: &str) -> Option<&str> {
        self.binding_access_names.get(name).map(String::as_str)
    }

    pub fn field_type(&self, name: &str) -> Option<&JavaType> {
        self.types.field_type(name).or_else(|| {
            name.strip_prefix("this.")
                .and_then(|name| self.types.field_type(name))
        })
    }

    pub fn local_shape(&self, name: &str) -> Option<&RustSlotShape> {
        self.local_shapes.get(name)
    }

    pub fn next_local_plan_for_declared_type<'a>(
        &mut self,
        locals: &'a [LocalPlan],
        name: &str,
        declared_ty: &JavaType,
    ) -> Option<&'a LocalPlan> {
        unique_local_plan_for_declared_type(locals, name, declared_ty)
    }

    pub fn field_shape(&self, name: &str) -> Option<&RustSlotShape> {
        self.field_shapes.get(name).or_else(|| {
            name.strip_prefix("this.")
                .and_then(|name| self.field_shapes.get(name))
        })
    }

    pub fn is_static_field(&self, name: &str) -> bool {
        self.static_field_names.contains_key(name)
            || self.static_storage_field_names.contains_key(name)
    }

    pub fn is_instance_field(&self, name: &str) -> bool {
        self.instance_field_names.contains(name)
    }

    pub fn instance_field_access_name(&self, name: &str) -> Option<&str> {
        self.instance_field_access_names
            .get(name)
            .map(String::as_str)
    }

    pub fn static_field_access_name(&self, name: &str) -> Option<&str> {
        self.static_field_access_names.get(name).map(String::as_str)
    }

    pub fn static_field_access_owner(&self, name: &str) -> Option<&str> {
        self.static_field_owners.get(name).map(String::as_str)
    }

    pub fn static_storage_field_access_name(&self, name: &str) -> Option<&str> {
        self.static_storage_field_access_names
            .get(name)
            .map(String::as_str)
    }

    pub fn static_storage_field_semantic_name(&self, name: &str) -> Option<&str> {
        self.static_storage_field_names
            .get(name)
            .map(String::as_str)
    }

    pub fn type_static_storage_field_access_name(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> Option<&str> {
        let alias = crate::parser::to_snake_case(field_name);
        [
            (type_name.to_string(), field_name.to_string()),
            (type_name.to_string(), alias.clone()),
            (
                crate::parser::to_snake_case(type_name),
                field_name.to_string(),
            ),
            (crate::parser::to_snake_case(type_name), alias),
        ]
        .into_iter()
        .find_map(|key| {
            self.type_static_storage_field_access_names
                .get(&key)
                .map(String::as_str)
        })
    }
}

fn is_this_expr(expr: &Expr) -> bool {
    matches!(expr, Expr::This) || matches!(expr, Expr::Var(name) if name == "this")
}

fn java_type_is_erased_object_boundary(ty: &JavaType) -> bool {
    matches!(ty, JavaType::Named { name, .. } | JavaType::Unknown(name)
        if name == "Object" || name == "java.lang.Object")
}

fn java_type_erased_occurrence_key(ty: &JavaType) -> String {
    match ty {
        JavaType::Void => "void".to_string(),
        JavaType::Primitive(primitive) => format!("primitive:{primitive:?}"),
        JavaType::String => "String".to_string(),
        JavaType::Array(inner) => format!("array:{}", java_type_erased_occurrence_key(inner)),
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            format!(
                "named:{}",
                crate::stdlib::canonical_external_receiver_type_key(name)
            )
        }
        JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(bound))) => {
            format!("? extends {}", java_type_erased_occurrence_key(bound))
        }
        JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(bound))) => {
            format!("? super {}", java_type_erased_occurrence_key(bound))
        }
        JavaType::Wildcard(None) => "?".to_string(),
    }
}

fn java_types_have_same_erased_occurrence_identity(
    plan_ty: &JavaType,
    declared_ty: &JavaType,
) -> bool {
    plan_ty == declared_ty
        || java_type_erased_occurrence_key(plan_ty) == java_type_erased_occurrence_key(declared_ty)
}

fn local_plan_matches_declared_type(local: &LocalPlan, declared_ty: &JavaType) -> bool {
    java_types_have_same_erased_occurrence_identity(&local.java_ty, declared_ty)
        || java_types_have_same_erased_occurrence_identity(&local.shape.java_ty, declared_ty)
}

fn unique_local_plan_for_declared_type<'a>(
    locals: &'a [LocalPlan],
    name: &str,
    declared_ty: &JavaType,
) -> Option<&'a LocalPlan> {
    let exact_matches = locals
        .iter()
        .filter(|local| local.name == name && local_plan_matches_declared_type(local, declared_ty))
        .collect::<Vec<_>>();
    if exact_matches.is_empty() {
        return None;
    }
    if java_type_is_erased_object_boundary(declared_ty)
        && let [local] = exact_matches
            .iter()
            .filter(|local| shape_is_erased_object_boundary(&local.shape))
            .copied()
            .collect::<Vec<_>>()
            .as_slice()
    {
        return Some(local);
    }
    match exact_matches.as_slice() {
        [local] => Some(*local),
        _ => None,
    }
}

pub fn fallback_local_shape_from_declared_type(ty: &JavaType) -> RustSlotShape {
    RustSlotShape::conservative(ty.clone())
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ExprShapeContext<'a> {
    pub signatures: Option<&'a SignaturePlan>,
    pub representations: Option<&'a RepresentationPlan>,
    pub facts: Option<&'a FactSet>,
}

pub fn infer_expr_shape(expr: &Expr, env: &ShapeEnv) -> RustSlotShape {
    infer_expr_shape_with_context(expr, env, ExprShapeContext::default())
}

pub fn infer_assignment_target_shape_with_context(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    match expr {
        Expr::Var(name) => env
            .local_shape(name)
            .cloned()
            .or_else(|| env.field_shape(name).cloned()),
        Expr::FieldAccess(receiver, field) if is_this_expr(receiver) => {
            env.field_shape(field).cloned()
        }
        Expr::FieldAccess(receiver, field) => infer_field_access_shape(receiver, field, env, ctx),
        Expr::ArrayAccess(array, _) if matches!(array.as_ref(), Expr::ArrayAccess(_, _)) => {
            infer_indexed_assignment_target_shape(array, env, ctx)
        }
        Expr::ArrayAccess(array, _) => infer_indexed_assignment_target_shape(array, env, ctx),
        _ => None,
    }
}

pub fn infer_expr_shape_with_context(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> RustSlotShape {
    match expr {
        Expr::Literal(lit) => RustSlotShape::conservative(crate::types::literal_type(lit)),
        Expr::Var(name) => env
            .local_shape(name)
            .cloned()
            .or_else(|| env.field_shape(name).cloned())
            .unwrap_or_else(|| RustSlotShape::conservative(JavaType::Unknown(name.clone()))),
        Expr::FieldAccess(receiver, field) if is_this_expr(receiver) => env
            .field_shape(field)
            .cloned()
            .unwrap_or_else(|| RustSlotShape::conservative(infer_expr_type(expr, env.type_env()))),
        Expr::FieldAccess(receiver, field) => infer_field_access_shape(receiver, field, env, ctx)
            .or_else(|| planned_member_access_shape(receiver, field, env, ctx))
            .unwrap_or_else(|| RustSlotShape::conservative(infer_expr_type(expr, env.type_env()))),
        Expr::ArrayAccess(array, _) => infer_indexed_expr_shape(array, env, ctx),
        Expr::Call {
            receiver,
            name,
            args,
            resolved: Some(call),
        } => {
            native_java_call_expr_shape(receiver.as_deref(), name, args.len(), Some(call), env, ctx)
                .or_else(|| {
                    external_call_expr_shape(receiver.as_deref(), name, args.len(), env, ctx)
                })
                .or_else(|| borrowed_string_view_call_shape(receiver.as_deref(), name, args, env))
                .or_else(|| {
                    java_string_runtime_call_expr_shape(receiver.as_deref(), name, args.len(), ctx)
                })
                .or_else(|| {
                    ctx.signatures.and_then(|signatures| {
                        resolved_call_return_shape_by_java_key(call, name, args.len(), signatures)
                            .or_else(|| {
                                signatures.method_return_shape_by_rust_name(
                                    &call.declaring_type,
                                    &call.rust_name,
                                )
                            })
                    })
                })
                .unwrap_or_else(|| {
                    value_shape_or_conservative(ctx, infer_expr_type(expr, env.type_env()))
                })
        }
        Expr::Call {
            receiver,
            name,
            args,
            resolved: None,
        } => native_java_call_expr_shape(receiver.as_deref(), name, args.len(), None, env, ctx)
            .or_else(|| external_call_expr_shape(receiver.as_deref(), name, args.len(), env, ctx))
            .or_else(|| borrowed_string_view_call_shape(receiver.as_deref(), name, args, env))
            .or_else(|| {
                java_string_runtime_call_expr_shape(receiver.as_deref(), name, args.len(), ctx)
            })
            .unwrap_or_else(|| {
                value_shape_or_conservative(ctx, infer_expr_type(expr, env.type_env()))
            }),
        Expr::New {
            ty,
            resolved: Some(resolved),
            ..
        } => {
            let java_ty = if matches!(ty, JavaType::Unknown(_)) {
                JavaType::Named {
                    name: resolved.declaring_type.clone(),
                    args: Vec::new(),
                }
            } else {
                ty.clone()
            };
            value_shape_or_conservative(ctx, java_ty)
        }
        Expr::Cast { ty, .. } => value_shape_or_conservative(ctx, ty.clone()),
        _ => value_shape_or_conservative(ctx, infer_expr_type(expr, env.type_env())),
    }
}

fn resolved_call_return_shape_by_java_key(
    call: &crate::java_ir::ResolvedCall,
    source_name: &str,
    arg_count: usize,
    signatures: &SignaturePlan,
) -> Option<RustSlotShape> {
    if call.arg_target_types.len() != arg_count {
        return None;
    }
    signatures.method_return_shape_by_java_key(&JavaCallableKey::resolved_source_method(
        JavaOwnerKey::resolved_source(&call.declaring_type),
        source_name,
        &call.arg_target_types,
        call.kind,
    ))
}

fn native_java_call_expr_shape(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    resolved: Option<&crate::java_ir::ResolvedCall>,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    let default_facts;
    let facts = match ctx.facts {
        Some(facts) => facts,
        None => {
            default_facts = FactSet::default();
            &default_facts
        }
    };
    let target = crate::call_plan::plan_call_target_with_facts_and_arity(
        receiver,
        name,
        arg_count,
        resolved,
        facts,
        env.type_env(),
    );
    if let crate::call_plan::CallTargetPlan::NativeCollectionMethod {
        receiver, method, ..
    } = &target
        && let Some(lowered) =
            crate::call_plan::native_collection_lowered_type(receiver, *method, env.type_env())
    {
        return Some(lowered.into_slot_shape());
    }
    crate::call_plan::native_java_call_lowered_type(&target)
        .map(|lowered| lowered.into_slot_shape())
}

fn external_call_expr_shape(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    let facts = ctx.facts?;
    crate::call_plan::external_call_lowered_type(
        receiver,
        name,
        arg_count,
        facts,
        env.type_env(),
        None,
    )
    .map(|lowered| lowered.into_slot_shape())
}

fn java_string_runtime_call_expr_shape(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    receiver?;
    crate::stdlib::java_string_runtime_method_return_type(name, arg_count)
        .map(|java_ty| value_shape_or_conservative(ctx, java_ty))
}

fn borrowed_string_view_call_shape(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    env: &ShapeEnv,
) -> Option<RustSlotShape> {
    if !crate::stdlib::is_borrowed_string_view_method(name, args.len()) {
        return None;
    }
    let receiver = receiver?;
    let receiver_shape = infer_expr_shape(receiver, env);
    let receiver_ty = if matches!(receiver_shape.java_ty, JavaType::String) {
        receiver_shape.java_ty
    } else {
        infer_expr_type(receiver, env.type_env())
    };
    let effective_receiver_ty =
        crate::types::optional_element_type(&receiver_ty).unwrap_or(receiver_ty);
    if !matches!(effective_receiver_ty, JavaType::String) {
        return None;
    }
    Some(crate::shape_projection::borrowed_string_content_shape())
}

pub fn shape_env_for_type(ty: &TypeDecl, signatures: &SignaturePlan) -> ShapeEnv {
    shape_env_for_type_with_fields(ty, signatures, true)
}

fn shape_env_for_type_with_fields(
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    include_instance_fields: bool,
) -> ShapeEnv {
    let mut env = ShapeEnv::new();
    env.set_current_type(&ty.name);
    seed_type_param_bounds(&mut env, &ty.type_params, &ty.type_param_bounds);
    let source_instance_field_rust_names = crate::java_naming::source_instance_field_rust_names(ty);
    let source_static_field_rust_names = crate::java_naming::source_static_field_rust_names(ty);
    for member in &ty.members {
        if let Member::Field(field) = member {
            let shape = signatures.required_field_shape(&ty.name, &field.name);
            let instance_access_name = source_instance_field_rust_names
                .get(&field.name)
                .cloned()
                .unwrap_or_else(|| field.name.clone());
            let static_access_name = source_static_field_rust_names
                .get(&field.name)
                .cloned()
                .unwrap_or_else(|| field.name.clone());
            if crate::item_assembly::static_final_field_has_associated_const(ty, field) {
                env.insert_type_field_shape_with_java_type(
                    &ty.name,
                    field.name.clone(),
                    shape.clone(),
                    field.ty.clone(),
                );
                env.insert_static_field_shape_with_access_name_and_java_type(
                    field.name.clone(),
                    static_access_name,
                    shape,
                    field.ty.clone(),
                );
            } else if field.modifiers.contains(&Modifier::Static) {
                env.insert_type_static_storage_field_shape_with_access_name_and_java_type(
                    &ty.name,
                    field.name.clone(),
                    static_access_name.clone(),
                    shape.clone(),
                    field.ty.clone(),
                );
                env.insert_static_storage_field_shape_with_access_name_and_java_type(
                    field.name.clone(),
                    static_access_name,
                    shape,
                    field.ty.clone(),
                );
            } else if include_instance_fields {
                env.insert_type_field_shape_with_java_type(
                    &ty.name,
                    field.name.clone(),
                    shape.clone(),
                    field.ty.clone(),
                );
                env.insert_field_shape_with_access_name_and_java_type(
                    field.name.clone(),
                    instance_access_name,
                    shape,
                    field.ty.clone(),
                );
            } else {
                env.insert_type_field_shape_with_java_type(
                    &ty.name,
                    field.name.clone(),
                    shape,
                    field.ty.clone(),
                );
            }
        }
    }
    env
}

pub fn shape_env_for_constructor(
    ty: &TypeDecl,
    ctor: &crate::java_ir::ConstructorDecl,
    constructor_signature: &ConstructorSignaturePlan,
    signatures: &SignaturePlan,
) -> ShapeEnv {
    let mut env = shape_env_for_type(ty, signatures);
    insert_param_shapes(&mut env, &ctor.params, &constructor_signature.params);
    env
}

pub fn shape_env_for_method(
    ty: &TypeDecl,
    method: &MethodDecl,
    method_signature: &MethodSignaturePlan,
    signatures: &SignaturePlan,
) -> ShapeEnv {
    let mut env = if method.modifiers.contains(&Modifier::Static) {
        shape_env_for_type_with_fields(ty, signatures, false)
    } else {
        shape_env_for_type(ty, signatures)
    };
    seed_type_param_bounds(&mut env, &method.type_params, &method.type_param_bounds);
    insert_param_shapes(&mut env, &method.params, &method_signature.params);
    env
}

fn seed_type_param_bounds(
    env: &mut ShapeEnv,
    type_params: &[String],
    type_param_bounds: &[Option<JavaType>],
) {
    for (name, bound) in type_params.iter().zip(type_param_bounds.iter()) {
        if let Some(bound) = bound {
            env.insert_type_param_bound(name.clone(), bound.clone());
        }
    }
}

pub fn learn_stmt_shapes(
    stmt: &Stmt,
    env: &mut ShapeEnv,
    locals: &[LocalPlan],
    signatures: &SignaturePlan,
) {
    match stmt {
        Stmt::VarDecl {
            ty,
            name,
            source_name,
            initializer,
            binding,
        } => {
            let local_plan =
                local_plan_for_declaration_shape(signatures, locals, binding.as_ref(), name, ty);
            let shape = local_plan
                .map(|plan| plan.shape.clone())
                .unwrap_or_else(|| fallback_local_shape_from_declared_type(ty));
            let env_binding = binding
                .clone()
                .or_else(|| local_plan.map(|plan| plan.binding.clone()));
            env.insert_local_shape_with_java_type_and_binding(
                name.clone(),
                shape,
                ty.clone(),
                env_binding,
            );
            if let Some(receiver) = map_key_set_view_initializer_receiver(initializer.as_ref()) {
                env.insert_map_key_set_view_receiver(name.clone(), receiver.clone());
                if source_name != name {
                    env.insert_map_key_set_view_receiver(source_name.clone(), receiver.clone());
                }
            }
            if let Some(receiver) = map_values_view_initializer_receiver(initializer.as_ref()) {
                env.insert_map_values_view_receiver(name.clone(), receiver.clone());
                if source_name != name {
                    env.insert_map_values_view_receiver(source_name.clone(), receiver.clone());
                }
            }
        }
        Stmt::For { init, .. } => {
            for stmt in init {
                learn_stmt_shapes(stmt, env, locals, signatures);
            }
        }
        Stmt::EnhancedFor {
            var_type,
            var_name,
            binding,
            ..
        } => {
            let local_plan = local_plan_for_declaration_shape(
                signatures,
                locals,
                binding.as_ref(),
                var_name,
                var_type,
            );
            let shape = local_plan
                .map(|plan| plan.shape.clone())
                .unwrap_or_else(|| fallback_local_shape_from_declared_type(var_type));
            let env_binding = binding
                .clone()
                .or_else(|| local_plan.map(|plan| plan.binding.clone()));
            env.insert_loop_variable_shape_with_access_name_and_binding(
                var_name.clone(),
                var_name.clone(),
                shape,
                env_binding,
            );
        }
        Stmt::Try {
            resources, body, ..
        } => {
            for resource in resources {
                learn_stmt_shapes(resource, env, locals, signatures);
            }
            for stmt in &body.statements {
                learn_stmt_shapes(stmt, env, locals, signatures);
            }
        }
        _ => {}
    }
}

fn map_key_set_view_initializer_receiver(initializer: Option<&Expr>) -> Option<&Expr> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = initializer?
    else {
        return None;
    };
    (args.is_empty() && matches!(name.as_str(), "keySet" | "key_set")).then_some(receiver.as_ref())
}

fn map_values_view_initializer_receiver(initializer: Option<&Expr>) -> Option<&Expr> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = initializer?
    else {
        return None;
    };
    (args.is_empty() && name == "values").then_some(receiver.as_ref())
}

fn local_plan_for_declaration_shape<'a>(
    signatures: &'a SignaturePlan,
    locals: &'a [LocalPlan],
    binding: Option<&crate::binding_id::BindingId>,
    name: &str,
    declared_ty: &JavaType,
) -> Option<&'a LocalPlan> {
    match binding {
        Some(binding) => signatures.local_plan_by_binding(locals, binding),
        None => unique_local_plan_for_declared_type(locals, name, declared_ty),
    }
}

#[cfg(test)]
fn local_shape_for_declared_type<'a>(
    locals: &'a [LocalPlan],
    name: &str,
    declared_ty: &JavaType,
) -> Option<&'a RustSlotShape> {
    unique_local_plan_for_declared_type(locals, name, declared_ty).map(|local| &local.shape)
}

fn insert_param_shapes(env: &mut ShapeEnv, source_params: &[Param], params: &[ParamPlan]) {
    let access_names = crate::java_naming::source_binding_rust_names(
        source_params.iter().map(|param| param.source_name.as_str()),
    );
    for (source_param, param) in source_params.iter().zip(params.iter()) {
        let access_name = access_names
            .get(&source_param.source_name)
            .cloned()
            .unwrap_or_else(|| param.name.clone());
        env.insert_param_shape_with_access_name_java_type_and_binding(
            source_param.source_name.clone(),
            access_name.clone(),
            param.shape.clone(),
            param.java_ty.clone(),
            source_param.binding.clone(),
        );
        if param.name != source_param.source_name {
            env.insert_param_shape_with_access_name_java_type_and_binding(
                param.name.clone(),
                access_name,
                param.shape.clone(),
                param.java_ty.clone(),
                source_param.binding.clone(),
            );
        }
    }
    for param in params.iter().skip(source_params.len()) {
        env.insert_param_shape_with_java_type(
            param.name.clone(),
            param.shape.clone(),
            param.java_ty.clone(),
        );
    }
}

fn value_shape_or_conservative(ctx: ExprShapeContext<'_>, java_ty: JavaType) -> RustSlotShape {
    project_java_value_shape_with_representations(ctx.representations, java_ty)
}

fn infer_field_access_shape(
    receiver: &Expr,
    field: &str,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    if let Some(shape) = source_java_enum_static_field_shape(receiver, field, env, ctx) {
        return Some(shape);
    }
    if let Some(shape) = static_storage_field_access_shape(receiver, field, env, ctx) {
        return Some(shape);
    }
    if let Some(signatures) = ctx.signatures
        && let Some(crate::member_access_plan::MemberAccessPlan {
            op: crate::member_access_plan::MemberAccessOp::StaticConstant { owner },
            ..
        }) = crate::member_access_plan::plan_member_access_with_facts(
            receiver,
            field,
            env.type_env(),
            ctx.facts.unwrap_or(&FactSet::default()),
        )
    {
        if let Some(shape) = signatures
            .field_shape(&owner, field)
            .or_else(|| signatures.field_shape(&owner, &crate::parser::to_snake_case(field)))
        {
            return Some(field_access_shape_from_signature_shape(shape));
        }
    }
    let receiver_ty = infer_expr_type(receiver, env.type_env());
    let name = match receiver_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        _ => return None,
    };
    if env.local_type(&name).is_some() || env.field_type(&name).is_some() {
        return None;
    }
    ctx.signatures.and_then(|signatures| {
        signatures
            .field_shape(&name, field)
            .or_else(|| signatures.field_shape(&name, &crate::parser::to_snake_case(field)))
            .map(field_access_shape_from_signature_shape)
    })
}

fn static_storage_field_access_shape(
    receiver: &Expr,
    field: &str,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    let Expr::Var(receiver_name) = receiver else {
        return None;
    };
    if env.local_type(receiver_name).is_some() || env.field_type(receiver_name).is_some() {
        return None;
    }
    let signatures = ctx.signatures?;
    source_java_enum_static_field_owner_candidates(receiver_name, env)
        .into_iter()
        .find_map(|owner| {
            env.type_static_storage_field_access_name(&owner, field)?;
            signatures
                .field_shape(&owner, field)
                .or_else(|| signatures.field_shape(&owner, &crate::parser::to_snake_case(field)))
                .map(field_access_shape_from_signature_shape)
        })
}

fn source_java_enum_static_field_shape(
    receiver: &Expr,
    field: &str,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    let Expr::Var(receiver_name) = receiver else {
        return None;
    };
    if env.local_type(receiver_name).is_some() || env.field_type(receiver_name).is_some() {
        return None;
    }
    let representations = ctx.representations?;
    source_java_enum_static_field_owner_candidates(receiver_name, env)
        .into_iter()
        .find_map(|owner| {
            representations
                .java_enum_variant_path_for_label(&owner, field)
                .map(|_| {
                    project_java_value_shape_with_representations(
                        Some(representations),
                        JavaType::Named {
                            name: owner,
                            args: Vec::new(),
                        },
                    )
                })
        })
}

fn source_java_enum_static_field_owner_candidates(
    receiver_name: &str,
    env: &ShapeEnv,
) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(alias) = env.type_env().static_type_alias(receiver_name) {
        push_unique_type_name(&mut candidates, alias.to_string());
    }
    push_unique_type_name(&mut candidates, receiver_name.to_string());
    push_unique_type_name(
        &mut candidates,
        upper_camel_type_name_from_static_receiver(receiver_name),
    );
    candidates
}

fn push_unique_type_name(out: &mut Vec<String>, candidate: String) {
    if !candidate.is_empty() && !out.iter().any(|existing| existing == &candidate) {
        out.push(candidate);
    }
}

fn upper_camel_type_name_from_static_receiver(receiver_name: &str) -> String {
    receiver_name
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<String>()
}

fn field_access_shape_from_signature_shape(shape: &RustSlotShape) -> RustSlotShape {
    if matches!(shape.java_ty, JavaType::String) {
        borrowed_string_content_shape()
    } else {
        shape.clone()
    }
}

fn planned_member_access_shape(
    receiver: &Expr,
    field: &str,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    crate::member_access_plan::plan_member_access_with_facts(
        receiver,
        field,
        env.type_env(),
        ctx.facts.unwrap_or(&FactSet::default()),
    )
    .map(|plan| plan.shape)
}

fn infer_indexed_expr_shape(
    array: &Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> RustSlotShape {
    let array_shape = infer_expr_shape_with_context(array, env, ctx);
    indexed_read_shape_for_access_with_facts(&array_shape, ctx.facts.unwrap_or(&FactSet::default()))
        .unwrap_or_else(|| {
            crate::shape_projection::indexed_read_shape_with_facts(
                &array_shape,
                ctx.facts.unwrap_or(&FactSet::default()),
            )
        })
}

fn infer_indexed_assignment_target_shape(
    array: &Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    indexed_assignment_shape_with_facts(
        &infer_expr_shape_with_context(array, env, ctx),
        ctx.facts.unwrap_or(&FactSet::default()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java_ir::PrimitiveType;
    use crate::slot_shape::{DispatchShape, OwnershipShape};

    fn int_plan_with_ownership(ownership: OwnershipShape) -> LocalPlan {
        let java_ty = JavaType::Primitive(PrimitiveType::Int);
        let mut shape = RustSlotShape::conservative(java_ty.clone());
        shape.ownership = ownership;
        LocalPlan {
            binding: crate::binding_id::BindingId::compatibility_local("value"),
            name: "value".to_string(),
            java_ty,
            shape,
            binding_mutable: false,
        }
    }

    #[test]
    fn unique_idless_local_plan_match_is_still_accepted() {
        let plans = vec![int_plan_with_ownership(OwnershipShape::Owned)];
        let mut env = ShapeEnv::new();
        let java_ty = JavaType::Primitive(PrimitiveType::Int);

        assert_eq!(
            env.next_local_plan_for_declared_type(&plans, "value", &java_ty)
                .map(|plan| plan.shape.ownership),
            Some(OwnershipShape::Owned)
        );
    }

    #[test]
    fn ambiguous_same_name_same_type_idless_local_plan_fails_closed() {
        let plans = vec![
            int_plan_with_ownership(OwnershipShape::Owned),
            int_plan_with_ownership(OwnershipShape::SharedBorrow),
        ];
        let mut env = ShapeEnv::new();
        let java_ty = JavaType::Primitive(PrimitiveType::Int);

        assert!(
            env.next_local_plan_for_declared_type(&plans, "value", &java_ty)
                .is_none(),
            "id-less same-name/same-type declarations must fail closed instead of selecting by traversal order"
        );
    }

    fn erased_object_plan_with_ownership(ownership: OwnershipShape) -> LocalPlan {
        let java_ty = JavaType::Unknown("Object".to_string());
        let mut shape = RustSlotShape::conservative(java_ty.clone());
        shape.ownership = ownership;
        shape.dispatch = DispatchShape::ErasedObject;
        LocalPlan {
            binding: crate::binding_id::BindingId::compatibility_local("value"),
            name: "value".to_string(),
            java_ty,
            shape,
            binding_mutable: false,
        }
    }

    #[test]
    fn ambiguous_erased_object_idless_local_plan_lookup_fails_closed() {
        let plans = vec![
            erased_object_plan_with_ownership(OwnershipShape::Owned),
            erased_object_plan_with_ownership(OwnershipShape::SharedBorrow),
        ];
        let mut env = ShapeEnv::new();
        let declared_ty = JavaType::Unknown("Object".to_string());

        assert!(
            env.next_local_plan_for_declared_type(&plans, "value", &declared_ty)
                .is_none(),
            "ambiguous erased-object compatibility plans must fail closed"
        );
    }

    #[test]
    fn local_plan_type_mismatch_does_not_fall_back_by_name() {
        let string_ty = JavaType::String;
        let mut string_shape = RustSlotShape::conservative(string_ty.clone());
        string_shape.ownership = OwnershipShape::Owned;
        let plan = LocalPlan {
            binding: crate::binding_id::BindingId::compatibility_local("value"),
            name: "value".to_string(),
            java_ty: string_ty,
            shape: string_shape,
            binding_mutable: false,
        };
        let plans = vec![plan];
        let mut env = ShapeEnv::new();

        assert!(
            env.next_local_plan_for_declared_type(
                &plans,
                "value",
                &JavaType::Primitive(PrimitiveType::Boolean),
            )
            .is_none(),
            "id-less compatibility lookup must not reuse a same-name plan with a different declared type"
        );
    }

    #[test]
    fn local_shape_type_mismatch_does_not_fall_back_by_name() {
        let string_ty = JavaType::String;
        let mut string_shape = RustSlotShape::conservative(string_ty.clone());
        string_shape.ownership = OwnershipShape::Owned;
        let plans = vec![LocalPlan {
            binding: crate::binding_id::BindingId::compatibility_local("value"),
            name: "value".to_string(),
            java_ty: string_ty,
            shape: string_shape,
            binding_mutable: false,
        }];

        assert!(
            local_shape_for_declared_type(
                &plans,
                "value",
                &JavaType::Primitive(PrimitiveType::Boolean),
            )
            .is_none(),
            "shape learning must not reuse a same-name shape with a different declared type"
        );
    }

    #[test]
    fn ambiguous_same_name_same_type_idless_local_shape_fails_closed() {
        let plans = vec![
            int_plan_with_ownership(OwnershipShape::Owned),
            int_plan_with_ownership(OwnershipShape::SharedBorrow),
        ];
        let java_ty = JavaType::Primitive(PrimitiveType::Int);

        assert!(
            local_shape_for_declared_type(&plans, "value", &java_ty).is_none(),
            "id-less shape learning must fail closed instead of selecting the first duplicate plan"
        );
    }

    #[test]
    fn local_plan_type_match_accepts_concrete_shape_over_erased_carrier() {
        let declared_ty = JavaType::Named {
            name: "FeatureReader".to_string(),
            args: vec![JavaType::String],
        };
        let mut concrete_shape = RustSlotShape::conservative(JavaType::Named {
            name: "FeatureReader".to_string(),
            args: Vec::new(),
        });
        concrete_shape.ownership = OwnershipShape::Owned;
        let plans = vec![LocalPlan {
            binding: crate::binding_id::BindingId::compatibility_local("reader"),
            name: "reader".to_string(),
            java_ty: JavaType::Unknown("Object".to_string()),
            shape: concrete_shape,
            binding_mutable: false,
        }];
        let mut env = ShapeEnv::new();

        assert!(
            env.next_local_plan_for_declared_type(&plans, "reader", &declared_ty)
                .is_some(),
            "erased carriers may still have a concrete solved shape matching the declared type"
        );
    }

    #[test]
    fn bound_var_decl_shape_learning_does_not_fall_back_by_name() {
        let java_ty = JavaType::Primitive(PrimitiveType::Int);
        let mut planned_shape = RustSlotShape::conservative(java_ty.clone());
        planned_shape.ownership = OwnershipShape::SharedBorrow;
        let plans = vec![LocalPlan {
            binding: BindingId::new(BindingKind::Local, "value".to_string(), 0),
            name: "value".to_string(),
            java_ty: java_ty.clone(),
            shape: planned_shape,
            binding_mutable: false,
        }];
        let stmt = Stmt::VarDecl {
            ty: java_ty.clone(),
            name: "value".to_string(),
            source_name: "value".to_string(),
            initializer: None,
            binding: Some(BindingId::new(BindingKind::Local, "value".to_string(), 1)),
        };
        let mut env = ShapeEnv::new();

        learn_stmt_shapes(&stmt, &mut env, &plans, &SignaturePlan::default());

        assert_ne!(
            env.local_shape("value").map(|shape| shape.ownership),
            Some(OwnershipShape::SharedBorrow),
            "bound declarations must fail closed instead of borrowing a same-name local's shape"
        );
    }

    #[test]
    fn bound_enhanced_for_shape_learning_does_not_fall_back_by_name() {
        let java_ty = JavaType::Primitive(PrimitiveType::Int);
        let mut planned_shape = RustSlotShape::conservative(java_ty.clone());
        planned_shape.ownership = OwnershipShape::SharedBorrow;
        let plans = vec![LocalPlan {
            binding: BindingId::new(BindingKind::Local, "item".to_string(), 0),
            name: "item".to_string(),
            java_ty: java_ty.clone(),
            shape: planned_shape,
            binding_mutable: false,
        }];
        let stmt = Stmt::EnhancedFor {
            var_type: java_ty.clone(),
            var_name: "item".to_string(),
            source_var_name: "item".to_string(),
            iterable: Expr::Var("items".to_string()),
            body: crate::java_ir::Block::default(),
            label: None,
            binding: Some(BindingId::new(BindingKind::Local, "item".to_string(), 1)),
        };
        let mut env = ShapeEnv::new();

        learn_stmt_shapes(&stmt, &mut env, &plans, &SignaturePlan::default());

        assert_ne!(
            env.local_shape("item").map(|shape| shape.ownership),
            Some(OwnershipShape::SharedBorrow),
            "bound enhanced-for variables must not inherit a same-name local's shape"
        );
    }

    #[test]
    fn idless_var_decl_shape_learning_preserves_selected_compatibility_binding() {
        let java_ty = JavaType::String;
        let selected_binding = BindingId::new(BindingKind::Local, "value".to_string(), 7);
        let plans = vec![LocalPlan {
            binding: selected_binding.clone(),
            name: "value".to_string(),
            java_ty: java_ty.clone(),
            shape: RustSlotShape::for_storage(java_ty.clone()),
            binding_mutable: false,
        }];
        let stmt = Stmt::VarDecl {
            ty: java_ty,
            name: "value".to_string(),
            source_name: "value".to_string(),
            initializer: Some(Expr::Literal(crate::java_ir::Literal::String(
                "x".to_string(),
            ))),
            binding: None,
        };
        let mut env = ShapeEnv::new();

        learn_stmt_shapes(&stmt, &mut env, &plans, &SignaturePlan::default());

        assert_eq!(env.binding_id("value"), Some(&selected_binding));
    }

    #[test]
    fn idless_enhanced_for_shape_learning_preserves_selected_compatibility_binding() {
        let java_ty = JavaType::String;
        let selected_binding = BindingId::new(BindingKind::LoopVariable, "item".to_string(), 4);
        let plans = vec![LocalPlan {
            binding: selected_binding.clone(),
            name: "item".to_string(),
            java_ty: java_ty.clone(),
            shape: RustSlotShape::for_storage(java_ty.clone()),
            binding_mutable: false,
        }];
        let stmt = Stmt::EnhancedFor {
            var_type: java_ty,
            var_name: "item".to_string(),
            source_var_name: "item".to_string(),
            iterable: Expr::Var("items".to_string()),
            body: crate::java_ir::Block::default(),
            label: None,
            binding: None,
        };
        let mut env = ShapeEnv::new();

        learn_stmt_shapes(&stmt, &mut env, &plans, &SignaturePlan::default());

        assert_eq!(env.binding_id("item"), Some(&selected_binding));
    }

    #[test]
    fn bound_local_env_insert_preserves_resolver_binding_id() {
        let mut env = ShapeEnv::new();
        let binding = BindingId::new(BindingKind::Local, "value".to_string(), 1);
        env.insert_local_shape_with_java_type_and_binding(
            "value",
            RustSlotShape::for_storage(JavaType::String),
            JavaType::String,
            Some(binding.clone()),
        );

        assert_eq!(env.binding_id("value"), Some(&binding));
    }

    #[test]
    fn bound_param_env_insert_preserves_resolver_binding_id() {
        let mut env = ShapeEnv::new();
        let binding = BindingId::new(BindingKind::Param, "value".to_string(), 1);
        env.insert_param_shape_with_access_name_java_type_and_binding(
            "value",
            "value",
            RustSlotShape::for_storage(JavaType::String),
            JavaType::String,
            Some(binding.clone()),
        );

        assert_eq!(env.binding_id("value"), Some(&binding));
    }

    #[test]
    fn bound_loop_variable_env_insert_preserves_resolver_binding_id() {
        let mut env = ShapeEnv::new();
        let binding = BindingId::new(BindingKind::LoopVariable, "item".to_string(), 1);
        env.insert_loop_variable_shape_with_access_name_and_binding(
            "item",
            "item",
            RustSlotShape::for_storage(JavaType::String),
            Some(binding.clone()),
        );

        assert_eq!(env.binding_id("item"), Some(&binding));
    }

    #[test]
    fn bound_catch_param_env_insert_preserves_resolver_binding_id() {
        let mut env = ShapeEnv::new();
        let binding = BindingId::new(BindingKind::CatchParam, "ex".to_string(), 1);
        env.insert_catch_param_with_binding(
            "ex",
            JavaType::Named {
                name: "JavaException".to_string(),
                args: Vec::new(),
            },
            Some(binding.clone()),
        );

        assert_eq!(env.binding_id("ex"), Some(&binding));
    }
}
