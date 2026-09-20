use std::collections::{HashMap, HashSet};

use crate::binding_id::{BindingId, BindingKind};
use crate::facts::FactSet;
use crate::java_ir::{Expr, JavaType, Member, MethodDecl, Modifier, Param, Stmt, TypeDecl};
use crate::representation::RepresentationPlan;
use crate::shape_projection::{
    borrowed_string_content_shape, indexed_assignment_shape_with_facts,
    indexed_read_shape_for_access_with_facts,
    project_java_value_shape as project_java_value_shape_with_representations,
};
use crate::signature::{
    ConstructorSignaturePlan, LocalPlan, MethodSignaturePlan, ParamPlan, SignaturePlan,
};
use crate::slot_shape::RustSlotShape;
use crate::types::{TypeEnv, infer_expr_type};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImmutableCollectionViewMutationProfile {
    Empty,
    NonEmpty,
}

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
    map_key_set_view_receivers_by_binding: HashMap<BindingId, Expr>,
    map_values_view_receivers: HashMap<String, Expr>,
    map_values_view_receivers_by_binding: HashMap<BindingId, Expr>,
    map_entry_set_view_receivers: HashMap<String, Expr>,
    map_entry_set_view_receivers_by_binding: HashMap<BindingId, Expr>,
    map_entry_value_receivers: HashMap<String, Expr>,
    map_entry_value_receivers_by_binding: HashMap<BindingId, Expr>,
    unmodifiable_collection_view_sources: HashMap<String, Expr>,
    unmodifiable_collection_view_sources_by_binding: HashMap<BindingId, Expr>,
    immutable_collection_view_locals: HashMap<String, ImmutableCollectionViewMutationProfile>,
    immutable_collection_view_locals_by_binding:
        HashMap<BindingId, ImmutableCollectionViewMutationProfile>,
    fixed_size_list_view_args: HashMap<String, Vec<Expr>>,
    fixed_size_list_view_args_by_binding: HashMap<BindingId, Vec<Expr>>,
    simple_map_entry_values: HashMap<String, (Expr, Expr)>,
    simple_map_entry_values_by_binding: HashMap<BindingId, (Expr, Expr)>,
    simple_immutable_map_entry_locals: HashSet<String>,
    simple_immutable_map_entry_locals_by_binding: HashSet<BindingId>,
    nested_array2_locals: HashSet<String>,
    nested_array2_locals_by_binding: HashSet<BindingId>,
}

impl ShapeEnv {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_env(&self) -> &TypeEnv {
        &self.types
    }

    pub(crate) fn insert_map_key_set_view_receiver(
        &mut self,
        name: impl Into<String>,
        receiver: Expr,
    ) {
        let name = name.into();
        if let Some(binding) = self.binding_ids.get(&name).cloned() {
            self.map_key_set_view_receivers_by_binding
                .insert(binding, receiver);
        } else {
            self.map_key_set_view_receivers.insert(name, receiver);
        }
    }

    pub(crate) fn insert_map_values_view_receiver(
        &mut self,
        name: impl Into<String>,
        receiver: Expr,
    ) {
        let name = name.into();
        if let Some(binding) = self.binding_ids.get(&name).cloned() {
            self.map_values_view_receivers_by_binding
                .insert(binding, receiver);
        } else {
            self.map_values_view_receivers.insert(name, receiver);
        }
    }

    pub(crate) fn insert_map_entry_set_view_receiver(
        &mut self,
        name: impl Into<String>,
        receiver: Expr,
    ) {
        let name = name.into();
        if let Some(binding) = self.binding_ids.get(&name).cloned() {
            self.map_entry_set_view_receivers_by_binding
                .insert(binding, receiver);
        } else {
            self.map_entry_set_view_receivers.insert(name, receiver);
        }
    }

    pub(crate) fn insert_map_entry_value_receiver(
        &mut self,
        name: impl Into<String>,
        receiver: Expr,
    ) {
        let name = name.into();
        if let Some(binding) = self.binding_ids.get(&name).cloned() {
            self.map_entry_value_receivers_by_binding
                .insert(binding, receiver);
        } else {
            self.map_entry_value_receivers.insert(name, receiver);
        }
    }

    pub(crate) fn insert_unmodifiable_collection_view_source(
        &mut self,
        name: impl Into<String>,
        source: Expr,
    ) {
        let name = name.into();
        if let Some(binding) = self.binding_ids.get(&name).cloned() {
            self.unmodifiable_collection_view_sources_by_binding
                .insert(binding, source);
        } else {
            self.unmodifiable_collection_view_sources
                .insert(name, source);
        }
    }

    pub(crate) fn insert_fixed_size_list_view_args(
        &mut self,
        name: impl Into<String>,
        args: Vec<Expr>,
    ) {
        let name = name.into();
        if let Some(binding) = self.binding_ids.get(&name).cloned() {
            self.fixed_size_list_view_args_by_binding
                .insert(binding, args);
        } else {
            self.fixed_size_list_view_args.insert(name, args);
        }
    }

    pub(crate) fn insert_immutable_collection_view_local(
        &mut self,
        name: impl Into<String>,
        profile: ImmutableCollectionViewMutationProfile,
    ) {
        let name = name.into();
        if let Some(binding) = self.binding_ids.get(&name).cloned() {
            self.immutable_collection_view_locals_by_binding
                .insert(binding, profile);
        }
        self.immutable_collection_view_locals.insert(name, profile);
    }

    pub(crate) fn insert_simple_map_entry_value(
        &mut self,
        name: impl Into<String>,
        key: Expr,
        value: Expr,
    ) {
        let name = name.into();
        if let Some(binding) = self.binding_ids.get(&name).cloned() {
            self.simple_map_entry_values_by_binding
                .insert(binding, (key, value));
        } else {
            self.simple_map_entry_values.insert(name, (key, value));
        }
    }

    pub(crate) fn forget_simple_map_entry_value(&mut self, name: &str) {
        if let Some(binding) = self.binding_ids.get(name) {
            self.simple_map_entry_values_by_binding.remove(binding);
            self.simple_immutable_map_entry_locals_by_binding
                .remove(binding);
        }
        self.simple_map_entry_values.remove(name);
        self.simple_immutable_map_entry_locals.remove(name);
    }

    pub(crate) fn insert_simple_immutable_map_entry_local(&mut self, name: impl Into<String>) {
        let name = name.into();
        if let Some(binding) = self.binding_ids.get(&name).cloned() {
            self.simple_immutable_map_entry_locals_by_binding
                .insert(binding);
        } else {
            self.simple_immutable_map_entry_locals.insert(name);
        }
    }

    pub(crate) fn simple_map_entry_local_is_immutable(&self, name: &str) -> bool {
        if let Some(binding) = self.binding_ids.get(name) {
            return self
                .simple_immutable_map_entry_locals_by_binding
                .contains(binding);
        }
        self.simple_immutable_map_entry_locals.contains(name)
    }

    pub(crate) fn forget_map_entry_value_receiver(&mut self, name: &str) {
        if let Some(binding) = self.binding_ids.get(name) {
            self.map_entry_value_receivers_by_binding.remove(binding);
        }
        self.map_entry_value_receivers.remove(name);
    }

    pub(crate) fn insert_nested_array2_local(&mut self, name: impl Into<String>) {
        let name = name.into();
        if let Some(binding) = self.binding_ids.get(&name).cloned() {
            self.nested_array2_locals_by_binding.insert(binding);
        } else {
            self.nested_array2_locals.insert(name);
        }
    }

    pub(crate) fn local_uses_nested_array2_slice(&self, name: &str) -> bool {
        if let Some(binding) = self.binding_ids.get(name) {
            return self.nested_array2_locals_by_binding.contains(binding);
        }
        self.nested_array2_locals.contains(name)
    }

    fn clear_local_side_facts(&mut self, name: &str) {
        if let Some(binding) = self.binding_ids.get(name) {
            self.map_key_set_view_receivers_by_binding.remove(binding);
            self.map_values_view_receivers_by_binding.remove(binding);
            self.map_entry_set_view_receivers_by_binding.remove(binding);
            self.map_entry_value_receivers_by_binding.remove(binding);
            self.unmodifiable_collection_view_sources_by_binding
                .remove(binding);
            self.immutable_collection_view_locals_by_binding
                .remove(binding);
            self.fixed_size_list_view_args_by_binding.remove(binding);
            self.simple_map_entry_values_by_binding.remove(binding);
            self.simple_immutable_map_entry_locals_by_binding
                .remove(binding);
            self.nested_array2_locals_by_binding.remove(binding);
        }
        self.map_key_set_view_receivers.remove(name);
        self.map_values_view_receivers.remove(name);
        self.map_entry_set_view_receivers.remove(name);
        self.map_entry_value_receivers.remove(name);
        self.unmodifiable_collection_view_sources.remove(name);
        self.immutable_collection_view_locals.remove(name);
        self.fixed_size_list_view_args.remove(name);
        self.simple_map_entry_values.remove(name);
        self.simple_immutable_map_entry_locals.remove(name);
        self.nested_array2_locals.remove(name);
    }

    fn clear_local_side_facts_for_access_name(&mut self, name: &str, access_name: &str) {
        self.clear_local_side_facts(name);
        if access_name != name {
            self.clear_local_side_facts(access_name);
        }
    }

    pub(crate) fn map_key_set_view_receiver(&self, name: &str) -> Option<&Expr> {
        if let Some(binding) = self.binding_ids.get(name) {
            return self.map_key_set_view_receivers_by_binding.get(binding);
        }
        self.map_key_set_view_receivers.get(name)
    }

    pub(crate) fn map_values_view_receiver(&self, name: &str) -> Option<&Expr> {
        if let Some(binding) = self.binding_ids.get(name) {
            return self.map_values_view_receivers_by_binding.get(binding);
        }
        self.map_values_view_receivers.get(name)
    }

    pub(crate) fn map_entry_set_view_receiver(&self, name: &str) -> Option<&Expr> {
        if let Some(binding) = self.binding_ids.get(name) {
            return self.map_entry_set_view_receivers_by_binding.get(binding);
        }
        self.map_entry_set_view_receivers.get(name)
    }

    pub(crate) fn map_entry_value_receiver(&self, name: &str) -> Option<&Expr> {
        if let Some(binding) = self.binding_ids.get(name) {
            return self.map_entry_value_receivers_by_binding.get(binding);
        }
        self.map_entry_value_receivers.get(name)
    }

    pub(crate) fn unmodifiable_collection_view_source(&self, name: &str) -> Option<&Expr> {
        if let Some(binding) = self.binding_ids.get(name) {
            return self
                .unmodifiable_collection_view_sources_by_binding
                .get(binding);
        }
        self.unmodifiable_collection_view_sources.get(name)
    }

    pub(crate) fn fixed_size_list_view_args(&self, name: &str) -> Option<&[Expr]> {
        if let Some(binding) = self.binding_ids.get(name) {
            return self
                .fixed_size_list_view_args_by_binding
                .get(binding)
                .map(Vec::as_slice);
        }
        self.fixed_size_list_view_args.get(name).map(Vec::as_slice)
    }

    pub(crate) fn immutable_collection_view_local(
        &self,
        name: &str,
    ) -> Option<ImmutableCollectionViewMutationProfile> {
        if let Some(binding) = self.binding_ids.get(name) {
            return self
                .immutable_collection_view_locals_by_binding
                .get(binding)
                .copied();
        }
        self.immutable_collection_view_locals.get(name).copied()
    }

    pub(crate) fn simple_map_entry_value(&self, name: &str) -> Option<(&Expr, &Expr)> {
        if let Some(binding) = self.binding_ids.get(name) {
            return self
                .simple_map_entry_values_by_binding
                .get(binding)
                .map(|(key, value)| (key, value));
        }
        self.simple_map_entry_values
            .get(name)
            .map(|(key, value)| (key, value))
    }

    pub fn insert_local(&mut self, name: impl Into<String>, ty: JavaType) {
        let name = name.into();
        self.clear_local_side_facts(&name);
        self.local_shapes
            .insert(name.clone(), RustSlotShape::for_storage(ty.clone()));
        self.types.insert_local(name.clone(), ty);
        self.bind_name(BindingKind::Local, name);
    }

    pub(crate) fn mark_callable_local(&mut self, name: impl Into<String>) {
        self.types.mark_callable_local(name);
    }

    pub(crate) fn mark_comparator_object_local(&mut self, name: impl Into<String>) {
        self.types.mark_comparator_object_local(name);
    }

    pub(crate) fn mark_comparator_object_field(&mut self, name: impl Into<String>) {
        self.types.mark_comparator_object_field(name);
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
        self.clear_local_side_facts(&name);
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
        self.clear_local_side_facts(&name);
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
        self.clear_local_side_facts(&name);
        self.types.insert_local(name.clone(), java_ty);
        self.local_shapes.insert(name.clone(), shape);
        if let Some(binding) = binding {
            self.binding_ids.insert(name, binding);
        } else {
            self.bind_name(BindingKind::Local, name);
        }
    }

    pub(crate) fn insert_local_shape_with_access_name_and_java_type(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
        java_ty: JavaType,
    ) {
        let name = name.into();
        let access_name = access_name.into();
        self.clear_local_side_facts(&access_name);
        self.insert_local_shape_with_java_type(name.clone(), shape, java_ty);
        self.binding_access_names
            .insert(name.clone(), access_name.clone());
        self.binding_access_names
            .insert(access_name.clone(), access_name);
    }

    pub(crate) fn insert_local_shape_with_access_name_java_type_and_binding(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
        java_ty: JavaType,
        binding: Option<BindingId>,
    ) {
        let name = name.into();
        let access_name = access_name.into();
        self.clear_local_side_facts(&access_name);
        self.insert_local_shape_with_java_type_and_binding(
            name.clone(),
            shape,
            java_ty,
            binding.clone(),
        );
        self.bind_access_name_alias(&name, &access_name, binding);
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
        self.types.insert_param(name.clone(), java_ty);
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
        self.types.insert_param(name.clone(), java_ty);
        self.local_shapes.insert(name.clone(), shape);
        if let Some(binding) = binding {
            self.binding_ids.insert(name, binding);
        } else {
            self.bind_name(BindingKind::Param, name);
        }
    }

    pub(crate) fn insert_param_shape_with_access_name_and_java_type(
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

    pub(crate) fn insert_param_shape_with_access_name_java_type_and_binding(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
        java_ty: JavaType,
        binding: Option<BindingId>,
    ) {
        let name = name.into();
        let access_name = access_name.into();
        self.insert_param_shape_with_java_type_and_binding(
            name.clone(),
            shape,
            java_ty,
            binding.clone(),
        );
        self.bind_access_name_alias(&name, &access_name, binding);
        self.binding_access_names
            .insert(name.clone(), access_name.clone());
        self.binding_access_names
            .insert(access_name.clone(), access_name);
    }

    pub(crate) fn insert_loop_variable_shape_with_access_name_and_binding(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
        shape: RustSlotShape,
        binding: Option<BindingId>,
    ) {
        let name = name.into();
        let access_name = access_name.into();
        self.clear_local_side_facts_for_access_name(&name, &access_name);
        self.types.insert_local(name.clone(), shape.java_ty.clone());
        self.local_shapes.insert(name.clone(), shape);
        if let Some(binding) = binding {
            self.binding_ids.insert(name.clone(), binding.clone());
            self.bind_access_name_alias(&name, &access_name, Some(binding));
        } else {
            self.bind_name(BindingKind::LoopVariable, name.clone());
        }
        self.binding_access_names
            .insert(name.clone(), access_name.clone());
        self.binding_access_names
            .insert(access_name.clone(), access_name);
    }

    fn bind_access_name_alias(
        &mut self,
        name: &str,
        access_name: &str,
        binding: Option<BindingId>,
    ) {
        if name != access_name
            && let Some(binding) = binding
        {
            self.binding_ids.insert(access_name.to_string(), binding);
        }
    }

    fn insert_catch_param_with_binding(
        &mut self,
        name: impl Into<String>,
        ty: JavaType,
        binding: Option<BindingId>,
    ) {
        let name = name.into();
        self.clear_local_side_facts(&name);
        self.local_shapes
            .insert(name.clone(), RustSlotShape::for_storage(ty.clone()));
        self.types.insert_local(name.clone(), ty);
        if let Some(binding) = binding {
            self.binding_ids.insert(name, binding);
        } else {
            self.bind_name(BindingKind::CatchParam, name);
        }
    }

    pub(crate) fn insert_catch_param_with_access_name_and_binding(
        &mut self,
        name: impl Into<String>,
        access_name: impl Into<String>,
        ty: JavaType,
        binding: Option<BindingId>,
    ) {
        let name = name.into();
        let access_name = access_name.into();
        self.clear_local_side_facts_for_access_name(&name, &access_name);
        self.insert_catch_param_with_binding(name.clone(), ty, binding.clone());
        self.bind_access_name_alias(&name, &access_name, binding);
        self.binding_access_names
            .insert(name.clone(), access_name.clone());
        self.binding_access_names
            .insert(access_name.clone(), access_name);
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

    fn insert_field_shape_with_access_name(
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

    pub(crate) fn insert_field_shape_with_access_name_and_java_type(
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

    pub(crate) fn insert_type_static_storage_field_shape_with_access_name(
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

    fn insert_type_static_storage_field_shape_with_access_name_and_java_type(
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

    fn insert_static_field_shape_with_access_name(
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

    fn insert_static_field_shape_with_access_name_and_java_type(
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

    pub(crate) fn insert_static_field_shape_for_owner_with_access_name(
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

    fn insert_static_storage_field_shape_with_access_name(
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

    pub(crate) fn insert_static_storage_field_shape_owned_by_with_access_name(
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
        Some(crate::java_naming::rust_name_for_java_named_type(owner))
    }

    fn insert_static_storage_field_shape_with_access_name_and_java_type(
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

    pub(crate) fn insert_binding_access_name(
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

    /// Local bindings available at the current lowering site. Anonymous-object
    /// lowering uses this to materialize captured outer values as adapter
    /// fields instead of leaving generated trait methods to capture a function
    /// environment.
    pub(crate) fn local_shapes(&self) -> impl Iterator<Item = (&str, &RustSlotShape)> {
        self.local_shapes
            .iter()
            .map(|(name, shape)| (name.as_str(), shape))
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
    java_types_have_same_erased_occurrence_identity(local.java_ty(), declared_ty)
        || java_types_have_same_erased_occurrence_identity(&local.shape().java_ty, declared_ty)
}

fn unique_local_plan_for_declared_type<'a>(
    locals: &'a [LocalPlan],
    name: &str,
    declared_ty: &JavaType,
) -> Option<&'a LocalPlan> {
    let exact_matches = locals
        .iter()
        .filter(|local| {
            local.source_name() == name && local_plan_matches_declared_type(local, declared_ty)
        })
        .collect::<Vec<_>>();
    if exact_matches.is_empty() {
        return None;
    }
    match exact_matches.as_slice() {
        [local] => Some(*local),
        _ => None,
    }
}

pub(crate) fn idless_compatibility_local_plan_for_declared_type<'a>(
    locals: &'a [LocalPlan],
    name: &str,
    declared_ty: &JavaType,
) -> Option<&'a LocalPlan> {
    unique_local_plan_for_declared_type(locals, name, declared_ty)
}

pub(crate) fn fallback_local_shape_from_declared_type(ty: &JavaType) -> RustSlotShape {
    RustSlotShape::conservative(ty.clone())
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ExprShapeContext<'a> {
    signatures: Option<&'a SignaturePlan>,
    representations: Option<&'a RepresentationPlan>,
    facts: Option<&'a FactSet>,
}

impl<'a> ExprShapeContext<'a> {
    pub fn new(
        signatures: Option<&'a SignaturePlan>,
        representations: Option<&'a RepresentationPlan>,
        facts: Option<&'a FactSet>,
    ) -> Self {
        Self {
            signatures,
            representations,
            facts,
        }
    }

    pub fn signatures(self) -> Option<&'a SignaturePlan> {
        self.signatures
    }

    pub fn representations(self) -> Option<&'a RepresentationPlan> {
        self.representations
    }

    pub fn facts(self) -> Option<&'a FactSet> {
        self.facts
    }
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
    signatures.resolved_source_method_return_shape(call, source_name)
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
            let shape = signatures.required_declared_field_shape(ty, field);
            let field_shape_java_ty = shape.java_ty.clone();
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
                    static_access_name.clone(),
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
                    static_access_name.clone(),
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
                    instance_access_name.clone(),
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
            if field_shape_java_ty != field.ty
                && comparator_object_field_initializer(&field.ty, field.initializer.as_ref())
            {
                env.mark_comparator_object_field(field.name.clone());
                env.mark_comparator_object_field(static_access_name.clone());
                env.mark_comparator_object_field(instance_access_name.clone());
                env.mark_comparator_object_field(crate::parser::to_snake_case(&field.name));
            }
        }
    }
    env
}

fn comparator_object_field_initializer(declared_ty: &JavaType, initializer: Option<&Expr>) -> bool {
    java_type_is_comparator(declared_ty)
        && initializer.is_some_and(|initializer| match initializer {
            Expr::New { ty, body: None, .. } => !java_type_is_comparator(ty),
            Expr::Cast { ty, expr } => {
                java_type_is_comparator(ty)
                    && matches!(
                        expr.as_ref(),
                        Expr::New {
                            ty,
                            body: None,
                            ..
                        } if !java_type_is_comparator(ty)
                    )
            }
            _ => false,
        })
}

fn java_type_is_comparator(ty: &JavaType) -> bool {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
            canonical
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Comparator")
        }
        _ => false,
    }
}

pub fn shape_env_for_constructor(
    ty: &TypeDecl,
    ctor: &crate::java_ir::ConstructorDecl,
    constructor_signature: &ConstructorSignaturePlan,
    signatures: &SignaturePlan,
) -> ShapeEnv {
    let mut env = shape_env_for_type(ty, signatures);
    insert_param_shapes(&mut env, &ctor.params, &constructor_signature.params());
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
    insert_param_shapes(&mut env, &method.params, &method_signature.params());
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
            let access_name = env
                .binding_access_name(source_name)
                .map(str::to_string)
                .unwrap_or_else(|| name.clone());
            let local_plan =
                local_plan_for_declaration_shape(signatures, locals, binding.as_ref(), name, ty);
            let shape = local_plan
                .map(|plan| plan.shape().clone())
                .unwrap_or_else(|| fallback_local_shape_from_declared_type(ty));
            let env_binding = binding
                .clone()
                .or_else(|| local_plan.map(|plan| plan.binding().clone()));
            env.insert_local_shape_with_access_name_java_type_and_binding(
                source_name.clone(),
                access_name.clone(),
                shape,
                ty.clone(),
                env_binding,
            );
            if let Some(receiver) = map_key_set_view_initializer_receiver(initializer.as_ref()) {
                env.insert_map_key_set_view_receiver(name.clone(), receiver.clone());
                if source_name != name {
                    env.insert_map_key_set_view_receiver(source_name.clone(), receiver.clone());
                }
                if access_name != *name && access_name != *source_name {
                    env.insert_map_key_set_view_receiver(access_name.clone(), receiver.clone());
                }
            }
            if let Some(receiver) = map_values_view_initializer_receiver(initializer.as_ref()) {
                env.insert_map_values_view_receiver(name.clone(), receiver.clone());
                if source_name != name {
                    env.insert_map_values_view_receiver(source_name.clone(), receiver.clone());
                }
                if access_name != *name && access_name != *source_name {
                    env.insert_map_values_view_receiver(access_name.clone(), receiver.clone());
                }
            }
            if let Some(receiver) = map_entry_set_view_initializer_receiver(initializer.as_ref()) {
                env.insert_map_entry_set_view_receiver(name.clone(), receiver.clone());
                if source_name != name {
                    env.insert_map_entry_set_view_receiver(source_name.clone(), receiver.clone());
                }
                if access_name != *name && access_name != *source_name {
                    env.insert_map_entry_set_view_receiver(access_name.clone(), receiver.clone());
                }
            }
            if let Some(receiver) = map_entry_value_initializer_receiver(initializer.as_ref(), env)
            {
                env.insert_map_entry_value_receiver(name.clone(), receiver.clone());
                if source_name != name {
                    env.insert_map_entry_value_receiver(source_name.clone(), receiver.clone());
                }
                if access_name != *name && access_name != *source_name {
                    env.insert_map_entry_value_receiver(access_name.clone(), receiver.clone());
                }
            }
            if let Some((key, value)) = initializer
                .as_ref()
                .and_then(simple_map_entry_constructor_args)
            {
                env.insert_simple_map_entry_value(name.clone(), key.clone(), value.clone());
                if initializer
                    .as_ref()
                    .is_some_and(simple_map_entry_constructor_is_immutable)
                {
                    env.insert_simple_immutable_map_entry_local(name.clone());
                }
                if source_name != name {
                    env.insert_simple_map_entry_value(
                        source_name.clone(),
                        key.clone(),
                        value.clone(),
                    );
                    if initializer
                        .as_ref()
                        .is_some_and(simple_map_entry_constructor_is_immutable)
                    {
                        env.insert_simple_immutable_map_entry_local(source_name.clone());
                    }
                }
                if access_name != *name && access_name != *source_name {
                    env.insert_simple_map_entry_value(
                        access_name.clone(),
                        key.clone(),
                        value.clone(),
                    );
                    if initializer
                        .as_ref()
                        .is_some_and(simple_map_entry_constructor_is_immutable)
                    {
                        env.insert_simple_immutable_map_entry_local(access_name.clone());
                    }
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
                .map(|plan| plan.shape().clone())
                .unwrap_or_else(|| fallback_local_shape_from_declared_type(var_type));
            let env_binding = binding
                .clone()
                .or_else(|| local_plan.map(|plan| plan.binding().clone()));
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

fn map_entry_set_view_initializer_receiver(initializer: Option<&Expr>) -> Option<&Expr> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = initializer?
    else {
        return None;
    };
    (args.is_empty() && matches!(name.as_str(), "entrySet" | "entry_set"))
        .then_some(receiver.as_ref())
}

fn map_entry_value_initializer_receiver(
    initializer: Option<&Expr>,
    env: &ShapeEnv,
) -> Option<Expr> {
    if let Some(receiver) = map_entry_next_backing_receiver(initializer) {
        return Some(receiver.clone());
    }
    let Expr::Call {
        receiver: Some(iterator_expr),
        name: next_name,
        args: next_args,
        ..
    } = initializer?
    else {
        return None;
    };
    if next_name != "next" || !next_args.is_empty() {
        return None;
    }
    let Expr::Call {
        receiver: Some(entry_set_expr),
        name: iterator_name,
        args: iterator_args,
        ..
    } = iterator_expr.as_ref()
    else {
        return None;
    };
    if iterator_name != "iterator" || !iterator_args.is_empty() {
        return None;
    }
    let Expr::Var(local_name) = entry_set_expr.as_ref() else {
        return None;
    };
    env.map_entry_set_view_receiver(local_name).cloned()
}

fn map_entry_next_backing_receiver(receiver: Option<&Expr>) -> Option<&Expr> {
    let Expr::Call {
        receiver: Some(iterator_expr),
        name: next_name,
        args: next_args,
        ..
    } = receiver?
    else {
        return None;
    };
    if next_name != "next" || !next_args.is_empty() {
        return None;
    }
    let Expr::Call {
        receiver: Some(entry_set_expr),
        name: iterator_name,
        args: iterator_args,
        ..
    } = iterator_expr.as_ref()
    else {
        return None;
    };
    if iterator_name != "iterator" || !iterator_args.is_empty() {
        return None;
    }
    let Expr::Call {
        receiver: Some(map_receiver),
        name: entry_set_name,
        args: entry_set_args,
        ..
    } = entry_set_expr.as_ref()
    else {
        return None;
    };
    if !matches!(entry_set_name.as_str(), "entrySet" | "entry_set") || !entry_set_args.is_empty() {
        return None;
    }
    Some(map_receiver.as_ref())
}

fn simple_map_entry_constructor_args(expr: &Expr) -> Option<(&Expr, &Expr)> {
    let Expr::New { ty, args, .. } = expr else {
        return None;
    };
    if !java_type_is_simple_map_entry(ty) || args.len() != 2 {
        return None;
    }
    Some((&args[0], &args[1]))
}

fn java_type_is_simple_map_entry(ty: &JavaType) -> bool {
    let JavaType::Named { name, .. } = ty else {
        return false;
    };
    matches!(
        name.rsplit('.').next().unwrap_or(name).trim(),
        "SimpleEntry" | "SimpleImmutableEntry"
    )
}

fn simple_map_entry_constructor_is_immutable(expr: &Expr) -> bool {
    let Expr::New { ty, args, .. } = expr else {
        return false;
    };
    args.len() == 2 && java_type_is_simple_immutable_map_entry(ty)
}

fn java_type_is_simple_immutable_map_entry(ty: &JavaType) -> bool {
    let JavaType::Named { name, .. } = ty else {
        return false;
    };
    name.rsplit('.').next().unwrap_or(name).trim() == "SimpleImmutableEntry"
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
    unique_local_plan_for_declared_type(locals, name, declared_ty).map(|local| local.shape())
}

fn insert_param_shapes(env: &mut ShapeEnv, source_params: &[Param], params: &[ParamPlan]) {
    let access_names = crate::java_naming::source_binding_rust_names(
        source_params.iter().map(|param| param.source_name.as_str()),
    );
    for (source_param, param) in source_params.iter().zip(params.iter()) {
        let access_name = access_names
            .get(&source_param.source_name)
            .cloned()
            .unwrap_or_else(|| param.name().to_string());
        env.insert_param_shape_with_access_name_java_type_and_binding(
            source_param.source_name.clone(),
            access_name.clone(),
            param.shape().clone(),
            param.java_ty().clone(),
            source_param.binding.clone(),
        );
        if param.name() != source_param.source_name {
            env.insert_param_shape_with_access_name_java_type_and_binding(
                param.name().to_string(),
                access_name,
                param.shape().clone(),
                param.java_ty().clone(),
                source_param.binding.clone(),
            );
        }
    }
    for param in params.iter().skip(source_params.len()) {
        env.insert_param_shape_with_java_type(
            param.name().to_string(),
            param.shape().clone(),
            param.java_ty().clone(),
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
        && let Some(plan) = crate::member_access_plan::plan_member_access_with_facts(
            receiver,
            field,
            env.type_env(),
            ctx.facts.unwrap_or(&FactSet::default()),
        )
        && let crate::member_access_plan::MemberAccessOp::StaticConstant { owner } = plan.op()
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
    .map(|plan| plan.shape().clone())
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
        LocalPlan::new(
            crate::binding_id::BindingId::compatibility_local("value"),
            "value".to_string(),
            java_ty,
            shape,
            false,
        )
    }

    #[test]
    fn unique_idless_local_plan_match_is_still_accepted() {
        let plans = vec![int_plan_with_ownership(OwnershipShape::Owned)];
        let java_ty = JavaType::Primitive(PrimitiveType::Int);

        assert_eq!(
            idless_compatibility_local_plan_for_declared_type(&plans, "value", &java_ty)
                .map(|plan| plan.shape().ownership),
            Some(OwnershipShape::Owned)
        );
    }

    #[test]
    fn ambiguous_same_name_same_type_idless_local_plan_fails_closed() {
        let plans = vec![
            int_plan_with_ownership(OwnershipShape::Owned),
            int_plan_with_ownership(OwnershipShape::SharedBorrow),
        ];
        let java_ty = JavaType::Primitive(PrimitiveType::Int);

        assert!(
            idless_compatibility_local_plan_for_declared_type(&plans, "value", &java_ty).is_none(),
            "id-less same-name/same-type declarations must fail closed instead of selecting by traversal order"
        );
    }

    fn erased_object_plan_with_ownership(ownership: OwnershipShape) -> LocalPlan {
        let java_ty = JavaType::Unknown("Object".to_string());
        let mut shape = RustSlotShape::conservative(java_ty.clone());
        shape.ownership = ownership;
        shape.dispatch = DispatchShape::ErasedObject;
        LocalPlan::new(
            crate::binding_id::BindingId::compatibility_local("value"),
            "value".to_string(),
            java_ty,
            shape,
            false,
        )
    }

    #[test]
    fn ambiguous_erased_object_idless_local_plan_lookup_fails_closed() {
        let plans = vec![
            erased_object_plan_with_ownership(OwnershipShape::Owned),
            erased_object_plan_with_ownership(OwnershipShape::SharedBorrow),
        ];
        let declared_ty = JavaType::Unknown("Object".to_string());

        assert!(
            idless_compatibility_local_plan_for_declared_type(&plans, "value", &declared_ty)
                .is_none(),
            "ambiguous erased-object compatibility plans must fail closed"
        );
    }

    #[test]
    fn mixed_erased_object_idless_local_plan_lookup_fails_closed() {
        let declared_ty = JavaType::Unknown("Object".to_string());
        let mut concrete_shape = RustSlotShape::conservative(JavaType::String);
        concrete_shape.ownership = OwnershipShape::Owned;
        let plans = vec![
            erased_object_plan_with_ownership(OwnershipShape::Owned),
            LocalPlan::new(
                crate::binding_id::BindingId::compatibility_local("value"),
                "value".to_string(),
                declared_ty.clone(),
                concrete_shape,
                false,
            ),
        ];

        assert!(
            idless_compatibility_local_plan_for_declared_type(&plans, "value", &declared_ty)
                .is_none(),
            "mixed Object compatibility plans must fail closed instead of selecting the only erased boundary"
        );
    }

    #[test]
    fn local_plan_type_mismatch_does_not_fall_back_by_name() {
        let string_ty = JavaType::String;
        let mut string_shape = RustSlotShape::conservative(string_ty.clone());
        string_shape.ownership = OwnershipShape::Owned;
        let plan = LocalPlan::new(
            crate::binding_id::BindingId::compatibility_local("value"),
            "value".to_string(),
            string_ty,
            string_shape,
            false,
        );
        let plans = vec![plan];

        assert!(
            idless_compatibility_local_plan_for_declared_type(
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
        let plans = vec![LocalPlan::new(
            crate::binding_id::BindingId::compatibility_local("value"),
            "value".to_string(),
            string_ty,
            string_shape,
            false,
        )];

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
        let plans = vec![LocalPlan::new(
            crate::binding_id::BindingId::compatibility_local("reader"),
            "reader".to_string(),
            JavaType::Unknown("Object".to_string()),
            concrete_shape,
            false,
        )];

        assert!(
            idless_compatibility_local_plan_for_declared_type(&plans, "reader", &declared_ty)
                .is_some(),
            "erased carriers may still have a concrete solved shape matching the declared type"
        );
    }

    #[test]
    fn bound_var_decl_shape_learning_does_not_fall_back_by_name() {
        let java_ty = JavaType::Primitive(PrimitiveType::Int);
        let mut planned_shape = RustSlotShape::conservative(java_ty.clone());
        planned_shape.ownership = OwnershipShape::SharedBorrow;
        let plans = vec![LocalPlan::new(
            BindingId::new(BindingKind::Local, "value".to_string(), 0),
            "value".to_string(),
            java_ty.clone(),
            planned_shape,
            false,
        )];
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
        let plans = vec![LocalPlan::new(
            BindingId::new(BindingKind::Local, "item".to_string(), 0),
            "item".to_string(),
            java_ty.clone(),
            planned_shape,
            false,
        )];
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
        let plans = vec![LocalPlan::new(
            selected_binding.clone(),
            "value".to_string(),
            java_ty.clone(),
            RustSlotShape::for_storage(java_ty.clone()),
            false,
        )];
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
    fn bound_var_decl_shape_learning_preserves_keyword_access_name_binding() {
        let java_ty = JavaType::String;
        let binding = BindingId::new(BindingKind::Local, "self".to_string(), 0);
        let plans = vec![LocalPlan::new(
            binding.clone(),
            "self__".to_string(),
            java_ty.clone(),
            RustSlotShape::for_storage(java_ty.clone()),
            false,
        )];
        let stmt = Stmt::VarDecl {
            ty: java_ty,
            name: "self__".to_string(),
            source_name: "self".to_string(),
            initializer: Some(Expr::Literal(crate::java_ir::Literal::String(
                "x".to_string(),
            ))),
            binding: Some(binding.clone()),
        };
        let mut env = ShapeEnv::new();

        learn_stmt_shapes(&stmt, &mut env, &plans, &SignaturePlan::default());

        assert_eq!(env.binding_id("self"), Some(&binding));
        assert_eq!(env.binding_id("self__"), Some(&binding));
        assert!(env.local_shape("self").is_some());
    }

    #[test]
    fn idless_enhanced_for_shape_learning_preserves_selected_compatibility_binding() {
        let java_ty = JavaType::String;
        let selected_binding = BindingId::new(BindingKind::LoopVariable, "item".to_string(), 4);
        let plans = vec![LocalPlan::new(
            selected_binding.clone(),
            "item".to_string(),
            java_ty.clone(),
            RustSlotShape::for_storage(java_ty.clone()),
            false,
        )];
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
    fn bound_map_view_facts_do_not_fall_back_to_same_name_local() {
        let mut env = ShapeEnv::new();
        let first = BindingId::new(BindingKind::Local, "entries".to_string(), 0);
        let second = BindingId::new(BindingKind::Local, "entries".to_string(), 1);
        let shape = RustSlotShape::for_storage(JavaType::Named {
            name: "java.util.Set".to_string(),
            args: vec![],
        });

        env.insert_local_shape_with_java_type_and_binding(
            "entries",
            shape.clone(),
            shape.java_ty.clone(),
            Some(first),
        );
        env.insert_map_entry_set_view_receiver("entries", Expr::Var("first_map".to_string()));
        assert_eq!(
            env.map_entry_set_view_receiver("entries"),
            Some(&Expr::Var("first_map".to_string()))
        );

        env.insert_local_shape_with_java_type_and_binding(
            "entries",
            shape.clone(),
            shape.java_ty.clone(),
            Some(second),
        );

        assert_eq!(
            env.map_entry_set_view_receiver("entries"),
            None,
            "a same-name redeclaration must not inherit the previous binding's map-entry view"
        );
    }

    #[test]
    fn bound_collection_view_facts_do_not_survive_same_name_redeclaration() {
        let mut env = ShapeEnv::new();
        let first = BindingId::new(BindingKind::Local, "view".to_string(), 0);
        let second = BindingId::new(BindingKind::Local, "view".to_string(), 1);
        let shape = RustSlotShape::for_storage(JavaType::Named {
            name: "java.util.List".to_string(),
            args: vec![JavaType::String],
        });

        env.insert_local_shape_with_java_type_and_binding(
            "view",
            shape.clone(),
            shape.java_ty.clone(),
            Some(first),
        );
        env.insert_unmodifiable_collection_view_source("view", Expr::Var("values".to_string()));
        env.insert_fixed_size_list_view_args("view", vec![Expr::Var("value".to_string())]);
        env.insert_immutable_collection_view_local(
            "view",
            ImmutableCollectionViewMutationProfile::Empty,
        );
        assert_eq!(
            env.unmodifiable_collection_view_source("view"),
            Some(&Expr::Var("values".to_string()))
        );
        assert_eq!(
            env.fixed_size_list_view_args("view"),
            Some(&[Expr::Var("value".to_string())][..])
        );
        assert_eq!(
            env.immutable_collection_view_local("view"),
            Some(ImmutableCollectionViewMutationProfile::Empty)
        );

        env.insert_local_shape_with_java_type_and_binding(
            "view",
            shape.clone(),
            shape.java_ty.clone(),
            Some(second),
        );

        assert_eq!(env.unmodifiable_collection_view_source("view"), None);
        assert_eq!(env.fixed_size_list_view_args("view"), None);
        assert_eq!(env.immutable_collection_view_local("view"), None);
    }

    #[test]
    fn map_view_shape_learning_preserves_access_name_alias() {
        fn view_decl(name: &str) -> Stmt {
            Stmt::VarDecl {
                ty: JavaType::Named {
                    name: "java.util.Collection".to_string(),
                    args: Vec::new(),
                },
                name: "self".to_string(),
                source_name: "self".to_string(),
                initializer: Some(Expr::Call {
                    receiver: Some(Box::new(Expr::Var("map".to_string()))),
                    name: name.to_string(),
                    args: Vec::new(),
                    resolved: None,
                }),
                binding: None,
            }
        }

        let mut key_env = ShapeEnv::new();
        key_env.insert_binding_access_name("self", "self__");
        learn_stmt_shapes(
            &view_decl("keySet"),
            &mut key_env,
            &[],
            &SignaturePlan::default(),
        );
        assert_eq!(
            key_env.map_key_set_view_receiver("self__"),
            Some(&Expr::Var("map".to_string()))
        );

        let mut values_env = ShapeEnv::new();
        values_env.insert_binding_access_name("self", "self__");
        learn_stmt_shapes(
            &view_decl("values"),
            &mut values_env,
            &[],
            &SignaturePlan::default(),
        );
        assert_eq!(
            values_env.map_values_view_receiver("self__"),
            Some(&Expr::Var("map".to_string()))
        );

        let mut entries_env = ShapeEnv::new();
        entries_env.insert_binding_access_name("self", "self__");
        learn_stmt_shapes(
            &view_decl("entrySet"),
            &mut entries_env,
            &[],
            &SignaturePlan::default(),
        );
        assert_eq!(
            entries_env.map_entry_set_view_receiver("self__"),
            Some(&Expr::Var("map".to_string()))
        );
    }

    #[test]
    fn map_entry_shape_learning_preserves_access_name_alias() {
        let mut env = ShapeEnv::new();
        env.insert_binding_access_name("self", "self__");
        env.insert_local("map", JavaType::Unknown("Map".to_string()));

        let entry_binding = BindingId::new(BindingKind::Local, "self".to_string(), 0);
        let entry_decl = Stmt::VarDecl {
            ty: JavaType::Named {
                name: "java.util.Map.Entry".to_string(),
                args: vec![JavaType::String, JavaType::String],
            },
            name: "self".to_string(),
            source_name: "self".to_string(),
            initializer: Some(crate::parser::parse_expr(
                "map.entrySet().iterator().next()",
            )),
            binding: Some(entry_binding),
        };

        learn_stmt_shapes(&entry_decl, &mut env, &[], &SignaturePlan::default());

        assert_eq!(
            env.map_entry_value_receiver("self__"),
            Some(&Expr::Var("map".to_string()))
        );

        let simple_binding = BindingId::new(BindingKind::Local, "entry".to_string(), 0);
        env.insert_binding_access_name("entry", "entry__");
        let simple_decl = Stmt::VarDecl {
            ty: JavaType::Named {
                name: "java.util.Map.Entry".to_string(),
                args: vec![JavaType::String, JavaType::String],
            },
            name: "entry".to_string(),
            source_name: "entry".to_string(),
            initializer: Some(crate::parser::parse_expr(
                "new java.util.AbstractMap.SimpleEntry(\"a\", \"1\")",
            )),
            binding: Some(simple_binding),
        };

        learn_stmt_shapes(&simple_decl, &mut env, &[], &SignaturePlan::default());

        let (key, value) = env
            .simple_map_entry_value("entry__")
            .expect("simple-entry constructor facts should be available by access name");
        assert_eq!(
            key,
            &Expr::Literal(crate::java_ir::Literal::String("a".to_string()))
        );
        assert_eq!(
            value,
            &Expr::Literal(crate::java_ir::Literal::String("1".to_string()))
        );
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
    fn bound_local_env_insert_preserves_resolver_binding_id_for_access_name() {
        let mut env = ShapeEnv::new();
        let binding = BindingId::new(BindingKind::Local, "self".to_string(), 1);
        env.insert_local_shape_with_access_name_java_type_and_binding(
            "self",
            "self__",
            RustSlotShape::for_storage(JavaType::String),
            JavaType::String,
            Some(binding.clone()),
        );

        assert_eq!(env.binding_id("self"), Some(&binding));
        assert_eq!(env.binding_id("self__"), Some(&binding));
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
    fn bound_param_env_insert_preserves_resolver_binding_id_for_access_name() {
        let mut env = ShapeEnv::new();
        let binding = BindingId::new(BindingKind::Param, "type".to_string(), 1);
        env.insert_param_shape_with_access_name_java_type_and_binding(
            "type",
            "r#type",
            RustSlotShape::for_storage(JavaType::String),
            JavaType::String,
            Some(binding.clone()),
        );

        assert_eq!(env.binding_id("type"), Some(&binding));
        assert_eq!(env.binding_id("r#type"), Some(&binding));
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
    fn bound_loop_variable_env_insert_preserves_resolver_binding_id_for_access_name() {
        let mut env = ShapeEnv::new();
        let binding = BindingId::new(BindingKind::LoopVariable, "crate".to_string(), 1);
        env.insert_loop_variable_shape_with_access_name_and_binding(
            "crate",
            "crate__",
            RustSlotShape::for_storage(JavaType::String),
            Some(binding.clone()),
        );

        assert_eq!(env.binding_id("crate"), Some(&binding));
        assert_eq!(env.binding_id("crate__"), Some(&binding));
    }

    #[test]
    fn loop_variable_insert_clears_shadowed_collection_side_facts() {
        let mut env = ShapeEnv::new();
        env.insert_map_key_set_view_receiver("crate", Expr::Var("source".to_string()));
        env.insert_map_values_view_receiver("crate__", Expr::Var("access".to_string()));
        env.insert_unmodifiable_collection_view_source("crate", Expr::Var("values".to_string()));
        env.insert_nested_array2_local("crate__");

        env.insert_loop_variable_shape_with_access_name_and_binding(
            "crate",
            "crate__",
            RustSlotShape::for_storage(JavaType::String),
            None,
        );

        assert_eq!(env.map_key_set_view_receiver("crate"), None);
        assert_eq!(env.map_values_view_receiver("crate__"), None);
        assert_eq!(env.unmodifiable_collection_view_source("crate"), None);
        assert!(!env.local_uses_nested_array2_slice("crate__"));
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

    #[test]
    fn bound_catch_param_env_insert_preserves_resolver_binding_id_for_access_name() {
        let mut env = ShapeEnv::new();
        let binding = BindingId::new(BindingKind::CatchParam, "type".to_string(), 1);
        env.insert_catch_param_with_access_name_and_binding(
            "type",
            "type__",
            JavaType::Named {
                name: "JavaException".to_string(),
                args: Vec::new(),
            },
            Some(binding.clone()),
        );

        assert_eq!(env.binding_id("type"), Some(&binding));
        assert_eq!(env.binding_id("type__"), Some(&binding));
        assert_eq!(env.binding_access_name("type"), Some("type__"));
    }

    #[test]
    fn catch_param_insert_clears_shadowed_collection_side_facts() {
        let mut env = ShapeEnv::new();
        env.insert_map_entry_set_view_receiver("type", Expr::Var("entries".to_string()));
        env.insert_map_entry_value_receiver("type__", Expr::Var("entry".to_string()));
        env.insert_simple_map_entry_value(
            "type",
            Expr::Literal(crate::java_ir::Literal::String("k".to_string())),
            Expr::Literal(crate::java_ir::Literal::String("v".to_string())),
        );
        env.insert_immutable_collection_view_local(
            "type__",
            ImmutableCollectionViewMutationProfile::Empty,
        );

        env.insert_catch_param_with_access_name_and_binding(
            "type",
            "type__",
            JavaType::Named {
                name: "JavaException".to_string(),
                args: Vec::new(),
            },
            None,
        );

        assert_eq!(env.map_entry_set_view_receiver("type"), None);
        assert_eq!(env.map_entry_value_receiver("type__"), None);
        assert_eq!(env.simple_map_entry_value("type"), None);
        assert_eq!(env.immutable_collection_view_local("type__"), None);
    }
}
