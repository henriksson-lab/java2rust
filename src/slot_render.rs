use crate::effective_facts::EffectiveFacts;
use crate::facts::{
    DispatchRequirementKind, ExternalLibraryMode, FactProvenance, FactSet, HierarchyStrategy,
};
use crate::java_ir::{JavaType, PrimitiveType, WildcardBound};
use crate::java_naming::rust_name_for_java_named_type;
use crate::representation::{RepresentationPlan, RepresentationProvenance};
use crate::rust_ir::{RustExpr, RustExprShape, RustType, rust_type_arg_projection_or_raw};
use crate::slot_shape::{DispatchShape, OwnershipShape, RustSlotShape, ViewShape};
use crate::storage_strategy::StoragePlacement;
use crate::types::optional_element_type;

pub(crate) fn rust_generic_type(base: impl Into<String>, args: Vec<RustType>) -> RustType {
    RustType::Generic {
        base: base.into(),
        args,
    }
}

/// Static members live on the nominal source declaration, not its virtual
/// instance carrier. Java owner binders are out of scope in a static member;
/// spell their existing planned raw erasures rather than leaking those binders
/// into the caller or inventing a second wildcard/Any policy.
pub(crate) fn rust_source_static_owner_type(
    owner: &str,
    effective_facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> RustType {
    let declaration_name = representations
        .type_representation(owner)
        .map(|entry| entry.java_name())
        .unwrap_or(owner);
    let base = rust_name_for_java_named_type(declaration_name);
    match representations.raw_generic_arguments_for(declaration_name) {
        Some(arguments) if !arguments.is_empty() => RustType::Generic {
            base,
            args: arguments
                .iter()
                .map(|argument| {
                    rust_type_for_java_generic_arg_with_representations(
                        argument,
                        effective_facts,
                        representations,
                    )
                })
                .collect(),
        },
        _ => RustType::Named(base),
    }
}

pub(crate) fn reference_trait_type_name(trait_path: &str) -> String {
    let (base, suffix) = trait_path
        .find('<')
        .map_or((trait_path, ""), |index| trait_path.split_at(index));
    let (prefix, leaf) = base
        .rsplit_once("::")
        .map_or((String::new(), base), |(prefix, leaf)| {
            (format!("{prefix}::"), leaf)
        });
    format!("{prefix}__JavaReference{leaf}{suffix}")
}

pub(crate) fn rust_reference_dyn_trait_type(
    trait_name: impl Into<String>,
    _sync: bool,
) -> RustType {
    RustType::Named(reference_trait_type_name(&trait_name.into()))
}

pub(crate) fn rust_boxed_dyn_trait_type(trait_name: impl Into<String>) -> RustType {
    rust_generic_type("Box", vec![RustType::DynTrait(trait_name.into())])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotDefaultPolicy {
    NoDefaultForClosedEnum { enum_name: String },
    NoDefaultForDynTrait { trait_name: String },
    NoDefaultForBoxedDynTrait { trait_name: String },
    NoDefaultForErasedObject,
    Value { policy: SlotDefaultValuePolicy },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SlotNoDefaultMessagePolicy {
    message: String,
}

impl SlotNoDefaultMessagePolicy {
    fn compile_error_expr(&self) -> RustExpr {
        crate::unsupported::compile_error_expr(&self.message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotDefaultValuePolicy {
    EmptyStringView,
    EmptyStringReferenceView,
    EmptyIdentityString,
    EmptySliceView,
    PrimitiveBooleanZero,
    PrimitiveCharZero,
    PrimitiveNumericZero,
    PrimitiveFloatZero,
    PrimitiveDoubleZero,
    EmptyString,
    EmptySequence,
    EmptyRetainedList { sync: bool, empty_add_changes: bool },
    EmptyRectangularPrimitiveArray { element: PrimitiveType },
    DefaultTrait,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SlotRustTypePolicy {
    OwnedOrCopy { ty: RustType },
    SharedBorrow { ty: RustType },
    MutableBorrow { ty: RustType },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotValueRustTypePolicy {
    ClosedEnum {
        enum_name: String,
    },
    BoxedDynTrait {
        trait_name: String,
        java_ty: JavaType,
    },
    ReferenceDynTrait {
        trait_name: String,
        java_ty: JavaType,
        sync: bool,
    },
    ErasedObject,
    DynTrait {
        trait_name: String,
        java_ty: JavaType,
    },
    ConcreteDispatch {
        java_ty: JavaType,
    },
    DynTraitView {
        trait_name: String,
        java_ty: JavaType,
    },
    BoxedDynTraitView {
        trait_name: String,
        java_ty: JavaType,
    },
    StringView,
    StringReferenceView,
    SliceView {
        element_ty: JavaType,
        nested_array: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum NativeJavaRustTypePolicy {
    Sequence {
        container: NativeSequenceContainer,
        element_ty: JavaType,
    },
    Optional {
        element_ty: JavaType,
    },
    Direct {
        ty: RustType,
    },
    Named {
        name: String,
    },
    WildcardObject,
    UnknownRaw {
        raw: String,
    },
    ArrayRequiresSequenceElement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSequenceContainer {
    Vec,
    HashSet,
    BTreeSet,
    LinkedHashSet,
    HashFreeSet,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NativeNamedJavaRustTypePolicy {
    name: String,
    policy: NativeJavaRustTypePolicy,
}

impl NativeNamedJavaRustTypePolicy {
    fn into_policy(self) -> NativeJavaRustTypePolicy {
        self.policy
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotContentCapabilityPolicy {
    string_content: bool,
    sequence_element: Option<JavaType>,
}

impl SlotContentCapabilityPolicy {
    pub fn new(string_content: bool, sequence_element: Option<JavaType>) -> Self {
        Self {
            string_content,
            sequence_element,
        }
    }

    pub fn string_content(&self) -> bool {
        self.string_content
    }

    pub fn sequence_element(&self) -> Option<&JavaType> {
        self.sequence_element.as_ref()
    }

    fn into_sequence_element(self) -> Option<JavaType> {
        self.sequence_element
    }
}

pub fn rust_type_for_slot(shape: &RustSlotShape) -> RustType {
    rust_type_for_slot_with_facts(shape, &FactSet::default())
}

pub fn erased_object_rust_type() -> RustType {
    rust_boxed_dyn_trait_type("std::any::Any")
}

/// An inner instance retains its enclosing Java reference. Render that
/// reference through the ordinary storage/representation path, including
/// lexical generic arguments and hierarchy-carrier ownership, for both the
/// capture field and the constructor which initializes it.
pub(crate) fn rust_source_outer_capture_type(
    outer_type: &str,
    type_params: &[String],
    facts: &FactSet,
    representations: Option<&RepresentationPlan>,
) -> RustType {
    let java_ty = JavaType::Named {
        name: outer_type.to_string(),
        args: type_params
            .iter()
            .map(|name| JavaType::Named {
                name: name.clone(),
                args: Vec::new(),
            })
            .collect(),
    };
    if let Some(representations) = representations {
        let shape = crate::shape_projection::storage_shape_for_java_type(representations, java_ty);
        rust_type_for_slot_with_facts_and_representations(&shape, facts, representations)
    } else {
        rust_type_for_slot_with_facts(&RustSlotShape::for_storage(java_ty), facts)
    }
}

pub fn rust_type_for_slot_with_facts(shape: &RustSlotShape, facts: &FactSet) -> RustType {
    let effective_facts = EffectiveFacts::new(facts);
    rust_type_for_slot_with_effective_facts(shape, &effective_facts)
}

pub fn rust_type_for_slot_with_facts_and_representations(
    shape: &RustSlotShape,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> RustType {
    rust_type_for_slot_with_effective_facts_and_representations(
        shape,
        facts,
        EffectiveFacts::new(facts),
        representations,
    )
}

pub fn rust_type_for_slot_with_facts_representations_and_external_index(
    shape: &RustSlotShape,
    facts: &FactSet,
    representations: &RepresentationPlan,
    external_index: &crate::facts::ExternalFactLookupIndex<'_>,
) -> RustType {
    rust_type_for_slot_with_effective_facts_and_representations(
        shape,
        facts,
        EffectiveFacts::with_external_index(facts, external_index),
        representations,
    )
}

fn rust_type_for_slot_with_effective_facts_and_representations(
    shape: &RustSlotShape,
    facts: &FactSet,
    effective_facts: EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> RustType {
    if let Some(rendered) =
        synchronized_functional_slot_rust_type(shape, &effective_facts, |local| {
            rust_type_for_slot_with_effective_facts_and_representations(
                local,
                facts,
                effective_facts,
                representations,
            )
        })
    {
        return rendered;
    }
    if let Some(rendered) = carrier_borrow_slot_rust_type(shape, |owned| {
        rust_type_for_slot_with_effective_facts_and_representations(
            owned,
            facts,
            effective_facts.clone(),
            representations,
        )
    }) {
        return rendered;
    }
    if let Some(rendered) =
        list_interface_slot_rust_type(shape, &effective_facts, Some((facts, representations)))
    {
        return rendered;
    }
    if !shape.nested_native_allocations.is_empty() {
        let mut outer = shape.clone();
        outer.nested_native_allocations.clear();
        let rendered = rust_type_for_slot_with_effective_facts_and_representations(
            &outer,
            facts,
            effective_facts,
            representations,
        );
        return project_nested_native_allocations(shape, &effective_facts, rendered);
    }
    if let Some(rendered) = native_allocation_slot_rust_type(shape, &effective_facts, |payload| {
        rust_type_for_slot_with_effective_facts_and_representations(
            payload,
            facts,
            effective_facts,
            representations,
        )
    }) {
        return rendered;
    }
    let effective_facts =
        effective_facts.with_nested_collection_views(&shape.nested_collection_views);
    let rendered = rust_type_for_slot_with_facts_and_representations_unwrapped(
        shape,
        facts,
        effective_facts,
        representations,
    );
    let rendered = shared_handle_slot_rust_type_with_representations(
        shape,
        &effective_facts,
        representations,
        rendered,
    );
    if matches!(
        shape.ownership,
        OwnershipShape::Owned | OwnershipShape::CopyValue
    ) && let RustType::DynTrait(trait_name) = rendered
    {
        // An owned Rust trait object must have a sized carrier. Borrowed slots
        // are rendered as `&dyn Trait`, and shared Java objects may already be
        // wrapped in Rc/Arc above; only the remaining bare owned form needs a
        // box.
        rust_boxed_dyn_trait_type(trait_name)
    } else {
        rendered
    }
}

fn rust_type_for_slot_with_facts_and_representations_unwrapped(
    shape: &RustSlotShape,
    facts: &FactSet,
    effective_facts: EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> RustType {
    if let Some(element_ty) = shape.optional_element_type() {
        if let JavaType::Named { name, args } = &element_ty
            && args.is_empty()
            && let Some(primitive) = boxed_primitive_storage_java_type(name, &effective_facts)
        {
            return RustType::Option(Box::new(rust_type_for_java_type_with_effective_facts(
                &JavaType::Primitive(primitive),
                &effective_facts,
            )));
        }
        let mut element_shape = shape.clone();
        element_shape.java_ty = element_ty;
        if matches!(
            element_shape.ownership,
            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
        ) && let DispatchShape::BoxedDynTrait { trait_name } = &element_shape.dispatch
        {
            // `Box<dyn Trait>` is the owned storage carrier, not the borrowed
            // method-boundary ABI.  In particular, a nullable borrowed Java
            // interface is `Option<&dyn Trait>` rather than
            // `Option<&Box<dyn Trait>>`.  Normalize the optional payload here
            // so signature rendering and argument coercion consume the same
            // trait view even when representation planning supplied the
            // escaping/storage dispatch.
            element_shape.dispatch = DispatchShape::DynTrait {
                trait_name: trait_name.clone(),
            };
            element_shape.view = ViewShape::DynTraitView;
        }
        // An owned Rust `Option` must store the array payload as `Vec<T>`;
        // storing the slice itself would produce the unsized `Option<[T]>`.
        // Borrowed slots are different: their payload is the sized reference,
        // so retain the view and render `Option<&[T]>`/`Option<&mut [T]>`.
        if matches!(element_shape.java_ty, JavaType::Array(_))
            && element_shape.view == ViewShape::Slice
            && matches!(
                element_shape.ownership,
                OwnershipShape::Owned | OwnershipShape::CopyValue
            )
        {
            element_shape.view = ViewShape::Native;
            element_shape.ownership = OwnershipShape::Owned;
        }
        // Likewise, `Option` stores a string value rather than a borrowed
        // `str` view. Rendering the inherited view here would produce the
        // unsized `Option<str>` for nullable Java `String` locals/returns.
        element_shape.normalize_owned_string_payload_view();
        // Through the wrapping entry point: the element is where the handle
        // belongs, giving `Option<Rc<T>>`.
        return RustType::Option(Box::new(
            rust_type_for_slot_with_effective_facts_and_representations(
                &element_shape,
                facts,
                effective_facts,
                representations,
            ),
        ));
    }

    if shape.view == ViewShape::Native
        && matches!(shape.dispatch, DispatchShape::Concrete)
        && crate::stdlib::native_collection_receiver_kind_for_type(&shape.java_ty).is_some()
    {
        // A collection's outer slot can remain concrete while its generic
        // arguments require a source hierarchy carrier, trait object, or
        // erased representation. Route the whole type through the canonical
        // representation-aware collection renderer before the facts-only slot
        // fallback can freeze those arguments as nominal Java classes.
        let viewed = rust_type_for_java_type_with_effective_facts_and_representations(
            &shape.java_ty,
            effective_facts,
            representations,
        );
        return match shape.ownership {
            OwnershipShape::Owned | OwnershipShape::CopyValue => viewed,
            OwnershipShape::SharedBorrow => RustType::Ref {
                mutable: false,
                lifetime: None,
                inner: Box::new(viewed),
            },
            OwnershipShape::MutableBorrow => RustType::Ref {
                mutable: true,
                lifetime: None,
                inner: Box::new(viewed),
            },
        };
    }

    let viewed = match (&shape.view, &shape.dispatch) {
        (ViewShape::JavaSemanticHash, _) => {
            let base = rust_type_for_java_type_with_effective_facts_and_representations(
                &shape.java_ty,
                effective_facts,
                representations,
            );
            let semantic = semantic_hash_collection_base(&shape.java_ty)
                .expect("semantic hash view requires a native hash collection");
            crate::item_assembly::java_semantic_hash_collection_field_rust_type(base, semantic)
                .expect("semantic hash view requires a collection carrier")
        }
        (ViewShape::Str, _) => return rust_type_for_slot_with_facts(shape, facts),
        (ViewShape::Slice, _) if java_array_rank(&shape.java_ty) >= 3 => {
            let element_ty = crate::types::sequence_element_type(&shape.java_ty)
                .expect("rank-three array has an outer element type");
            RustType::Slice(Box::new(
                rust_type_for_nested_higher_rank_array_with_representations(
                    &element_ty,
                    &effective_facts,
                    representations,
                ),
            ))
        }
        (ViewShape::Slice, _)
            if crate::types::ndarray_array2_element_type(&shape.java_ty).is_some() =>
        {
            // Render the ndarray value carrier here; the ownership match below
            // is the single place that wraps it in `&`/`&mut`. Calling the
            // slot renderer recursively would apply ownership twice and emit
            // `&&Array2<T>` or `&mut &mut Array2<T>`.
            rust_type_for_java_type_with_effective_facts_and_representations(
                &shape.java_ty,
                effective_facts,
                representations,
            )
        }
        (ViewShape::Slice, _) => {
            let content = slot_content_capability_policy_with_effective_facts(
                &shape.java_ty,
                &effective_facts,
            );
            if let Some(element_ty) = content.into_sequence_element() {
                let element_ty = rust_storage_sequence_element_java_type(
                    &shape.java_ty,
                    element_ty,
                    &effective_facts,
                );
                RustType::Slice(Box::new(
                    rust_type_for_java_generic_arg_with_representations(
                        &element_ty,
                        &effective_facts,
                        representations,
                    ),
                ))
            } else {
                rust_type_for_slot_with_facts(shape, facts)
            }
        }
        (_, _) if crate::javalibs::facade_sequence_element_type(&shape.java_ty).is_some() => {
            // A typed external iterator is a native sequence carrier even if
            // its underlying external object policy is erased.  Keeping the
            // nominal facade here would lose both iteration and its generic
            // element contract.
            rust_type_for_java_type_with_effective_facts_and_representations(
                &shape.java_ty,
                effective_facts,
                representations,
            )
        }
        (_, _)
            if matches!(
                &shape.java_ty,
                JavaType::Named { name, args }
                    if crate::stdlib::external_facade_generic_carrier_arity(name)
                        == Some(args.len())
            ) =>
        {
            // A generated generic facade is itself the executable carrier.
            // Conservative hierarchy dispatch may classify the external Java
            // type as erased, but erasing here would discard the facade's
            // payload parameter (for example `Future<T>`) and make its methods
            // disagree with collection storage using the same Java type.
            rust_type_for_java_type_with_effective_facts_and_representations(
                &shape.java_ty,
                effective_facts,
                representations,
            )
        }
        (_, DispatchShape::Concrete) => {
            if (external_final_facade_should_keep_named_type(&shape.java_ty, &effective_facts)
                || external_model_concrete_dispatch_should_keep_named_type(
                    &shape.java_ty,
                    &effective_facts,
                    representations,
                ))
                && let JavaType::Named { name, .. } = &shape.java_ty
            {
                RustType::Named(rust_name_for_java_named_type(name))
            } else if matches!(
                &shape.java_ty,
                JavaType::Named { name, .. } | JavaType::Unknown(name)
                    if crate::stdlib::is_java_platform_value_carrier_type_name(name)
            ) && !effective_facts.has_parsed_source_definition(&shape.java_ty)
            {
                // A concrete platform-facade constraint is authoritative: it
                // was added only after source-name analysis established that
                // the declaration refers to the modeled Java carrier (or a
                // same-named source declaration, which must remain nominal as
                // well). Re-projecting it through the corpus-wide hierarchy
                // plan can erase an InputStream/OutputStream slot to Any even
                // though its generated body calls the facade's methods.
                rust_type_for_java_type_with_effective_facts(&shape.java_ty, &effective_facts)
            } else if crate::stdlib::native_collection_receiver_kind_for_type(&shape.java_ty)
                .is_some()
                || matches!(shape.java_ty, JavaType::Array(_))
            {
                rust_type_for_java_type_with_effective_facts_and_representations(
                    &shape.java_ty,
                    effective_facts,
                    representations,
                )
            } else {
                // Signature planning starts conservatively, so a source
                // carrier can still arrive here as a concrete slot.  Render
                // through the representation map rather than leaking its
                // nominal Java payload type into a carrier-typed signature.
                rust_type_for_java_type_with_effective_facts_and_representations(
                    &shape.java_ty,
                    effective_facts,
                    representations,
                )
            }
        }
        (_, DispatchShape::ReferenceDynTrait { trait_name, sync }) => {
            rust_reference_dyn_trait_type(
                parameterized_dyn_trait_path(
                    trait_name,
                    &shape.java_ty,
                    &effective_facts,
                    representations,
                ),
                *sync,
            )
        }
        (_, DispatchShape::BoxedDynTrait { trait_name }) if matches!(nominal_dispatch_type(&shape.java_ty), JavaType::Named { args, .. } if !args.is_empty()) => {
            rust_boxed_dyn_trait_type(parameterized_dyn_trait_path(
                trait_name,
                &shape.java_ty,
                &effective_facts,
                representations,
            ))
        }
        (_, DispatchShape::DynTrait { trait_name }) if matches!(nominal_dispatch_type(&shape.java_ty), JavaType::Named { args, .. } if !args.is_empty()) => {
            RustType::DynTrait(parameterized_dyn_trait_path(
                trait_name,
                &shape.java_ty,
                &effective_facts,
                representations,
            ))
        }
        (_, DispatchShape::ClosedEnum { enum_name }) => RustType::Named(
            parameterized_closed_enum_type_name_with_effective_representations(
                enum_name,
                &shape.java_ty,
                &effective_facts,
                representations,
            ),
        ),
        _ => render_slot_value_rust_type_policy_with_effective_facts(
            &slot_value_rust_type_policy_with_effective_facts(shape, &effective_facts),
            &effective_facts,
        ),
    };

    let viewed =
        borrowed_external_dyn_trait_facade_view_type(shape, &effective_facts).unwrap_or(viewed);

    match shape.ownership {
        OwnershipShape::Owned | OwnershipShape::CopyValue => viewed,
        OwnershipShape::SharedBorrow => RustType::Ref {
            mutable: false,
            lifetime: None,
            inner: Box::new(viewed),
        },
        OwnershipShape::MutableBorrow => RustType::Ref {
            mutable: true,
            lifetime: None,
            inner: Box::new(viewed),
        },
    }
}

pub fn rust_type_for_slot_with_effective_facts(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> RustType {
    if let Some(rendered) = synchronized_functional_slot_rust_type(shape, facts, |local| {
        rust_type_for_slot_with_effective_facts(local, facts)
    }) {
        return rendered;
    }
    if let Some(rendered) = carrier_borrow_slot_rust_type(shape, |owned| {
        rust_type_for_slot_with_effective_facts(owned, facts)
    }) {
        return rendered;
    }
    if let Some(rendered) = list_interface_slot_rust_type(shape, facts, None) {
        return rendered;
    }
    if !shape.nested_native_allocations.is_empty() {
        let mut outer = shape.clone();
        outer.nested_native_allocations.clear();
        let rendered = rust_type_for_slot_with_effective_facts(&outer, facts);
        return project_nested_native_allocations(shape, facts, rendered);
    }
    if let Some(rendered) = native_allocation_slot_rust_type(shape, facts, |payload| {
        rust_type_for_slot_with_effective_facts(payload, facts)
    }) {
        return rendered;
    }
    let scoped_facts = (*facts).with_nested_collection_views(&shape.nested_collection_views);
    let facts = &scoped_facts;
    if let Some(element_ty) = shape.optional_element_type() {
        if let JavaType::Named { name, args } = &element_ty
            && args.is_empty()
            && let Some(primitive) = boxed_primitive_storage_java_type(name, facts)
        {
            return RustType::Option(Box::new(rust_type_for_java_type_with_effective_facts(
                &JavaType::Primitive(primitive),
                facts,
            )));
        }
        let mut element_shape = shape.clone();
        element_shape.java_ty = element_ty;
        element_shape.normalize_owned_string_payload_view();
        return RustType::Option(Box::new(rust_type_for_slot_with_effective_facts(
            &element_shape,
            facts,
        )));
    }
    if shape.view == ViewShape::JavaSemanticHash {
        let base = rust_type_for_java_type_with_effective_facts(&shape.java_ty, facts);
        let semantic = semantic_hash_collection_base(&shape.java_ty)
            .expect("semantic hash view requires a native hash collection");
        let rendered =
            crate::item_assembly::java_semantic_hash_collection_field_rust_type(base, semantic)
                .expect("semantic hash view requires a collection carrier");
        return match shape.ownership {
            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow => RustType::Ref {
                mutable: shape.ownership == OwnershipShape::MutableBorrow,
                lifetime: None,
                inner: Box::new(rendered),
            },
            _ => rendered,
        };
    }
    if crate::javalibs::facade_sequence_element_type(&shape.java_ty).is_some() {
        let rendered = rust_type_for_java_type_with_effective_facts(&shape.java_ty, facts);
        return match shape.ownership {
            OwnershipShape::Owned | OwnershipShape::CopyValue => rendered,
            OwnershipShape::SharedBorrow => RustType::Ref {
                mutable: false,
                lifetime: None,
                inner: Box::new(rendered),
            },
            OwnershipShape::MutableBorrow => RustType::Ref {
                mutable: true,
                lifetime: None,
                inner: Box::new(rendered),
            },
        };
    }
    let rendered =
        render_slot_rust_type_policy(&slot_rust_type_policy_with_effective_facts(shape, facts));
    shared_handle_slot_rust_type(shape, facts, rendered)
}

/// Native SAM facts prove platform identity before canonicalizing its Rust
/// name. Qualified declarations and captures must use the imported carrier.
fn native_functional_rust_type(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
    render_argument: impl FnMut(&JavaType) -> RustType,
) -> Option<RustType> {
    facts.native_functional_carrier(java_ty)?;
    let (name, args) = match java_ty {
        JavaType::Named { name, args } => (name, args.as_slice()),
        JavaType::Unknown(name) => (name, &[][..]),
        _ => return None,
    };
    let name = rust_name_for_java_named_type(crate::stdlib::native_functional_owner(
        &crate::stdlib::canonical_external_receiver_type_key(name),
    ));
    Some(if args.is_empty() {
        RustType::Named(name)
    } else {
        rust_generic_type(name, args.iter().map(render_argument).collect())
    })
}

pub(crate) fn functional_slot_carrier_name(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> Option<String> {
    let ty = shape
        .optional_element_type()
        .unwrap_or_else(|| shape.java_ty.clone());
    facts.native_functional_carrier(&ty)?;
    let name = match &ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        _ => return None,
    };
    let local = rust_name_for_java_named_type(crate::stdlib::native_functional_owner(
        &crate::stdlib::canonical_external_receiver_type_key(name),
    ));
    Some(if shape.native_allocation.sync() {
        crate::stdlib::synchronized_functional_carrier_name(&local)
    } else {
        local
    })
}

fn synchronized_functional_slot_rust_type(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
    render_local: impl FnOnce(&RustSlotShape) -> RustType,
) -> Option<RustType> {
    if !shape.native_allocation.sync() {
        return None;
    }
    let synchronized = functional_slot_carrier_name(shape, facts)?;
    let mut local = shape.clone();
    local.native_allocation = StoragePlacement::default();
    let local_name = functional_slot_carrier_name(&local, facts)?;
    fn project(ty: RustType, local: &str, synchronized: &str) -> RustType {
        match ty {
            RustType::Named(name) if name == local => RustType::Named(synchronized.into()),
            RustType::Generic { base, args } if base == local => RustType::Generic {
                base: synchronized.into(),
                args,
            },
            RustType::Option(inner) => {
                RustType::Option(Box::new(project(*inner, local, synchronized)))
            }
            RustType::Ref {
                mutable,
                lifetime,
                inner,
            } => RustType::Ref {
                mutable,
                lifetime,
                inner: Box::new(project(*inner, local, synchronized)),
            },
            other => other,
        }
    }
    Some(project(render_local(&local), &local_name, &synchronized))
}

pub(crate) fn semantic_hash_collection_base(ty: &JavaType) -> Option<&'static str> {
    if let Some(element) = crate::types::optional_element_type(ty) {
        return semantic_hash_collection_base(&element);
    }
    match crate::stdlib::java_collection_kind_for_type(ty)? {
        crate::stdlib::JavaCollectionKind::Set => Some("JavaSemanticHashSet"),
        crate::stdlib::JavaCollectionKind::Map => Some("JavaSemanticHashMap"),
        _ => None,
    }
}

/// Overlay solved physical child slots on the canonical rendered payload. Keep
/// its nullability, dispatch and equality carriers instead of rebuilding child
/// types from Java spellings without representation context.
fn project_nested_native_allocations(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
    rendered: RustType,
) -> RustType {
    fn wrap_child(
        shape: &RustSlotShape,
        facts: &EffectiveFacts<'_>,
        rendered: RustType,
    ) -> RustType {
        if let RustType::Option(inner) = rendered {
            let mut element = shape.clone();
            element.java_ty = shape
                .optional_element_type()
                .unwrap_or_else(|| shape.java_ty.clone());
            return RustType::Option(Box::new(wrap_child(&element, facts, *inner)));
        }
        synchronized_functional_slot_rust_type(shape, facts, |_| rendered.clone())
            .or_else(|| native_allocation_slot_rust_type(shape, facts, |_| rendered.clone()))
            .unwrap_or(rendered)
    }
    if shape.nested_native_allocations.is_empty() {
        return rendered;
    }
    let java_ty = shape
        .optional_element_type()
        .unwrap_or_else(|| shape.java_ty.clone());
    // Optional/reference/storage wrappers are transparent to child positions.
    match rendered {
        RustType::Option(inner) => {
            let mut element = shape.clone();
            element.java_ty = java_ty;
            return RustType::Option(Box::new(project_nested_native_allocations(
                &element, facts, *inner,
            )));
        }
        RustType::Ref {
            mutable,
            lifetime,
            inner,
        } => {
            return RustType::Ref {
                mutable,
                lifetime,
                inner: Box::new(project_nested_native_allocations(shape, facts, *inner)),
            };
        }
        RustType::Generic { ref base, .. }
            if matches!(
                base.as_str(),
                "std::rc::Rc" | "std::sync::Arc" | "std::cell::RefCell" | "std::sync::Mutex"
            ) =>
        {
            let RustType::Generic { base, args } = rendered else {
                unreachable!()
            };
            return RustType::Generic {
                base,
                args: args
                    .into_iter()
                    .map(|inner| project_nested_native_allocations(shape, facts, inner))
                    .collect(),
            };
        }
        _ => {}
    }
    if facts.parsed_source_controls_native_carrier(&java_ty) {
        return rendered;
    }
    let children = match &java_ty {
        JavaType::Array(element) => vec![(**element).clone()],
        JavaType::Named { args, .. }
            if crate::stdlib::java_collection_kind_for_type(&java_ty).is_some()
                || facts.native_iterable_element_type(&java_ty).is_some()
                || facts.native_list_cursor_element_type(&java_ty).is_some()
                || facts.native_functional_carrier(&java_ty).is_some()
                || facts.native_map_entry_type_args(&java_ty).is_some() =>
        {
            args.clone()
        }
        _ => return rendered,
    };
    let project = |index: usize, child: RustType| {
        let Some(java_child) = children.get(index) else {
            return child;
        };
        let mut slot = RustSlotShape::for_storage(
            crate::types::generic_read_type_preserving_optional(java_child),
        );
        slot.inherit_native_collection_child(shape, index);
        let child = project_nested_native_allocations(&slot, facts, child);
        wrap_child(&slot, facts, child)
    };
    match rendered {
        RustType::Tuple(items) if items.len() == children.len() => RustType::Tuple(
            items
                .into_iter()
                .enumerate()
                .map(|(index, child)| project(index, child))
                .collect(),
        ),
        RustType::Vec(element) => RustType::Vec(Box::new(project(0, *element))),
        RustType::Slice(element) => RustType::Slice(Box::new(project(0, *element))),
        RustType::Generic { base, args } if args.len() == children.len() => RustType::Generic {
            base,
            args: args
                .into_iter()
                .enumerate()
                .map(|(index, child)| project(index, child))
                .collect(),
        },
        other => other,
    }
}

/// A native allocation's handle is its physical value, not a view of its
/// payload. In particular, a borrowed slot is `&Rc<RefCell<Vec<T>>>`, not a
/// long-lived `&[T]` which would keep a borrow active during callback writes.
/// Root allocation wrappers exist only for native array/container payloads.
/// Generic qualifier markers on scalar/source values do not add such a layer.
pub(crate) fn native_allocation_has_outer_carrier(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> bool {
    let base = optional_element_type(java_ty).unwrap_or_else(|| java_ty.clone());
    !facts.parsed_source_controls_native_carrier(&base)
        && (matches!(base, JavaType::Array(_))
            || crate::stdlib::java_collection_kind_for_type(&base).is_some())
}

fn native_allocation_slot_rust_type(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
    render_payload: impl FnOnce(&RustSlotShape) -> RustType,
) -> Option<RustType> {
    if !shape.native_allocation.shared()
        || shape.optional_element_type().is_some()
        || !native_allocation_has_outer_carrier(&shape.java_ty, facts)
    {
        return None;
    }
    let mut payload = shape.clone();
    payload.native_allocation = StoragePlacement::default();
    payload.ownership = OwnershipShape::Owned;
    if payload.view == ViewShape::Slice {
        payload.view = ViewShape::Native;
    }
    let rendered_payload = render_payload(&payload);
    let payload_type = if facts.retained_native_map_kind(&payload.java_ty).is_some() {
        let RustType::Generic { args, .. } = rendered_payload else {
            return None;
        };
        if args.len() != 2 {
            return None;
        }
        // The allocation, not a Map-typed alias, owns iteration/equality
        // policy. Hash/linked native maps share a physical reference ABI.
        rust_generic_type("JavaSemanticHashMap", args)
    } else if facts
        .retained_native_collection_kind(&payload.java_ty)
        .is_some()
    {
        let element = match rendered_payload {
            RustType::Vec(element) => *element,
            RustType::Generic { mut args, .. } if args.len() == 1 => args.remove(0),
            _ => return None,
        };
        rust_generic_type(
            retained_native_collection_runtime_name(shape.native_allocation),
            vec![element],
        )
    } else {
        rendered_payload
    };
    let rendered = storage_placement_type(shape.native_allocation, payload_type);
    Some(match shape.ownership {
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow => RustType::Ref {
            mutable: shape.ownership == OwnershipShape::MutableBorrow,
            lifetime: None,
            inner: Box::new(rendered),
        },
        _ => rendered,
    })
}

pub(crate) fn retained_native_collection_runtime_name(placement: StoragePlacement) -> &'static str {
    if placement.sync() {
        "JavaSyncNativeCollection"
    } else {
        "JavaNativeCollection"
    }
}

/// Wraps an **owned** slot of a shared type in its handle (0002/0002b).
fn list_interface_slot_rust_type(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
    source_context: Option<(&FactSet, &RepresentationPlan)>,
) -> Option<RustType> {
    if shape.view != ViewShape::ListInterface {
        return None;
    }
    let optional = shape.optional_element_type();
    let base = optional.as_ref().unwrap_or(&shape.java_ty);
    let element = crate::types::sequence_element_type(base)?;
    let mut child = source_context.map_or_else(
        || RustSlotShape::for_storage(element.clone()),
        |(_, representations)| {
            crate::shape_projection::storage_shape_for_java_type(representations, element.clone())
        },
    );
    child.inherit_native_collection_child(shape, 0);
    let element = source_context.map_or_else(
        || rust_type_for_slot_with_effective_facts(&child, facts),
        |(base_facts, representations)| {
            rust_type_for_slot_with_facts_and_representations(&child, base_facts, representations)
        },
    );
    let mut ty = rust_generic_type("JavaListView", vec![element]);
    if matches!(
        shape.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) {
        ty = RustType::Ref {
            mutable: false,
            lifetime: None,
            inner: Box::new(ty),
        };
    }
    Some(if optional.is_some() {
        RustType::Option(Box::new(ty))
    } else {
        ty
    })
}

fn carrier_borrow_slot_rust_type(
    shape: &RustSlotShape,
    render_owned: impl FnOnce(&RustSlotShape) -> RustType,
) -> Option<RustType> {
    if shape.borrow_target != crate::slot_shape::BorrowTarget::Carrier
        || !matches!(
            shape.ownership,
            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
        )
    {
        return None;
    }
    let optional = shape.optional_element_type();
    let mut owned = shape.clone();
    owned.borrow_target = crate::slot_shape::BorrowTarget::Payload;
    owned.ownership = OwnershipShape::Owned;
    if let Some(element) = optional.as_ref() {
        owned.java_ty = element.clone();
    }
    let borrowed = RustType::Ref {
        mutable: shape.ownership == OwnershipShape::MutableBorrow,
        lifetime: None,
        inner: Box::new(render_owned(&owned)),
    };
    Some(if optional.is_some() {
        RustType::Option(Box::new(borrowed))
    } else {
        borrowed
    })
}

fn shared_handle_slot_rust_type(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
    rendered: RustType,
) -> RustType {
    if !matches!(
        shape.ownership,
        OwnershipShape::Owned | OwnershipShape::CopyValue
    ) || shape.optional_element_type().is_some()
        || matches!(
            shape.dispatch,
            DispatchShape::ErasedObject | DispatchShape::ClosedEnum { .. }
        )
    {
        // An optional slot is wrapped at its element by the recursion above;
        // wrapping here too would give `Rc<Option<T>>`.
        return rendered;
    }
    let JavaType::Named { name, .. } = &shape.java_ty else {
        return rendered;
    };
    let placement = facts.storage_placement_for(name);
    if !placement.is_implemented() || !placement.shared() {
        return rendered;
    }
    storage_placement_type(placement, rendered)
}

fn shared_handle_slot_rust_type_with_representations(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
    rendered: RustType,
) -> RustType {
    if !matches!(
        shape.ownership,
        OwnershipShape::Owned | OwnershipShape::CopyValue
    ) || shape.optional_element_type().is_some()
        || matches!(
            shape.dispatch,
            DispatchShape::ErasedObject | DispatchShape::ClosedEnum { .. }
        )
        || slot_uses_value_carrier_representation(shape, representations)
    {
        return rendered;
    }
    let JavaType::Named { name, .. } = &shape.java_ty else {
        return rendered;
    };
    let placement = crate::storage_strategy::effective_storage_placement(
        facts.storage_placement_for(name),
        representations.storage_placement_for(name),
    );
    if !placement.is_implemented() || !placement.shared() {
        return rendered;
    }
    storage_placement_type(placement, rendered)
}

/// A closed/source-hierarchy carrier is itself the owning Java-reference
/// representation; its variants already contain the payload handles. Do not
/// wrap that carrier in a second Rc/Arc merely because a conservative slot
/// arrived with `Concrete` dispatch.
fn slot_uses_value_carrier_representation(
    shape: &RustSlotShape,
    representations: &RepresentationPlan,
) -> bool {
    let JavaType::Named { name, .. } = &shape.java_ty else {
        return false;
    };
    representations
        .type_representation(name)
        .is_some_and(|entry| {
            matches!(
                entry.representation(),
                crate::representation::ObjectRepresentation::ClosedEnum { .. }
                    | crate::representation::ObjectRepresentation::SourceHierarchyCarrier { .. }
            )
        })
}

pub fn slot_rust_type_policy_with_effective_facts(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> SlotRustTypePolicy {
    let viewed = borrowed_external_dyn_trait_facade_view_type(shape, facts).unwrap_or_else(|| {
        render_slot_value_rust_type_policy_with_effective_facts(
            &slot_value_rust_type_policy_with_effective_facts(shape, facts),
            facts,
        )
    });

    match shape.ownership {
        OwnershipShape::Owned | OwnershipShape::CopyValue => {
            SlotRustTypePolicy::OwnedOrCopy { ty: viewed }
        }
        OwnershipShape::SharedBorrow => SlotRustTypePolicy::SharedBorrow { ty: viewed },
        OwnershipShape::MutableBorrow => SlotRustTypePolicy::MutableBorrow { ty: viewed },
    }
}

fn borrowed_external_dyn_trait_facade_view_type(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> Option<RustType> {
    if !matches!(
        shape.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) {
        return None;
    }
    let JavaType::Named { name, .. } = &shape.java_ty else {
        return None;
    };
    external_crate_mapped_dyn_trait_path(name, facts).map(RustType::ImplTrait)
}

pub fn slot_value_rust_type_policy_with_effective_facts(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> SlotValueRustTypePolicy {
    let content = slot_content_capability_policy_with_effective_facts(&shape.java_ty, facts);
    match (&shape.view, &shape.dispatch, &shape.java_ty) {
        (ViewShape::DynTraitView, DispatchShape::DynTrait { trait_name }, _) => {
            SlotValueRustTypePolicy::DynTraitView {
                trait_name: trait_name.clone(),
                java_ty: shape.java_ty.clone(),
            }
        }
        (ViewShape::DynTraitView, DispatchShape::BoxedDynTrait { trait_name }, _) => {
            SlotValueRustTypePolicy::BoxedDynTraitView {
                trait_name: trait_name.clone(),
                java_ty: shape.java_ty.clone(),
            }
        }
        (ViewShape::Str, _, JavaType::String)
            if facts.uses_java_string_identity_representation() =>
        {
            SlotValueRustTypePolicy::StringReferenceView
        }
        (ViewShape::Str, _, _) if content.string_content() => SlotValueRustTypePolicy::StringView,
        (ViewShape::Slice, _, java_ty)
            if crate::types::ndarray_array2_element_type(java_ty).is_some() =>
        {
            slot_dispatch_rust_type_policy(shape)
        }
        (ViewShape::Slice, _, _) => {
            if let Some(element_ty) = content.into_sequence_element() {
                SlotValueRustTypePolicy::SliceView {
                    element_ty: rust_storage_sequence_element_java_type(
                        &shape.java_ty,
                        element_ty,
                        facts,
                    ),
                    nested_array: java_array_rank(&shape.java_ty) >= 3,
                }
            } else {
                slot_dispatch_rust_type_policy(shape)
            }
        }
        (_, DispatchShape::ErasedObject, java_ty)
            if crate::javalibs::generic_facade_interface_erases_to_named_rust_type(java_ty)
                || crate::javalibs::facade_sequence_element_type(java_ty).is_some() =>
        {
            SlotValueRustTypePolicy::ConcreteDispatch {
                java_ty: java_ty.clone(),
            }
        }
        _ => slot_dispatch_rust_type_policy(shape),
    }
}

pub(crate) fn render_slot_rust_type_policy(policy: &SlotRustTypePolicy) -> RustType {
    match policy {
        SlotRustTypePolicy::OwnedOrCopy { ty } => ty.clone(),
        SlotRustTypePolicy::SharedBorrow { ty } => RustType::Ref {
            mutable: false,
            lifetime: None,
            inner: Box::new(ty.clone()),
        },
        SlotRustTypePolicy::MutableBorrow { ty } => RustType::Ref {
            mutable: true,
            lifetime: None,
            inner: Box::new(ty.clone()),
        },
    }
}

fn slot_dispatch_rust_type_policy(shape: &RustSlotShape) -> SlotValueRustTypePolicy {
    match &shape.dispatch {
        DispatchShape::ClosedEnum { enum_name } => SlotValueRustTypePolicy::ClosedEnum {
            enum_name: parameterized_closed_enum_type_name(enum_name, &shape.java_ty),
        },
        DispatchShape::ReferenceDynTrait { trait_name, sync } => {
            SlotValueRustTypePolicy::ReferenceDynTrait {
                trait_name: trait_name.clone(),
                java_ty: shape.java_ty.clone(),
                sync: *sync,
            }
        }
        DispatchShape::BoxedDynTrait { trait_name } => SlotValueRustTypePolicy::BoxedDynTrait {
            trait_name: trait_name.clone(),
            java_ty: shape.java_ty.clone(),
        },
        DispatchShape::ErasedObject => SlotValueRustTypePolicy::ErasedObject,
        DispatchShape::DynTrait { trait_name } => SlotValueRustTypePolicy::DynTrait {
            trait_name: trait_name.clone(),
            java_ty: shape.java_ty.clone(),
        },
        DispatchShape::Concrete => SlotValueRustTypePolicy::ConcreteDispatch {
            java_ty: shape.java_ty.clone(),
        },
    }
}

pub(crate) fn render_slot_value_rust_type_policy_with_effective_facts(
    policy: &SlotValueRustTypePolicy,
    facts: &EffectiveFacts<'_>,
) -> RustType {
    match policy {
        SlotValueRustTypePolicy::ClosedEnum { enum_name } => RustType::Named(enum_name.clone()),
        SlotValueRustTypePolicy::BoxedDynTrait {
            trait_name,
            java_ty,
        }
        | SlotValueRustTypePolicy::BoxedDynTraitView {
            trait_name,
            java_ty,
        } => {
            if external_crate_mapped_dyn_trait_path(trait_name, facts).is_some() {
                return erased_object_rust_type();
            }
            rust_boxed_dyn_trait_type(parameterized_dyn_trait_path_with_effective_facts(
                trait_name, java_ty, facts,
            ))
        }
        SlotValueRustTypePolicy::ReferenceDynTrait {
            trait_name,
            java_ty,
            sync,
        } => rust_reference_dyn_trait_type(
            parameterized_dyn_trait_path_with_effective_facts(trait_name, java_ty, facts),
            *sync,
        ),
        SlotValueRustTypePolicy::ErasedObject => erased_object_rust_type(),
        SlotValueRustTypePolicy::DynTrait {
            trait_name,
            java_ty,
        }
        | SlotValueRustTypePolicy::DynTraitView {
            trait_name,
            java_ty,
        } => {
            if let Some(trait_path) = external_crate_mapped_dyn_trait_path(trait_name, facts) {
                return RustType::ImplTrait(trait_path);
            }
            RustType::DynTrait(parameterized_dyn_trait_path_with_effective_facts(
                trait_name, java_ty, facts,
            ))
        }
        SlotValueRustTypePolicy::ConcreteDispatch { java_ty } => {
            if crate::javalibs::facade_sequence_element_type(java_ty).is_some() {
                rust_type_for_java_type_with_effective_facts(java_ty, facts)
            } else if let JavaType::Named { name, args } = java_ty
                && let Some(ty) = external_crate_mapped_named_rust_type(name, args, facts)
            {
                ty
            } else if let JavaType::Unknown(name) = java_ty
                && let Some(ty) = external_crate_mapped_named_rust_type(name, &[], facts)
            {
                ty
            } else if facts.native_functional_carrier(java_ty).is_some() {
                // Executable native SAM facades retain their instantiated
                // arguments. The legacy opaque-interface fallback below
                // erases them to the facade's default Object parameters.
                rust_type_for_java_type_with_effective_facts(java_ty, facts)
            } else if erased_facade_concrete_dispatch_should_keep_named_type(java_ty, facts)
                && let JavaType::Named { name, .. } = java_ty
            {
                RustType::Named(rust_name_for_java_named_type(name))
            } else if external_final_facade_should_keep_named_type(java_ty, facts)
                && let JavaType::Named { name, .. } = java_ty
            {
                RustType::Named(rust_name_for_java_named_type(name))
            } else if crate::javalibs::generic_facade_interface_erases_to_named_rust_type(java_ty)
                && let JavaType::Named { name, .. } = java_ty
            {
                RustType::Named(rust_name_for_java_named_type(name))
            } else {
                rust_type_for_java_type_with_effective_facts(java_ty, facts)
            }
        }
        SlotValueRustTypePolicy::StringView => RustType::Str,
        SlotValueRustTypePolicy::StringReferenceView => RustType::Named("JavaString".to_string()),
        SlotValueRustTypePolicy::SliceView {
            element_ty,
            nested_array,
        } => RustType::Slice(Box::new(if *nested_array {
            rust_type_for_nested_higher_rank_array(element_ty, facts)
        } else {
            rust_type_for_java_generic_arg_with_effective_facts(element_ty, facts)
        })),
    }
}

fn external_crate_mapped_dyn_trait_path(
    trait_name: &str,
    facts: &EffectiveFacts<'_>,
) -> Option<String> {
    let java_ty = JavaType::Named {
        name: trait_name.to_string(),
        args: Vec::new(),
    };
    let Some(HierarchyStrategy::DynTrait {
        trait_name: fact_trait_name,
    }) = facts.object_representation_override_strategy_for(&java_ty)
    else {
        return None;
    };
    let type_key = crate::stdlib::canonical_external_receiver_type_key(trait_name);
    if let Some(pack) = facts.external_library_pack_for_type(&type_key)
        && pack.mode == ExternalLibraryMode::RustCrateMapping
        && let Some(crate_name) = pack.rust_target.as_deref()
    {
        return Some(format!("{crate_name}::{fact_trait_name}"));
    }
    None
}

fn dyn_trait_rust_path_with_effective_facts(
    trait_name: &str,
    facts: &EffectiveFacts<'_>,
) -> String {
    let java_ty = JavaType::Named {
        name: trait_name.to_string(),
        args: Vec::new(),
    };
    if let Some(HierarchyStrategy::DynTrait {
        trait_name: fact_trait_name,
    }) = facts.object_representation_override_strategy_for(&java_ty)
    {
        let type_key = crate::stdlib::canonical_external_receiver_type_key(trait_name);
        if let Some(pack) = facts.external_library_pack_for_type(&type_key)
            && pack.mode == ExternalLibraryMode::RustCrateMapping
            && let Some(crate_name) = pack.rust_target.as_deref()
        {
            return format!("{crate_name}::{fact_trait_name}");
        }
        return rust_trait_name_path(fact_trait_name);
    }
    rust_trait_name_path(trait_name)
}

pub(crate) fn dyn_trait_rust_path(trait_name: &str, facts: &FactSet) -> String {
    let effective_facts = EffectiveFacts::new(facts);
    dyn_trait_rust_path_with_effective_facts(trait_name, &effective_facts)
}

fn rust_trait_name_path(trait_name: &str) -> String {
    if trait_name.contains("::") {
        trait_name.to_string()
    } else {
        rust_name_for_java_named_type(trait_name)
    }
}

pub fn default_expr_for_slot_with_facts(shape: &RustSlotShape, facts: &FactSet) -> RustExpr {
    let effective_facts = EffectiveFacts::new(facts);
    default_expr_for_slot_with_effective_facts(shape, &effective_facts)
}

pub fn default_expr_for_slot_with_effective_facts(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> RustExpr {
    let rendered =
        slot_default_policy_expr(&slot_default_policy_with_effective_facts(shape, facts));
    if shape.native_allocation.shared()
        && shape.optional_element_type().is_none()
        && matches!(
            shape.ownership,
            OwnershipShape::Owned | OwnershipShape::CopyValue
        )
    {
        return storage_placement_construction_expr(shape.native_allocation, rendered);
    }
    shared_handle_construction_expr(shape, facts, rendered)
}

/// Value-form counterpart of the type wrap, for **fresh constructions** only.
pub(crate) fn shared_handle_construction_expr(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
    rendered: RustExpr,
) -> RustExpr {
    if !matches!(
        shape.ownership,
        OwnershipShape::Owned | OwnershipShape::CopyValue
    ) || shape.optional_element_type().is_some()
    {
        return rendered;
    }
    let JavaType::Named { name, .. } = &shape.java_ty else {
        return rendered;
    };
    let placement = facts.storage_placement_for(name);
    if !placement.is_implemented() || !placement.shared() {
        return rendered;
    }
    storage_placement_construction_expr(placement, rendered)
}

/// Fresh construction for a representation-planned slot. A source hierarchy
/// carrier may be shared by the representation plan even when ordinary facts
/// do not mark the Java base class as shared.
pub(crate) fn shared_handle_construction_expr_with_representations(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
    rendered: RustExpr,
) -> RustExpr {
    if !matches!(
        shape.ownership,
        OwnershipShape::Owned | OwnershipShape::CopyValue
    ) || shape.optional_element_type().is_some()
        || slot_uses_value_carrier_representation(shape, representations)
    {
        return rendered;
    }
    let JavaType::Named { name, .. } = &shape.java_ty else {
        return rendered;
    };
    let placement = crate::storage_strategy::effective_storage_placement(
        facts.storage_placement_for(name),
        representations.storage_placement_for(name),
    );
    if !placement.is_implemented() || !placement.shared() {
        return rendered;
    }
    storage_placement_construction_expr(placement, rendered)
}

pub(crate) fn storage_placement_construction_expr(
    placement: StoragePlacement,
    rendered: RustExpr,
) -> RustExpr {
    let rendered = recover_storage_placement_construction_expr(placement, rendered);
    let rendered = relocate_result_projection_inside_storage(placement, rendered);
    if storage_placement_construction_expr_is_wrapped(placement, &rendered) {
        return rendered;
    }
    let payload_was_result_propagated = rendered.recorded_shape()
        == Some(RustExprShape::ResultPropagated)
        || matches!(rendered.unshaped(), RustExpr::Try(_));
    let celled = placement
        .cell_path()
        .map(|cell| storage_wrapper_new_call_expr(cell, rendered.clone()))
        .unwrap_or(rendered);
    let constructed = placement
        .handle_path()
        .map(|handle| storage_wrapper_new_call_expr(handle, celled.clone()))
        .unwrap_or(celled);
    if payload_was_result_propagated {
        RustExpr::shaped(RustExprShape::ResultPropagated, constructed)
    } else {
        constructed
    }
}

/// Shared-storage constructors are infallible adapters around their payload.
/// If a later slot boundary has put `?` around an already constructed handle,
/// move that projection back to the payload which can actually be a `Result`.
/// This also makes storage construction idempotent across expression and slot
/// lowering: `Handle(Cell(value?))?` must not become a nested handle.
fn relocate_result_projection_inside_storage(
    placement: StoragePlacement,
    expr: RustExpr,
) -> RustExpr {
    match expr {
        RustExpr::Shaped {
            shape: RustExprShape::ResultPropagated,
            actual,
            expr,
        } => {
            let RustExpr::Try(wrapped) = *expr else {
                return RustExpr::Shaped {
                    shape: RustExprShape::ResultPropagated,
                    actual,
                    expr,
                };
            };
            let Some(payload) = storage_placement_wrapped_payload(placement, (*wrapped).clone())
            else {
                return RustExpr::Shaped {
                    shape: RustExprShape::ResultPropagated,
                    actual,
                    expr: Box::new(RustExpr::Try(wrapped)),
                };
            };
            let payload = if matches!(payload.unshaped(), RustExpr::Try(_)) {
                payload
            } else {
                RustExpr::Try(Box::new(payload))
            };
            RustExpr::Shaped {
                shape: RustExprShape::ResultPropagated,
                actual,
                expr: Box::new(storage_placement_construction_expr(placement, payload)),
            }
        }
        RustExpr::Try(wrapped) => {
            let Some(payload) = storage_placement_wrapped_payload(placement, (*wrapped).clone())
            else {
                return RustExpr::Try(wrapped);
            };
            let payload = if matches!(payload.unshaped(), RustExpr::Try(_)) {
                payload
            } else {
                RustExpr::Try(Box::new(payload))
            };
            storage_placement_construction_expr(placement, payload)
        }
        other => other,
    }
}

fn storage_placement_wrapped_payload(
    placement: StoragePlacement,
    expr: RustExpr,
) -> Option<RustExpr> {
    let without_handle = match placement.handle_path() {
        Some(handle) => storage_wrapper_new_call_inner(expr, handle)?,
        None => expr,
    };
    match placement.cell_path() {
        Some(cell) => storage_wrapper_new_call_inner(without_handle, cell),
        None => Some(without_handle),
    }
}

fn storage_wrapper_new_call_inner(expr: RustExpr, wrapper: &str) -> Option<RustExpr> {
    let RustExpr::Call { function, mut args } = expr.into_unshaped() else {
        return None;
    };
    if !rust_path_expr_matches(&function, wrapper, "new") || args.len() != 1 {
        return None;
    }
    args.pop()
}

fn recover_storage_placement_construction_expr(
    placement: StoragePlacement,
    expr: RustExpr,
) -> RustExpr {
    match expr {
        RustExpr::Raw(rendered) => {
            let recovered = crate::coercion::recover_operator_expr_or_raw_leaf(&rendered);
            if storage_placement_construction_expr_is_wrapped(placement, &recovered) {
                recovered
            } else {
                RustExpr::raw_leaf(rendered)
            }
        }
        other => other,
    }
}

fn storage_placement_construction_expr_is_wrapped(
    placement: StoragePlacement,
    expr: &RustExpr,
) -> bool {
    let Some(handle) = placement.handle_path() else {
        return false;
    };
    match expr.unshaped() {
        RustExpr::Call { function, .. } => rust_path_expr_matches(function, handle, "new"),
        _ => false,
    }
}

fn rust_path_expr_matches(expr: &RustExpr, base: &str, leaf: &str) -> bool {
    match expr.unshaped() {
        RustExpr::Path(segments) => segments
            .iter()
            .map(String::as_str)
            .eq(base.split("::").chain(std::iter::once(leaf))),
        RustExpr::GenericPath {
            base: segments,
            member: Some(member),
            ..
        } if member == leaf => segments.iter().map(String::as_str).eq(base.split("::")),
        _ => false,
    }
}

fn storage_placement_type(placement: StoragePlacement, rendered: RustType) -> RustType {
    let celled = placement
        .cell_path()
        .map(|cell| rust_generic_type(cell, vec![rendered.clone()]))
        .unwrap_or(rendered);
    placement
        .handle_path()
        .map(|handle| rust_generic_type(handle, vec![celled.clone()]))
        .unwrap_or(celled)
}

fn storage_wrapper_new_call_expr(wrapper_path: &str, value: RustExpr) -> RustExpr {
    let mut segments = wrapper_path.split("::").collect::<Vec<_>>();
    segments.push("new");
    RustExpr::Call {
        function: Box::new(RustExpr::path(segments)),
        args: vec![value],
    }
}

pub fn slot_default_policy_with_effective_facts(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> SlotDefaultPolicy {
    if shape.optional_element_type().is_some() {
        return SlotDefaultPolicy::Value {
            policy: SlotDefaultValuePolicy::DefaultTrait,
        };
    }
    match &shape.dispatch {
        DispatchShape::ClosedEnum { enum_name } => {
            return SlotDefaultPolicy::NoDefaultForClosedEnum {
                enum_name: enum_name.clone(),
            };
        }
        DispatchShape::DynTrait { trait_name } => {
            return SlotDefaultPolicy::NoDefaultForDynTrait {
                trait_name: trait_name.clone(),
            };
        }
        DispatchShape::BoxedDynTrait { trait_name }
        | DispatchShape::ReferenceDynTrait { trait_name, .. } => {
            return SlotDefaultPolicy::NoDefaultForBoxedDynTrait {
                trait_name: trait_name.clone(),
            };
        }
        DispatchShape::ErasedObject => return SlotDefaultPolicy::NoDefaultForErasedObject,
        DispatchShape::Concrete => {}
    }
    SlotDefaultPolicy::Value {
        policy: slot_default_value_policy_with_effective_facts(shape, facts),
    }
}

pub fn slot_default_value_policy_with_effective_facts(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> SlotDefaultValuePolicy {
    if shape.ownership == OwnershipShape::Owned
        && shape.optional_element_type().is_none()
        && shape.native_allocation.shared()
        && facts.retained_native_collection_kind(&shape.java_ty)
            == Some(crate::stdlib::JavaCollectionKind::List)
    {
        let empty_add_changes = match &shape.java_ty {
            JavaType::Named { name, .. } | JavaType::Unknown(name) => {
                crate::stdlib::native_list_empty_add_changes(name)
            }
            _ => true,
        };
        return SlotDefaultValuePolicy::EmptyRetainedList {
            sync: shape.native_allocation.sync(),
            empty_add_changes,
        };
    }
    let content = slot_content_capability_policy_with_effective_facts(&shape.java_ty, facts);
    match (&shape.ownership, &shape.view, &shape.java_ty) {
        (OwnershipShape::SharedBorrow, ViewShape::Str, _) if content.string_content() => {
            if shape.java_ty == JavaType::String && facts.uses_java_string_identity_representation()
            {
                SlotDefaultValuePolicy::EmptyStringReferenceView
            } else {
                SlotDefaultValuePolicy::EmptyStringView
            }
        }
        (OwnershipShape::SharedBorrow, ViewShape::Slice, _) => {
            SlotDefaultValuePolicy::EmptySliceView
        }
        (_, _, JavaType::Primitive(PrimitiveType::Boolean)) => {
            SlotDefaultValuePolicy::PrimitiveBooleanZero
        }
        (_, _, JavaType::Primitive(PrimitiveType::Char)) => {
            SlotDefaultValuePolicy::PrimitiveCharZero
        }
        (_, _, JavaType::Primitive(PrimitiveType::Float)) => {
            SlotDefaultValuePolicy::PrimitiveFloatZero
        }
        (_, _, JavaType::Primitive(PrimitiveType::Double)) => {
            SlotDefaultValuePolicy::PrimitiveDoubleZero
        }
        (_, _, JavaType::Primitive(_)) => SlotDefaultValuePolicy::PrimitiveNumericZero,
        (OwnershipShape::Owned, ViewShape::Native, JavaType::String) => {
            if facts.uses_java_string_identity_representation() {
                SlotDefaultValuePolicy::EmptyIdentityString
            } else {
                SlotDefaultValuePolicy::EmptyString
            }
        }
        (OwnershipShape::Owned, ViewShape::Native, JavaType::Named { name, args })
            if external_crate_mapped_named_rust_type(name, args, facts).is_some() =>
        {
            SlotDefaultValuePolicy::DefaultTrait
        }
        (OwnershipShape::Owned, ViewShape::Native, JavaType::Unknown(name))
            if external_crate_mapped_named_rust_type(name, &[], facts).is_some() =>
        {
            SlotDefaultValuePolicy::DefaultTrait
        }
        (OwnershipShape::Owned, ViewShape::Native, ty)
            if crate::stdlib::java_type_is_array_blocking_queue(ty)
                || crate::stdlib::java_type_is_priority_queue(ty) =>
        {
            SlotDefaultValuePolicy::DefaultTrait
        }
        (OwnershipShape::Owned, ViewShape::Native, ty)
            if crate::stdlib::java_collection_kind_for_type(ty)
                .is_some_and(|kind| !matches!(kind, crate::stdlib::JavaCollectionKind::List)) =>
        {
            // Sequence capability does not imply a Vec carrier. Queues, sets,
            // and maps have distinct native/facade carriers; `Default` creates
            // their empty value without guessing the representation.
            SlotDefaultValuePolicy::DefaultTrait
        }
        (OwnershipShape::Owned, ViewShape::Native, ty)
            if matches!(
                crate::types::ndarray_array2_element_type(ty),
                Some(JavaType::Primitive(_))
            ) =>
        {
            // Rectangular primitive arrays use Array2 rather than Vec. Its
            // empty Java/default state must therefore use the same carrier,
            // without assuming ndarray implements Default for Array2.
            let Some(JavaType::Primitive(element)) = crate::types::ndarray_array2_element_type(ty)
            else {
                unreachable!("guarded rectangular primitive array")
            };
            SlotDefaultValuePolicy::EmptyRectangularPrimitiveArray { element }
        }
        (OwnershipShape::Owned, ViewShape::Native, _) if content.sequence_element().is_some() => {
            SlotDefaultValuePolicy::EmptySequence
        }
        _ => SlotDefaultValuePolicy::DefaultTrait,
    }
}

fn slot_default_policy_expr(policy: &SlotDefaultPolicy) -> RustExpr {
    match policy {
        SlotDefaultPolicy::NoDefaultForClosedEnum { .. }
        | SlotDefaultPolicy::NoDefaultForDynTrait { .. }
        | SlotDefaultPolicy::NoDefaultForBoxedDynTrait { .. }
        | SlotDefaultPolicy::NoDefaultForErasedObject => {
            slot_no_default_message_policy(policy).compile_error_expr()
        }
        SlotDefaultPolicy::Value { policy } => slot_default_value_policy_expr(policy),
    }
}

pub(crate) fn slot_no_default_message_policy(
    policy: &SlotDefaultPolicy,
) -> SlotNoDefaultMessagePolicy {
    let message = match policy {
        SlotDefaultPolicy::NoDefaultForClosedEnum { enum_name } => {
            format!("no default value is available for closed enum slot {enum_name}")
        }
        SlotDefaultPolicy::NoDefaultForDynTrait { trait_name } => {
            format!("no default value is available for dyn trait slot {trait_name}")
        }
        SlotDefaultPolicy::NoDefaultForBoxedDynTrait { trait_name } => {
            format!("no default value is available for boxed dyn trait slot {trait_name}")
        }
        SlotDefaultPolicy::NoDefaultForErasedObject => {
            "no default value is available for erased object slot".to_string()
        }
        SlotDefaultPolicy::Value { .. } => {
            "default value is available; no no-default fallback is required".to_string()
        }
    };
    SlotNoDefaultMessagePolicy { message }
}

fn slot_default_value_policy_expr(policy: &SlotDefaultValuePolicy) -> RustExpr {
    match policy {
        SlotDefaultValuePolicy::EmptyStringView => RustExpr::Literal("\"\"".to_string()),
        SlotDefaultValuePolicy::EmptyStringReferenceView => RustExpr::Call {
            function: Box::new(RustExpr::path(["JavaString", "empty_ref"])),
            args: Vec::new(),
        },
        SlotDefaultValuePolicy::EmptyIdentityString => RustExpr::Call {
            function: Box::new(RustExpr::path(["JavaString", "intern_literal"])),
            args: vec![RustExpr::Literal("\"\"".to_string())],
        },
        SlotDefaultValuePolicy::EmptySliceView => RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(RustExpr::ArrayLiteral(Vec::new())),
        },
        SlotDefaultValuePolicy::PrimitiveBooleanZero => RustExpr::Literal("false".to_string()),
        SlotDefaultValuePolicy::PrimitiveCharZero => {
            RustExpr::Literal("JavaCodeUnit(0u16)".to_string())
        }
        SlotDefaultValuePolicy::PrimitiveNumericZero => RustExpr::Literal("0".to_string()),
        SlotDefaultValuePolicy::PrimitiveFloatZero => RustExpr::Literal("0f32".to_string()),
        SlotDefaultValuePolicy::PrimitiveDoubleZero => RustExpr::Literal("0f64".to_string()),
        SlotDefaultValuePolicy::EmptyString => RustExpr::Call {
            function: Box::new(RustExpr::path(["String", "new"])),
            args: Vec::new(),
        },
        SlotDefaultValuePolicy::EmptySequence => RustExpr::Call {
            function: Box::new(RustExpr::path(["Vec", "new"])),
            args: Vec::new(),
        },
        SlotDefaultValuePolicy::EmptyRetainedList {
            sync,
            empty_add_changes,
        } => {
            // The payload policy must agree with native_allocation_slot_rust_type;
            // the ordinary default wrapper adds Rc/RefCell or Arc/Mutex once.
            let runtime =
                retained_native_collection_runtime_name(StoragePlacement::new(true, true, *sync));
            let mut args = vec![
                slot_default_value_policy_expr(&SlotDefaultValuePolicy::EmptySequence),
                RustExpr::Literal("true".into()),
            ];
            if !empty_add_changes {
                args.push(RustExpr::Literal("false".into()));
            }
            RustExpr::Call {
                function: Box::new(RustExpr::path([
                    runtime,
                    if *empty_add_changes {
                        "list"
                    } else {
                        "list_with_empty_add_policy"
                    },
                ])),
                args,
            }
        }
        SlotDefaultValuePolicy::EmptyRectangularPrimitiveArray { element } => RustExpr::Call {
            function: Box::new(RustExpr::path(["ndarray", "Array2", "from_elem"])),
            args: vec![
                RustExpr::Tuple(vec![
                    RustExpr::Literal("0usize".to_string()),
                    RustExpr::Literal("0usize".to_string()),
                ]),
                match element {
                    PrimitiveType::Boolean => RustExpr::Literal("false".to_string()),
                    PrimitiveType::Float => RustExpr::Literal("0f32".to_string()),
                    PrimitiveType::Double => RustExpr::Literal("0f64".to_string()),
                    PrimitiveType::Char => RustExpr::Literal("JavaCodeUnit(0u16)".to_string()),
                    _ => RustExpr::Literal("0".to_string()),
                },
            ],
        },
        SlotDefaultValuePolicy::DefaultTrait => RustExpr::Call {
            function: Box::new(RustExpr::path(["Default", "default"])),
            args: Vec::new(),
        },
    }
}

pub fn rust_type_for_java_type(ty: &JavaType) -> RustType {
    rust_type_for_java_type_with_facts(ty, &FactSet::default())
}

pub fn rust_type_for_java_type_with_facts(ty: &JavaType, facts: &FactSet) -> RustType {
    let effective_facts = EffectiveFacts::new(facts);
    rust_type_for_java_type_with_effective_facts(ty, &effective_facts)
}

pub fn rust_type_for_java_type_with_facts_and_representations(
    ty: &JavaType,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> RustType {
    let effective_facts = EffectiveFacts::new(facts);
    rust_type_for_java_type_with_effective_facts_and_representations(
        ty,
        effective_facts,
        representations,
    )
}

pub fn rust_type_for_java_type_with_facts_representations_and_external_index(
    ty: &JavaType,
    facts: &FactSet,
    representations: &RepresentationPlan,
    external_index: &crate::facts::ExternalFactLookupIndex<'_>,
) -> RustType {
    rust_type_for_java_type_with_effective_facts_and_representations(
        ty,
        EffectiveFacts::with_external_index(facts, external_index),
        representations,
    )
}

fn rust_type_for_java_type_with_effective_facts_and_representations(
    ty: &JavaType,
    effective_facts: EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> RustType {
    if let Some(rendered) = planned_nested_collection_type(ty, &effective_facts, |argument| {
        rust_type_for_java_generic_arg_with_representations(
            argument,
            &effective_facts,
            representations,
        )
    }) {
        return rendered;
    }
    if java_array_rank(ty) >= 3 {
        // `Array2` is an exact rank-two carrier.  If its rank-two suffix is
        // rendered recursively as a generic argument of a higher-rank array,
        // declarations become `Vec<Array2<T>>` while construction and indexed
        // access correctly use Java's nested/ragged `Vec<Vec<Vec<T>>>`
        // representation. Keep the whole higher-rank carrier nested.
        return rust_type_for_nested_higher_rank_array_with_representations(
            ty,
            &effective_facts,
            representations,
        );
    }
    if let Some(instantiated) = raw_generic_instantiation_for_rendering(ty, representations) {
        // Java raw source generics erase each parameter to its declared upper
        // bound (or Object). Rust has no omitted-generic syntax, so make that
        // erasure explicit before choosing the concrete/trait/closed carrier.
        // Doing this at the canonical renderer keeps declarations, locals,
        // arguments, and hierarchy carriers on the same ABI.
        return rust_type_for_java_type_with_effective_facts_and_representations(
            &instantiated,
            effective_facts,
            representations,
        );
    }
    if let Some(carrier) =
        map_entry_rust_type_with_representations(ty, &effective_facts, representations)
    {
        return carrier;
    }
    if let Some(element_ty) = optional_element_type(ty) {
        return RustType::Option(Box::new(
            rust_type_for_optional_element_arg_with_representations(
                &element_ty,
                &effective_facts,
                representations,
            ),
        ));
    }
    if let Some(element) = effective_facts.native_iterable_element_type(ty) {
        return rust_generic_type(
            "Iterable",
            vec![rust_type_for_java_generic_arg_with_representations(
                &element,
                &effective_facts,
                representations,
            )],
        );
    }
    if let JavaType::Named { name, args } = ty
        && effective_facts.parsed_source_controls_native_carrier(ty)
    {
        if let Some(dispatch) =
            rust_type_for_representation_dispatch(ty, &effective_facts, representations)
        {
            return dispatch;
        }
        if args.is_empty() {
            return RustType::Named(rust_name_for_java_named_type(name));
        }
        return rust_generic_type(
            rust_name_for_java_named_type(name),
            args.iter()
                .map(|argument| {
                    rust_type_for_java_generic_arg_with_representations(
                        argument,
                        &effective_facts,
                        representations,
                    )
                })
                .collect(),
        );
    }
    if let Some(carrier) = native_functional_rust_type(ty, &effective_facts, |arg| {
        rust_type_for_java_generic_arg_with_representations(arg, &effective_facts, representations)
    }) {
        return carrier;
    }
    if let JavaType::Named { name, args } = ty
        && crate::stdlib::canonical_external_receiver_type_key(name) == "Class"
    {
        let token_ty = args.first().map_or(RustType::Unit, |arg| {
            let token = match arg {
                JavaType::Wildcard(Some(WildcardBound::Extends(bound)))
                | JavaType::Wildcard(Some(WildcardBound::Super(bound))) => bound.as_ref(),
                JavaType::Wildcard(None) => return RustType::Unit,
                other => other,
            };
            rust_type_for_java_generic_arg_with_representations(
                token,
                &effective_facts,
                representations,
            )
        });
        return rust_generic_type("JavaClass", vec![token_ty]);
    }
    if let JavaType::Named { name, args } = ty
        && matches!(
            crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
            "Iterator"
                | "java.util.Iterator"
                | "CloseableIterator"
                | "htsjdk.samtools.util.CloseableIterator"
                | "ListIterator"
                | "java.util.ListIterator"
        )
    {
        let element_ty = args
            .first()
            .cloned()
            .unwrap_or_else(erased_object_java_type);
        let carrier = if matches!(
            crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
            "ListIterator" | "java.util.ListIterator"
        ) {
            "JavaListIterator"
        } else {
            "JavaIterator"
        };
        return rust_generic_type(
            carrier,
            vec![rust_type_for_java_generic_arg_with_representations(
                &element_ty,
                &effective_facts,
                representations,
            )],
        );
    }
    if let JavaType::Named { name, args } = ty
        && crate::stdlib::canonical_external_receiver_type_key(name) == "Stream"
    {
        let element_ty = args
            .first()
            .cloned()
            .unwrap_or_else(erased_object_java_type);
        return rust_generic_type(
            "JavaStream",
            vec![rust_type_for_java_generic_arg_with_representations(
                &element_ty,
                &effective_facts,
                representations,
            )],
        );
    }
    if (external_final_facade_should_keep_named_type(ty, &effective_facts)
        || external_model_concrete_dispatch_should_keep_named_type(
            ty,
            &effective_facts,
            representations,
        ))
        && let JavaType::Named { name, args } = ty
    {
        // An exact modeled external facade is the executable carrier even when
        // it also implements a collection interface. Collection membership is
        // useful for Java subtype and method reasoning, but must not replace a
        // facade with a native Vec/Set/Map at storage boundaries. Preserve the
        // payload arguments of a generic facade here: this branch precedes the
        // general external-generic renderer below, and spelling only the name
        // would silently turn `Reader<Record>` into its erased default
        // `Reader<Any>`.
        if crate::stdlib::external_facade_generic_carrier_arity(name) == Some(args.len()) {
            return rust_generic_type(
                rust_name_for_java_named_type(name),
                args.iter()
                    .map(|arg| {
                        rust_type_for_java_generic_arg_with_representations(
                            arg,
                            &effective_facts,
                            representations,
                        )
                    })
                    .collect(),
            );
        }
        return RustType::Named(rust_name_for_java_named_type(name));
    }
    if let JavaType::Named { name, args } = ty
        && (!args.is_empty() || crate::stdlib::java_collection_kind_for_name(name).is_some())
        && let Some(kind) = crate::stdlib::java_collection_kind_for_type(ty)
    {
        match kind {
            crate::stdlib::JavaCollectionKind::List
                if crate::stdlib::java_type_is_priority_queue(ty) =>
            {
                let element_ty = rust_storage_collection_element_java_type(
                    args.first()
                        .cloned()
                        .unwrap_or_else(erased_object_java_type),
                    &effective_facts,
                );
                return rust_generic_type(
                    "JavaPriorityQueue",
                    vec![rust_type_for_java_generic_arg_with_representations(
                        &element_ty,
                        &effective_facts,
                        representations,
                    )],
                );
            }
            crate::stdlib::JavaCollectionKind::List
                if crate::stdlib::java_type_is_array_blocking_queue(ty) =>
            {
                let element_ty = rust_storage_collection_element_java_type(
                    args.first()
                        .cloned()
                        .unwrap_or_else(erased_object_java_type),
                    &effective_facts,
                );
                return rust_generic_type(
                    "JavaArrayBlockingQueue",
                    vec![rust_type_for_java_generic_arg_with_representations(
                        &element_ty,
                        &effective_facts,
                        representations,
                    )],
                );
            }
            crate::stdlib::JavaCollectionKind::List => {
                let element_ty = rust_storage_collection_element_java_type(
                    args.first()
                        .cloned()
                        .unwrap_or_else(erased_object_java_type),
                    &effective_facts,
                );
                return RustType::Vec(Box::new(
                    rust_type_for_java_generic_arg_with_representations(
                        &element_ty,
                        &effective_facts,
                        representations,
                    ),
                ));
            }
            kind if kind.is_set_like() => {
                let element_ty = rust_storage_collection_element_java_type(
                    args.first()
                        .cloned()
                        .unwrap_or_else(erased_object_java_type),
                    &effective_facts,
                );
                let element_rust = rust_type_for_java_generic_arg_with_representations(
                    &element_ty,
                    &effective_facts,
                    representations,
                );
                if kind == crate::stdlib::JavaCollectionKind::InsertionOrderedSet {
                    return rust_generic_type("JavaLinkedHashSet", vec![element_rust]);
                }
                if java_set_element_uses_hash_free_carrier_with_representations(
                    Some(&element_ty),
                    &effective_facts,
                    representations,
                ) {
                    // Source hierarchy/erased carriers generally cannot
                    // implement Rust `Hash` or `Ord`, but Java Set semantics
                    // still require equality-based de-duplication.  Keep the
                    // set contract in the runtime carrier instead of silently
                    // degrading it to a Vec.
                    return if java_set_element_uses_semantic_equality_carrier(
                        &element_ty,
                        representations,
                    ) {
                        rust_generic_type(
                            if matches!(kind, crate::stdlib::JavaCollectionKind::SortedSet) {
                                "JavaSemanticTreeSet"
                            } else {
                                "JavaSemanticHashSet"
                            },
                            vec![element_rust],
                        )
                    } else {
                        RustType::Vec(Box::new(element_rust))
                    };
                }
                return rust_generic_type(
                    match kind {
                        crate::stdlib::JavaCollectionKind::SortedSet => {
                            "std::collections::BTreeSet"
                        }
                        crate::stdlib::JavaCollectionKind::InsertionOrderedSet => {
                            "JavaLinkedHashSet"
                        }
                        _ => "std::collections::HashSet",
                    },
                    vec![element_rust],
                );
            }
            kind if kind.is_map_like() => {}
            _ => {}
        }
    }

    if representations.uses_comparator_backed_native_map_carrier()
        && let Some(kind) = crate::stdlib::java_collection_kind_for_type(ty)
        && kind.is_map_like()
        && let Some((key, value)) = crate::stdlib::java_map_type_args(ty)
    {
        return rust_generic_type(
            "JavaSemanticHashMap",
            vec![
                rust_type_for_java_generic_arg_with_representations(
                    key,
                    &effective_facts,
                    representations,
                ),
                rust_type_for_java_generic_arg_with_representations(
                    value,
                    &effective_facts,
                    representations,
                ),
            ],
        );
    }

    if let Some(kind) = crate::stdlib::java_collection_kind_for_type(ty)
        && kind.is_map_like()
        && let Some((key, value)) = crate::stdlib::java_map_type_args(ty)
    {
        let key_rust = rust_type_for_java_generic_arg_with_representations(
            key,
            &effective_facts,
            representations,
        );
        let value_rust = rust_type_for_java_generic_arg_with_representations(
            value,
            &effective_facts,
            representations,
        );
        let carrier = if java_set_element_uses_hash_free_carrier_with_representations(
            Some(key),
            &effective_facts,
            representations,
        ) {
            "JavaSemanticHashMap"
        } else {
            match kind {
                crate::stdlib::JavaCollectionKind::SortedMap => "std::collections::BTreeMap",
                crate::stdlib::JavaCollectionKind::InsertionOrderedMap => "JavaLinkedHashMap",
                _ => "std::collections::HashMap",
            }
        };
        return rust_generic_type(carrier, vec![key_rust, value_rust]);
    }

    if let Some(ty) = rust_type_for_external_generic_facade_with_representations(
        ty,
        &effective_facts,
        representations,
    ) {
        return ty;
    }

    if let JavaType::Named { name, args } = ty
        && let Some(ty) = external_crate_mapped_named_rust_type(name, args, &effective_facts)
    {
        return ty;
    }
    if let JavaType::Unknown(name) = ty
        && let Some(ty) = external_crate_mapped_named_rust_type(name, &[], &effective_facts)
    {
        return ty;
    }

    // Rectangular primitive two-dimensional Java arrays have one carrier across
    // construction, access, and slots: `ndarray::Array2<T>`.  This
    // representation-aware path used to take the generic array branch below,
    // which rendered declarations as `Vec<Vec<T>>` even though array lowering
    // had correctly created an `Array2<T>`.
    if let Some(element_ty) = crate::types::ndarray_array2_element_type(ty) {
        return RustType::Array2(Box::new(
            rust_type_for_java_generic_arg_with_representations(
                &element_ty,
                &effective_facts,
                representations,
            ),
        ));
    }

    if let JavaType::Array(element_ty) = ty {
        let element_ty = rust_storage_array_element_java_type(element_ty.as_ref().clone());
        return RustType::Vec(Box::new(
            rust_type_for_java_generic_arg_with_representations(
                &element_ty,
                &effective_facts,
                representations,
            ),
        ));
    }

    if let Some(dispatch_ty) =
        rust_type_for_representation_dispatch(ty, &effective_facts, representations)
    {
        return dispatch_ty;
    }

    if let JavaType::Named { name, args } = ty
        && !args.is_empty()
    {
        // Every remaining nominal generic is still a carrier boundary for
        // its arguments, including an opaque external facade. Native and
        // explicitly mapped carriers were selected above. Do not discard the
        // representation plan while rendering their nominal peers' contents.
        return rust_generic_type(
            rust_name_for_java_named_type(name),
            args.iter()
                .enumerate()
                .map(|(index, arg)| {
                    rust_type_for_source_generic_argument(
                        name,
                        index,
                        arg,
                        &effective_facts,
                        representations,
                    )
                })
                .collect(),
        );
    }

    rust_type_for_java_type_with_effective_facts(ty, &effective_facts)
}

pub fn rust_type_for_java_type_with_effective_facts(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> RustType {
    if let Some(rendered) = planned_nested_collection_type(ty, facts, |argument| {
        rust_type_for_java_generic_arg_with_effective_facts(argument, facts)
    }) {
        return rendered;
    }
    if java_array_rank(ty) >= 3 {
        return rust_type_for_nested_higher_rank_array(ty, facts);
    }
    render_native_java_rust_type_policy_with_effective_facts(
        &native_java_rust_type_policy_with_effective_facts(ty, facts),
        facts,
    )
}

fn java_array_rank(ty: &JavaType) -> usize {
    match ty {
        JavaType::Array(element) => 1 + java_array_rank(element),
        _ => 0,
    }
}

fn planned_nested_collection_type(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
    render_argument: impl FnMut(&JavaType) -> RustType,
) -> Option<RustType> {
    if facts.nested_collection_view(ty) != Some(&ViewShape::JavaSemanticHash) {
        return None;
    }
    let JavaType::Named { args, .. } = ty else {
        return None;
    };
    Some(rust_generic_type(
        semantic_hash_collection_base(ty)?,
        args.iter().map(render_argument).collect(),
    ))
}

fn rust_type_for_nested_higher_rank_array_with_representations(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> RustType {
    match ty {
        JavaType::Array(element) => RustType::Vec(Box::new(
            rust_type_for_nested_higher_rank_array_with_representations(
                element,
                facts,
                representations,
            ),
        )),
        _ => rust_type_for_java_generic_arg_with_representations(ty, facts, representations),
    }
}

fn rust_type_for_nested_higher_rank_array(ty: &JavaType, facts: &EffectiveFacts<'_>) -> RustType {
    match ty {
        JavaType::Array(element) => RustType::Vec(Box::new(
            rust_type_for_nested_higher_rank_array(element, facts),
        )),
        _ => rust_type_for_java_generic_arg_with_effective_facts(ty, facts),
    }
}

pub fn native_java_rust_type_policy_with_effective_facts(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> NativeJavaRustTypePolicy {
    if optional_element_type(ty).is_none()
        && let Some(element) = facts.native_iterable_element_type(ty)
    {
        return NativeJavaRustTypePolicy::Direct {
            ty: rust_generic_type(
                "Iterable",
                vec![rust_type_for_java_generic_arg_with_effective_facts(
                    &element, facts,
                )],
            ),
        };
    }
    if let JavaType::Named { name, args } = ty
        && facts.parsed_source_controls_native_carrier(ty)
    {
        if args.is_empty() {
            return NativeJavaRustTypePolicy::Direct {
                ty: RustType::Named(rust_name_for_java_named_type(name)),
            };
        }
        return NativeJavaRustTypePolicy::Direct {
            ty: rust_generic_type(
                rust_name_for_java_named_type(name),
                args.iter()
                    .map(|argument| {
                        rust_type_for_java_generic_arg_with_effective_facts(argument, facts)
                    })
                    .collect(),
            ),
        };
    }
    if let Some(carrier) = native_functional_rust_type(ty, facts, |arg| {
        rust_type_for_java_generic_arg_with_effective_facts(arg, facts)
    }) {
        return NativeJavaRustTypePolicy::Direct { ty: carrier };
    }
    if let JavaType::Named { name, args } = ty
        && args.is_empty()
        && let Some(primitive) = boxed_primitive_storage_java_type(name, facts)
    {
        return native_java_rust_type_policy_with_effective_facts(
            &JavaType::Primitive(primitive),
            facts,
        );
    }
    if let Some(element_ty) = crate::types::ndarray_array2_element_type(ty) {
        return NativeJavaRustTypePolicy::Direct {
            ty: RustType::Array2(Box::new(
                rust_type_for_java_generic_arg_with_effective_facts(&element_ty, facts),
            )),
        };
    }
    if let JavaType::Named { name, args } = ty
        && let Some(ty) = external_crate_mapped_named_rust_type(name, args, facts)
    {
        return NativeJavaRustTypePolicy::Direct { ty };
    }
    if let JavaType::Unknown(name) = ty
        && let Some(ty) = external_crate_mapped_named_rust_type(name, &[], facts)
    {
        return NativeJavaRustTypePolicy::Direct { ty };
    }
    if let JavaType::Named { name, args } = ty
        && matches!(
            crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
            "Iterator"
                | "java.util.Iterator"
                | "CloseableIterator"
                | "htsjdk.samtools.util.CloseableIterator"
                | "ListIterator"
                | "java.util.ListIterator"
        )
    {
        let carrier = if matches!(
            crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
            "ListIterator" | "java.util.ListIterator"
        ) {
            "JavaListIterator"
        } else {
            "JavaIterator"
        };
        return NativeJavaRustTypePolicy::Direct {
            ty: rust_generic_type(
                carrier,
                vec![rust_type_for_java_generic_arg_with_effective_facts(
                    &args
                        .first()
                        .cloned()
                        .unwrap_or_else(erased_object_java_type),
                    facts,
                )],
            ),
        };
    }
    if let JavaType::Named { name, args } = ty
        && crate::stdlib::external_facade_generic_carrier_arity(name) == Some(args.len())
    {
        // Generic facade arguments are part of the executable Rust ABI even
        // when the Java hierarchy itself is modeled conservatively as an
        // erased external object. This facts-only renderer is used while
        // building facade method signatures, before a representation plan is
        // available, so it must preserve the same arguments as the full
        // representation-aware renderer.
        return NativeJavaRustTypePolicy::Direct {
            ty: rust_generic_type(
                rust_name_for_java_named_type(name),
                args.iter()
                    .map(|arg| rust_type_for_java_generic_arg_with_effective_facts(arg, facts))
                    .collect(),
            ),
        };
    }
    if erased_facade_concrete_dispatch_should_keep_named_type(ty, facts)
        && let JavaType::Named { name, .. } = ty
    {
        return NativeJavaRustTypePolicy::Direct {
            ty: RustType::Named(rust_name_for_java_named_type(name)),
        };
    }
    if matches!(
        facts.object_representation_override_strategy_for(ty),
        Some(HierarchyStrategy::ErasedObject)
    ) && !external_final_facade_should_keep_named_type(ty, facts)
        && !crate::javalibs::generic_facade_interface_erases_to_named_rust_type(ty)
        && crate::stdlib::java_collection_kind_for_type(ty).is_none()
    {
        return NativeJavaRustTypePolicy::Direct {
            ty: erased_object_rust_type(),
        };
    }
    if external_final_facade_should_keep_named_type(ty, facts)
        && let JavaType::Named { name, .. } = ty
    {
        return NativeJavaRustTypePolicy::Direct {
            ty: RustType::Named(rust_name_for_java_named_type(name)),
        };
    }
    if crate::javalibs::generic_facade_interface_erases_to_named_rust_type(ty)
        && let JavaType::Named { name, args } = ty
    {
        return NativeJavaRustTypePolicy::Direct {
            ty: rust_generic_type(
                rust_name_for_java_named_type(name),
                args.iter()
                    .map(|arg| rust_type_for_java_generic_arg_with_effective_facts(arg, facts))
                    .collect(),
            ),
        };
    }
    if let Some(enum_name) = closed_runtime_case_split_enum_name(ty, facts) {
        return NativeJavaRustTypePolicy::Direct {
            ty: RustType::Named(enum_name),
        };
    }
    if let JavaType::Named { name, args } = ty
        && (!args.is_empty() || crate::stdlib::java_collection_kind_for_name(name).is_some())
        && let Some(kind) = crate::stdlib::java_collection_kind_for_type(ty)
    {
        match kind {
            crate::stdlib::JavaCollectionKind::List
                if crate::stdlib::java_type_is_priority_queue(ty) =>
            {
                return NativeJavaRustTypePolicy::Direct {
                    ty: rust_generic_type(
                        "JavaPriorityQueue",
                        vec![rust_type_for_java_generic_arg_with_effective_facts(
                            &rust_storage_collection_element_java_type(
                                args.first()
                                    .cloned()
                                    .unwrap_or_else(erased_object_java_type),
                                facts,
                            ),
                            facts,
                        )],
                    ),
                };
            }
            crate::stdlib::JavaCollectionKind::List
                if crate::stdlib::java_type_is_array_blocking_queue(ty) =>
            {
                return NativeJavaRustTypePolicy::Direct {
                    ty: rust_generic_type(
                        "JavaArrayBlockingQueue",
                        vec![rust_type_for_java_generic_arg_with_effective_facts(
                            &rust_storage_collection_element_java_type(
                                args.first()
                                    .cloned()
                                    .unwrap_or_else(erased_object_java_type),
                                facts,
                            ),
                            facts,
                        )],
                    ),
                };
            }
            crate::stdlib::JavaCollectionKind::List => {
                return NativeJavaRustTypePolicy::Sequence {
                    container: NativeSequenceContainer::Vec,
                    element_ty: rust_storage_collection_element_java_type(
                        args.first()
                            .cloned()
                            .unwrap_or_else(erased_object_java_type),
                        facts,
                    ),
                };
            }
            kind if kind.is_set_like() => {
                let element_ty = rust_storage_collection_element_java_type(
                    args.first()
                        .cloned()
                        .unwrap_or_else(erased_object_java_type),
                    facts,
                );
                return NativeJavaRustTypePolicy::Sequence {
                    container: match kind {
                        crate::stdlib::JavaCollectionKind::SortedSet => {
                            NativeSequenceContainer::BTreeSet
                        }
                        crate::stdlib::JavaCollectionKind::InsertionOrderedSet => {
                            NativeSequenceContainer::LinkedHashSet
                        }
                        _ if java_set_element_uses_hash_free_carrier(&element_ty, facts) => {
                            NativeSequenceContainer::HashFreeSet
                        }
                        _ => NativeSequenceContainer::HashSet,
                    },
                    element_ty,
                };
            }
            kind if kind.is_map_like() && args.len() == 2 => {}
            kind if kind.is_map_like() => {
                return NativeJavaRustTypePolicy::Direct {
                    ty: rust_generic_type(
                        match kind {
                            crate::stdlib::JavaCollectionKind::SortedMap => {
                                "std::collections::BTreeMap"
                            }
                            crate::stdlib::JavaCollectionKind::InsertionOrderedMap => {
                                "JavaLinkedHashMap"
                            }
                            _ => "std::collections::HashMap",
                        },
                        vec![RustType::String, erased_object_rust_type()],
                    ),
                };
            }
            crate::stdlib::JavaCollectionKind::Set
            | crate::stdlib::JavaCollectionKind::SortedSet
            | crate::stdlib::JavaCollectionKind::InsertionOrderedSet
            | crate::stdlib::JavaCollectionKind::Map
            | crate::stdlib::JavaCollectionKind::SortedMap
            | crate::stdlib::JavaCollectionKind::InsertionOrderedMap => {
                unreachable!("set/map collection kinds are handled by guarded arms")
            }
        }
    }
    if let Some(element_ty) =
        slot_content_capability_policy_with_effective_facts(ty, facts).into_sequence_element()
    {
        return NativeJavaRustTypePolicy::Sequence {
            container: NativeSequenceContainer::Vec,
            element_ty: rust_storage_sequence_element_java_type(ty, element_ty, facts),
        };
    }
    if let Some(element_ty) = optional_element_type(ty) {
        return NativeJavaRustTypePolicy::Optional { element_ty };
    }
    match ty {
        // Java erases an intersection to its leftmost bound. A Rust generic
        // declaration retains every bound, while a value-position projection
        // uses that same Java erasure rule.
        JavaType::Intersection(bounds) => bounds
            .first()
            .map(|bound| native_java_rust_type_policy_with_effective_facts(bound, facts))
            .unwrap_or(NativeJavaRustTypePolicy::WildcardObject),
        JavaType::Void => NativeJavaRustTypePolicy::Direct { ty: RustType::Unit },
        JavaType::Primitive(PrimitiveType::Boolean) => {
            NativeJavaRustTypePolicy::Direct { ty: RustType::Bool }
        }
        JavaType::Primitive(PrimitiveType::Byte) => {
            NativeJavaRustTypePolicy::Direct { ty: RustType::I8 }
        }
        JavaType::Primitive(PrimitiveType::Short) => {
            NativeJavaRustTypePolicy::Direct { ty: RustType::I16 }
        }
        JavaType::Primitive(PrimitiveType::Char) => NativeJavaRustTypePolicy::Direct {
            ty: RustType::Named("JavaCodeUnit".to_string()),
        },
        JavaType::Primitive(PrimitiveType::Int) => {
            NativeJavaRustTypePolicy::Direct { ty: RustType::I32 }
        }
        JavaType::Primitive(PrimitiveType::Long) => {
            NativeJavaRustTypePolicy::Direct { ty: RustType::I64 }
        }
        JavaType::Primitive(PrimitiveType::Float) => {
            NativeJavaRustTypePolicy::Direct { ty: RustType::F32 }
        }
        JavaType::Primitive(PrimitiveType::Double) => {
            NativeJavaRustTypePolicy::Direct { ty: RustType::F64 }
        }
        JavaType::String => NativeJavaRustTypePolicy::Direct {
            ty: if facts.uses_java_string_identity_representation() {
                RustType::Named("JavaString".to_string())
            } else {
                RustType::String
            },
        },
        JavaType::Array(_) => NativeJavaRustTypePolicy::ArrayRequiresSequenceElement,
        JavaType::Named { name, args }
            if crate::stdlib::canonical_external_receiver_type_key(name) == "Class" =>
        {
            let token_ty = args.first().map_or_else(
                || RustType::Unit,
                |arg| class_token_arg_rust_type(arg, facts),
            );
            NativeJavaRustTypePolicy::Direct {
                ty: rust_generic_type("JavaClass", vec![token_ty]),
            }
        }
        JavaType::Named { name, args }
            if crate::stdlib::canonical_external_receiver_type_key(name) == "Stream" =>
        {
            let element_ty = args
                .first()
                .cloned()
                .unwrap_or_else(erased_object_java_type);
            NativeJavaRustTypePolicy::Direct {
                ty: rust_generic_type(
                    "JavaStream",
                    vec![rust_type_for_java_generic_arg_with_effective_facts(
                        &element_ty,
                        facts,
                    )],
                ),
            }
        }
        JavaType::Named { .. } if crate::stdlib::java_map_type_args(ty).is_some() => {
            let (key, value) = crate::stdlib::java_map_type_args(ty).unwrap();
            let kind = crate::stdlib::java_collection_kind_for_type(ty)
                .unwrap_or(crate::stdlib::JavaCollectionKind::Map);
            NativeJavaRustTypePolicy::Direct {
                ty: rust_generic_type(
                    if matches!(kind, crate::stdlib::JavaCollectionKind::SortedMap) {
                        "std::collections::BTreeMap"
                    } else if matches!(kind, crate::stdlib::JavaCollectionKind::InsertionOrderedMap)
                    {
                        "JavaLinkedHashMap"
                    } else {
                        "std::collections::HashMap"
                    },
                    vec![
                        rust_type_for_java_generic_arg_with_effective_facts(key, facts),
                        rust_type_for_java_generic_arg_with_effective_facts(value, facts),
                    ],
                ),
            }
        }
        JavaType::Named { name, args }
            if matches!(
                crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
                "Entry" | "Map.Entry" | "java.util.Map.Entry"
            ) && args.len() == 2 =>
        {
            NativeJavaRustTypePolicy::Direct {
                ty: RustType::Tuple(vec![
                    rust_type_for_java_generic_arg_with_effective_facts(&args[0], facts),
                    rust_type_for_java_generic_arg_with_effective_facts(&args[1], facts),
                ]),
            }
        }
        JavaType::Named { name, args } if !args.is_empty() => NativeJavaRustTypePolicy::Direct {
            ty: rust_generic_type(
                rust_name_for_java_named_type(name),
                args.iter()
                    .map(|arg| rust_type_for_java_generic_arg_with_effective_facts(arg, facts))
                    .collect(),
            ),
        },
        JavaType::Named { name, .. } => {
            if let JavaType::Named { args, .. } = ty
                && let Some(ty) = external_crate_mapped_named_rust_type(name, args, facts)
            {
                NativeJavaRustTypePolicy::Direct { ty }
            } else {
                native_named_java_rust_type_policy(name).into_policy()
            }
        }
        JavaType::Wildcard(_) => NativeJavaRustTypePolicy::WildcardObject,
        JavaType::Unknown(raw) => {
            if let Some(ty) = external_crate_mapped_named_rust_type(raw, &[], facts) {
                NativeJavaRustTypePolicy::Direct { ty }
            } else {
                NativeJavaRustTypePolicy::UnknownRaw { raw: raw.clone() }
            }
        }
    }
}

fn external_crate_mapped_named_rust_type(
    name: &str,
    args: &[JavaType],
    facts: &EffectiveFacts<'_>,
) -> Option<RustType> {
    let pack = facts.external_rust_crate_mapping_pack_for_type(name)?;
    let crate_name = pack.rust_target.as_deref()?;
    let type_name = facts
        .external_crate_rust_type_name_for(name)
        .map(str::to_string)
        .unwrap_or_else(|| crate::stdlib::default_external_crate_rust_type_name(name));
    let generic_args = args
        .iter()
        .map(|arg| rust_type_for_java_generic_arg_with_effective_facts(arg, facts))
        .collect::<Vec<_>>();
    if generic_args.is_empty() {
        Some(RustType::Named(format!("{crate_name}::{type_name}")))
    } else {
        Some(rust_generic_type(
            format!("{crate_name}::{type_name}"),
            generic_args,
        ))
    }
}

fn class_token_arg_rust_type(arg: &JavaType, facts: &EffectiveFacts<'_>) -> RustType {
    match arg {
        JavaType::Wildcard(None) => RustType::Unit,
        JavaType::Wildcard(Some(WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(WildcardBound::Super(bound))) => {
            class_token_arg_rust_type(bound, facts)
        }
        _ => rust_type_for_java_type_with_effective_facts(arg, facts),
    }
}

pub fn slot_content_capability_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> SlotContentCapabilityPolicy {
    SlotContentCapabilityPolicy::new(
        facts.is_string_like_type(java_ty),
        facts.sequence_element_type_for(java_ty),
    )
}

pub(crate) fn native_named_java_rust_type_policy(name: &str) -> NativeNamedJavaRustTypePolicy {
    let rust_name = rust_name_for_java_named_type(name);
    let policy = match name {
        name if crate::java_append_plan::qualified_native_builder_name(name).is_some() => {
            NativeJavaRustTypePolicy::Direct {
                ty: RustType::Named(
                    crate::java_append_plan::qualified_native_builder_name(name)
                        .unwrap()
                        .into(),
                ),
            }
        }
        "Integer" => NativeJavaRustTypePolicy::Direct { ty: RustType::I32 },
        "Long" => NativeJavaRustTypePolicy::Direct { ty: RustType::I64 },
        "Double" => NativeJavaRustTypePolicy::Direct { ty: RustType::F64 },
        "Float" => NativeJavaRustTypePolicy::Direct { ty: RustType::F32 },
        "Boolean" => NativeJavaRustTypePolicy::Direct { ty: RustType::Bool },
        "Character" => NativeJavaRustTypePolicy::Direct {
            ty: RustType::Named("JavaCodeUnit".to_string()),
        },
        "Object" | "java.lang.Object" => NativeJavaRustTypePolicy::Direct {
            ty: erased_object_rust_type(),
        },
        name if java_exception_like_type_name(name) => NativeJavaRustTypePolicy::Direct {
            ty: RustType::Named("JavaException".to_string()),
        },
        "Properties" => NativeJavaRustTypePolicy::Direct {
            ty: rust_generic_type(
                "std::collections::HashMap",
                vec![RustType::String, RustType::String],
            ),
        },
        _ => NativeJavaRustTypePolicy::Named {
            name: rust_name.clone(),
        },
    };
    NativeNamedJavaRustTypePolicy {
        name: name.to_string(),
        policy,
    }
}

fn java_exception_like_type_name(name: &str) -> bool {
    crate::stdlib::is_known_throwable_carrier_type(name)
}

fn erased_object_java_type() -> JavaType {
    JavaType::Named {
        name: "Object".to_string(),
        args: Vec::new(),
    }
}

fn external_final_facade_should_keep_named_type(ty: &JavaType, facts: &EffectiveFacts<'_>) -> bool {
    java_type_name_for_builtin_final_erased_lookup(ty)
        .is_some_and(|name| facts.has_external_final_erased_facade_for_name(name))
}

/// A generated external-model facade is the executable carrier for an exact
/// concrete dispatch constraint, even when the external Java class is not
/// final. Re-projecting that constraint through a conservative hierarchy plan
/// would erase the value to `Any` and make modeled instance calls impossible.
///
/// Representation provenance is the authoritative distinction here. Pack
/// ownership is intentionally sparse for types discovered through modeled
/// members, while parsed/corpus source representations may legitimately need
/// hierarchy or shared-storage carriers instead of a generated facade.
fn external_model_concrete_dispatch_should_keep_named_type(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> bool {
    let Some(name) = java_type_name_for_builtin_final_erased_lookup(ty) else {
        return false;
    };
    if matches!(
        facts.object_representation_override_strategy_for(ty),
        Some(HierarchyStrategy::ConcreteStruct)
    ) {
        // A concrete dispatch slot plus an explicit representation fact is a
        // complete nominal-carrier contract.  This commonly comes from an
        // external pack whose generated facade is executable even though the
        // conservative whole-program representation (built without source
        // for that dependency) remained erased.  Re-projecting through that
        // stale plan would turn `T::new()` and typed facade results back into
        // `Box<dyn Any>` at local and signature boundaries.
        return true;
    }
    erased_facade_concrete_dispatch_should_keep_named_type(ty, facts)
        && representations
            .type_representation(name)
            .is_some_and(|entry| {
                entry.provenance() == RepresentationProvenance::BuiltinExternalModel
            })
}

fn java_type_name_for_builtin_final_erased_lookup(ty: &JavaType) -> Option<&str> {
    match ty {
        JavaType::Named { name, .. } => Some(name.as_str()),
        JavaType::Unknown(name) => Some(name.as_str()),
        _ => None,
    }
}

fn erased_final_facade_generic_arg_should_keep_named_type(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> bool {
    matches!(
        facts.object_representation_override_strategy_for(ty),
        Some(HierarchyStrategy::ErasedObject)
    ) && facts.is_final_type(ty)
}

fn erased_facade_concrete_dispatch_should_keep_named_type(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> bool {
    let erased;
    let ty = if let JavaType::Named { name, args } = ty
        && !args.is_empty()
    {
        erased = JavaType::Named {
            name: name.clone(),
            args: Vec::new(),
        };
        &erased
    } else {
        ty
    };
    matches!(
        facts.object_representation_override_strategy_for(ty),
        Some(HierarchyStrategy::ErasedObject)
    )
}

/// Renders a generic argument, wrapping a shared type in its handle. Sound for
/// user generics because every generated trait carries the `Rc`/`Arc` forwarding
/// impls.
pub(crate) fn rust_type_for_java_generic_arg_with_effective_facts(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> RustType {
    let rendered = rust_type_for_java_generic_arg_unwrapped(ty, facts);
    let JavaType::Named { name, .. } = ty else {
        return rendered;
    };
    let placement = facts.storage_placement_for(name);
    if !placement.is_implemented() || !placement.shared() {
        return rendered;
    }
    storage_placement_type(placement, rendered)
}

fn rust_type_for_java_generic_arg_unwrapped(ty: &JavaType, facts: &EffectiveFacts<'_>) -> RustType {
    let ty = crate::types::generic_read_type_ref(ty);
    if let Some(element_ty) = optional_element_type(ty) {
        return RustType::Option(Box::new(rust_type_for_optional_element_arg(
            &element_ty,
            facts,
        )));
    }
    if let JavaType::Named { name, args } = ty
        && args.is_empty()
        && let Some(primitive) = boxed_primitive_storage_java_type(name, facts)
    {
        return rust_type_for_java_type_with_effective_facts(
            &JavaType::Primitive(primitive),
            facts,
        );
    }
    if let JavaType::Named { name, args } = ty
        && matches!(
            crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
            "Entry" | "Map.Entry" | "java.util.Map.Entry"
        )
        && args.len() == 2
    {
        return RustType::Tuple(vec![
            rust_type_for_java_generic_arg_with_effective_facts(&args[0], facts),
            rust_type_for_java_generic_arg_with_effective_facts(&args[1], facts),
        ]);
    }
    // A rectangular primitive matrix keeps its ndarray carrier even when it
    // appears inside another carrier (`Option<double[][]>`, `List<double[][]>`,
    // or the outer slots of `new double[n][][]`). Rendering generic array
    // arguments recursively as Vec used to disagree with matrix construction.
    if let Some(element_ty) = crate::types::ndarray_array2_element_type(ty) {
        return RustType::Array2(Box::new(
            rust_type_for_java_generic_arg_with_effective_facts(&element_ty, facts),
        ));
    }
    if let JavaType::Array(element_ty) = ty {
        return RustType::Vec(Box::new(
            rust_type_for_java_generic_arg_with_effective_facts(
                &rust_storage_array_element_java_type(element_ty.as_ref().clone()),
                facts,
            ),
        ));
    }
    if let Some(enum_name) = closed_runtime_case_split_enum_name(ty, facts) {
        return RustType::Named(enum_name);
    }
    if let Some(HierarchyStrategy::DynTrait { trait_name }) =
        facts.object_representation_override_strategy_for(ty)
    {
        return rust_boxed_dyn_trait_type(dyn_trait_rust_path_with_effective_facts(
            trait_name, facts,
        ));
    }
    if let JavaType::Named { name, args } = ty
        && args.is_empty()
        && facts.object_safe_trait_fact_for_name(name) == Some(true)
    {
        return rust_boxed_dyn_trait_type(dyn_trait_rust_path_with_effective_facts(name, facts));
    }
    if let JavaType::Unknown(raw) = ty
        && java_type_variable_name(raw)
    {
        return erased_object_rust_type();
    }
    if erased_final_facade_generic_arg_should_keep_named_type(ty, facts)
        && let JavaType::Named { name, .. } = ty
    {
        return RustType::Named(rust_name_for_java_named_type(name));
    }
    if matches!(
        facts.object_representation_override_strategy_for(ty),
        Some(HierarchyStrategy::ErasedObject)
    ) {
        return erased_object_rust_type();
    }
    rust_type_for_java_type_with_effective_facts(ty, facts)
}

fn rust_type_for_optional_element_arg(ty: &JavaType, facts: &EffectiveFacts<'_>) -> RustType {
    let ty = crate::types::generic_read_type_ref(ty);
    if let JavaType::Named { name, args } = ty
        && args.is_empty()
        && let Some(primitive) = boxed_primitive_storage_java_type(name, facts)
    {
        return rust_type_for_java_type_with_effective_facts(
            &JavaType::Primitive(primitive),
            facts,
        );
    }
    if erased_facade_concrete_dispatch_should_keep_named_type(ty, facts)
        && let JavaType::Named { name, .. } = ty
    {
        return RustType::Named(rust_name_for_java_named_type(name));
    }
    // `Option` is the null carrier for the Java reference, not a replacement
    // for the payload's storage carrier.  In particular a nullable element of
    // `Iterator<T>`/`List<T>` must remain `Option<Rc<RefCell<T>>>` when `T`
    // has shared storage.  Calling the unwrapped renderer here used to silently
    // discard that handle and made iterator locals disagree with the collection
    // they were created from.
    rust_type_for_java_generic_arg_with_effective_facts(ty, facts)
}

fn rust_storage_sequence_element_java_type(
    container_ty: &JavaType,
    element_ty: JavaType,
    facts: &EffectiveFacts<'_>,
) -> JavaType {
    if matches!(container_ty, JavaType::Array(_)) {
        rust_storage_array_element_java_type(element_ty)
    } else {
        rust_storage_collection_element_java_type(element_ty, facts)
    }
}

fn rust_storage_collection_element_java_type(
    element_ty: JavaType,
    facts: &EffectiveFacts<'_>,
) -> JavaType {
    if let Some(optional_element) = optional_element_type(&element_ty) {
        return crate::types::optional_java_type(rust_storage_collection_element_java_type(
            optional_element,
            facts,
        ));
    }
    if let JavaType::Named { name, args } = &element_ty
        && args.is_empty()
        && let Some(primitive) = boxed_primitive_storage_java_type(name, facts)
    {
        return JavaType::Primitive(primitive);
    }
    element_ty
}

fn boxed_primitive_storage_java_type(
    name: &str,
    facts: &EffectiveFacts<'_>,
) -> Option<PrimitiveType> {
    let simple = name.rsplit('.').next().unwrap_or(name);
    let primitive = crate::stdlib::boxed_primitive_type(simple)?;
    if name.starts_with("java.lang.") {
        return Some(primitive);
    }

    let java_ty = JavaType::Named {
        name: name.to_string(),
        args: Vec::new(),
    };
    if facts
        .type_capability_overrides_for(&java_ty)
        .any(|fact| fact.provenance == FactProvenance::ParsedSource)
    {
        return None;
    }
    if !name.contains('.') && !name.contains('$') {
        // Unqualified wrapper names are the implicitly imported java.lang
        // types unless resolution recorded a parsed source declaration above.
        // Requiring a separate built-in capability fact here made nullable
        // method/constructor parameters such as Integer render as
        // Option<Box<dyn Any>>, even though direct and collection slots already
        // use the scalar carrier.
        return Some(primitive);
    }
    facts
        .type_capability_overrides_for(&java_ty)
        .any(|fact| fact.provenance == FactProvenance::BuiltinExternalModel)
        .then_some(primitive)
}

fn rust_storage_array_element_java_type(element_ty: JavaType) -> JavaType {
    if reference_array_element_needs_nullable_carrier(&element_ty) {
        crate::types::optional_java_type(element_ty)
    } else {
        element_ty
    }
}

fn reference_array_element_needs_nullable_carrier(element_ty: &JavaType) -> bool {
    !matches!(
        element_ty,
        JavaType::Primitive(_) | JavaType::Void | JavaType::Array(_)
    ) && optional_element_type(element_ty).is_none()
}

pub(crate) fn rust_type_for_java_generic_arg_with_representations(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> RustType {
    let ty = crate::types::generic_read_type_ref(ty);
    if let Some(instantiated) = raw_generic_instantiation_for_rendering(ty, representations) {
        return rust_type_for_java_generic_arg_with_representations(
            &instantiated,
            facts,
            representations,
        );
    }
    if let Some(carrier) = map_entry_rust_type_with_representations(ty, facts, representations) {
        return carrier;
    }
    if let Some(element_ty) = optional_element_type(ty) {
        return RustType::Option(Box::new(
            rust_type_for_optional_element_arg_with_representations(
                &element_ty,
                facts,
                representations,
            ),
        ));
    }
    if let Some(ty) =
        rust_type_for_external_generic_facade_with_representations(ty, facts, representations)
    {
        return ty;
    }
    if builtin_or_conservative_erased_external_representation(ty, representations)
        && let JavaType::Named { name, .. } | JavaType::Unknown(name) = ty
    {
        return RustType::Named(rust_name_for_java_named_type(name));
    }
    if let Some(dispatch_ty) =
        rust_type_for_facade_aware_representation_dispatch(ty, facts, representations)
    {
        return dispatch_ty;
    }
    if let Some(storage_ty) =
        rust_type_for_representation_storage_placement(ty, facts, representations)
    {
        return storage_ty;
    }
    if matches!(ty, JavaType::Array(_))
        || matches!(ty, JavaType::Named { args, .. } if !args.is_empty())
    {
        // The outer carrier has been selected; its children must use the same
        // representation plan. The facts-only fallback recursively loses
        // source interface and hierarchy representations at the next level.
        return rust_type_for_java_type_with_effective_facts_and_representations(
            ty,
            *facts,
            representations,
        );
    }
    rust_type_for_java_generic_arg_with_effective_facts(ty, facts)
}

fn rust_type_for_external_generic_facade_with_representations(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> Option<RustType> {
    let JavaType::Named { name, args } = ty else {
        return None;
    };
    let arity = crate::stdlib::external_facade_generic_carrier_arity(name)?;
    if args.len() != arity {
        return None;
    }
    Some(rust_generic_type(
        rust_name_for_java_named_type(name),
        args.iter()
            .map(|arg| {
                rust_type_for_java_generic_arg_with_representations(arg, facts, representations)
            })
            .collect(),
    ))
}

fn map_entry_rust_type_with_representations(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> Option<RustType> {
    let JavaType::Named { name, args } = ty else {
        return None;
    };
    if args.len() != 2
        || !matches!(
            crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
            "Entry" | "Map.Entry" | "java.util.Map.Entry"
        )
    {
        return None;
    }
    // Source declarations and explicit user representations take precedence
    // over the default platform-entry carrier, as at other native boundaries.
    if representations
        .type_representation(name)
        .is_some_and(|entry| {
            matches!(
                entry.provenance(),
                RepresentationProvenance::ParsedSource | RepresentationProvenance::UserOverride
            )
        })
    {
        return None;
    }
    let args = args
        .iter()
        .map(|arg| rust_type_for_java_generic_arg_with_representations(arg, facts, representations))
        .collect();
    Some(if representations.source_map_entry_uses_value_carrier() {
        rust_generic_type("JavaMapEntryValue", args)
    } else {
        RustType::Tuple(args)
    })
}

fn rust_type_for_optional_element_arg_with_representations(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> RustType {
    let ty = crate::types::generic_read_type_ref(ty);
    if let Some(instantiated) = raw_generic_instantiation_for_rendering(ty, representations) {
        return rust_type_for_optional_element_arg_with_representations(
            &instantiated,
            facts,
            representations,
        );
    }
    if let JavaType::Named { name, args } = ty
        && args.is_empty()
        && let Some(primitive) = boxed_primitive_storage_java_type(name, facts)
    {
        return rust_type_for_java_type_with_effective_facts(
            &JavaType::Primitive(primitive),
            facts,
        );
    }
    if erased_facade_concrete_dispatch_should_keep_named_type(ty, facts)
        && let JavaType::Named { name, .. } = ty
    {
        return RustType::Named(rust_name_for_java_named_type(name));
    }
    // Nullability wraps the Java reference representation selected for the
    // payload. A closed/source hierarchy therefore stores
    // `Option<HierarchyValue>`, not `Option<Arc<Base>>` (or another concrete
    // root payload handle). Storage placement remains authoritative for
    // concrete classes, but hierarchy dispatch must be consulted first.
    if matches!(
        representations.dispatch_for(ty),
        DispatchShape::ClosedEnum { .. }
    ) && let Some(dispatch_ty) =
        rust_type_for_facade_aware_representation_dispatch(ty, facts, representations)
    {
        return dispatch_ty;
    }
    if let Some(storage_ty) =
        rust_type_for_representation_storage_placement(ty, facts, representations)
    {
        return storage_ty;
    }
    rust_type_for_java_generic_arg_with_representations(ty, facts, representations)
}

fn raw_generic_instantiation_for_rendering(
    ty: &JavaType,
    representations: &RepresentationPlan,
) -> Option<JavaType> {
    let JavaType::Named { name, args } = ty else {
        return None;
    };
    if !args.is_empty() {
        return None;
    }
    let erased_args = representations.raw_generic_arguments_for(name)?;
    (!erased_args.is_empty()).then(|| JavaType::Named {
        name: name.clone(),
        args: erased_args.to_vec(),
    })
}

fn rust_type_for_representation_storage_placement(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> Option<RustType> {
    let JavaType::Named { name, .. } = ty else {
        return None;
    };
    let placement = crate::storage_strategy::effective_storage_placement(
        facts.storage_placement_for(name),
        representations.storage_placement_for(name),
    );
    if !placement.is_implemented() || !placement.shared() {
        return None;
    }
    Some(storage_placement_type(
        placement,
        rust_type_for_java_type_with_effective_facts_and_representations(
            ty,
            *facts,
            representations,
        ),
    ))
}

fn rust_type_for_representation_dispatch(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> Option<RustType> {
    match representations.dispatch_for(ty) {
        DispatchShape::ClosedEnum { enum_name } => Some(RustType::Named(
            parameterized_closed_enum_type_name_with_effective_representations(
                &enum_name,
                ty,
                facts,
                representations,
            ),
        )),
        DispatchShape::ErasedObject => Some(erased_object_rust_type()),
        DispatchShape::ReferenceDynTrait { trait_name, sync } => {
            Some(rust_reference_dyn_trait_type(
                parameterized_dyn_trait_path(&trait_name, ty, facts, representations),
                sync,
            ))
        }
        DispatchShape::BoxedDynTrait { trait_name } => Some(rust_boxed_dyn_trait_type(
            parameterized_dyn_trait_path(&trait_name, ty, facts, representations),
        )),
        DispatchShape::DynTrait { trait_name } => Some(RustType::DynTrait(
            parameterized_dyn_trait_path(&trait_name, ty, facts, representations),
        )),
        DispatchShape::Concrete => None,
    }
}

/// A source hierarchy carrier has the same generic identity as its Java root.
/// Dispatch keeps the bare carrier name for expression paths (`Value::Case`),
/// while slot types retain the arguments (`Value<T>`). Keeping this projection
/// here prevents declarations, parameters, fields, and returns from inventing
/// their own carrier generic spelling.
fn nominal_dispatch_type(mut java_ty: &JavaType) -> &JavaType {
    // Dispatch has already selected a carrier from this bound. Retain its
    // complete nominal identity when rendering, without changing wildcard
    // variance or claiming that arbitrary reads/writes are permitted.
    while let JavaType::Wildcard(Some(
        crate::java_ir::WildcardBound::Extends(bound) | crate::java_ir::WildcardBound::Super(bound),
    )) = java_ty
    {
        java_ty = bound;
    }
    java_ty
}

pub(crate) fn parameterized_closed_enum_type_name(enum_name: &str, java_ty: &JavaType) -> String {
    parameterized_closed_enum_type_name_with_arg_renderer(enum_name, java_ty, |arg| {
        crate::emit::emit_type(&rust_type_for_java_type(arg))
    })
}

pub(crate) fn parameterized_closed_enum_type_name_with_facts(
    enum_name: &str,
    java_ty: &JavaType,
    facts: &FactSet,
) -> String {
    let effective = EffectiveFacts::new(facts);
    parameterized_closed_enum_type_name_with_arg_renderer(enum_name, java_ty, |arg| {
        crate::emit::emit_type(&rust_type_for_java_generic_arg_with_effective_facts(
            arg, &effective,
        ))
    })
}

pub(crate) fn parameterized_closed_enum_type_name_with_representations(
    enum_name: &str,
    java_ty: &JavaType,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> String {
    parameterized_closed_enum_type_name_with_effective_representations(
        enum_name,
        java_ty,
        &EffectiveFacts::new(facts),
        representations,
    )
}

fn parameterized_closed_enum_type_name_with_effective_representations(
    enum_name: &str,
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> String {
    let java_ty = nominal_dispatch_type(java_ty);
    if representations
        .existential_interface_carrier_for(java_ty)
        .is_some()
    {
        // The enum variants, rather than generic parameters on the enum,
        // retain each existential interface instantiation.
        return enum_name.to_string();
    }
    let java_ty = match java_ty {
        JavaType::Named { name, args } if args.is_empty() => representations
            .raw_generic_arguments_for(name)
            .filter(|args| !args.is_empty())
            .map(|args| JavaType::Named {
                name: name.clone(),
                args: args.to_vec(),
            })
            .unwrap_or_else(|| java_ty.clone()),
        _ => java_ty.clone(),
    };
    let JavaType::Named { name, args } = &java_ty else {
        return enum_name.to_string();
    };
    if args.is_empty() {
        return enum_name.to_string();
    }
    let args = args
        .iter()
        .enumerate()
        .map(|(index, arg)| {
            if matches!(arg, JavaType::Wildcard(None))
                && representations
                    .erased_wildcard_bound_carrier_for(name, index)
                    .is_some()
            {
                return "JavaErasedValue".to_string();
            }
            let effective = match arg {
                JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(bound)))
                | JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(bound))) => {
                    bound.as_ref()
                }
                _ => arg,
            };
            crate::emit::emit_type(&rust_type_for_source_generic_argument(
                name,
                index,
                effective,
                facts,
                representations,
            ))
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{enum_name}<{args}>")
}

fn parameterized_closed_enum_type_name_with_arg_renderer(
    enum_name: &str,
    java_ty: &JavaType,
    mut render_arg: impl FnMut(&JavaType) -> String,
) -> String {
    let java_ty = nominal_dispatch_type(java_ty);
    let JavaType::Named { args, .. } = java_ty else {
        return enum_name.to_string();
    };
    if args.is_empty() {
        return enum_name.to_string();
    }
    let args = args
        .iter()
        .map(|arg| {
            // A bounded Java wildcard still carries a useful type identity at
            // a nominal Rust carrier boundary. In particular, `Base<? super
            // T>` can invoke members accepting `T`; erasing it to `Any`
            // destroys that contract and makes `Base<T>` needlessly
            // incompatible with the call. Reuse the declared bound just as
            // type-parameter bound projection does elsewhere.
            let effective = match arg {
                JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(bound)))
                | JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(bound))) => {
                    bound.as_ref()
                }
                _ => arg,
            };
            render_arg(effective)
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{enum_name}<{args}>")
}

/// A nullable slot retains its payload's dispatch. Render the constructor
/// selected by that dispatch, not the surrounding nullability carrier. A
/// genuine Optional trait still owns its own arguments and must not be peeled.
fn dyn_trait_dispatch_type<'a>(trait_name: &str, java_ty: &'a JavaType) -> &'a JavaType {
    let mut nominal = nominal_dispatch_type(java_ty);
    loop {
        let JavaType::Named { name, args } = nominal else {
            return nominal;
        };
        if crate::stdlib::canonical_external_receiver_type_key(trait_name)
            == crate::stdlib::canonical_external_receiver_type_key(name)
        {
            return nominal;
        }
        if !matches!(name.as_str(), "Optional" | "java.util.Optional") || args.len() != 1 {
            return nominal;
        }
        nominal = nominal_dispatch_type(&args[0]);
    }
}

pub(crate) fn parameterized_dyn_trait_path(
    trait_name: &str,
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> String {
    let java_ty = dyn_trait_dispatch_type(trait_name, java_ty);
    let trait_path = dyn_trait_rust_path_with_effective_facts(trait_name, facts);
    let JavaType::Named { name, args } = java_ty else {
        return trait_path;
    };
    if args.is_empty() {
        return trait_path;
    }
    let args = args
        .iter()
        .enumerate()
        .map(|(index, arg)| {
            if matches!(arg, JavaType::Wildcard(None))
                && representations
                    .erased_wildcard_bound_carrier_for(name, index)
                    .is_some()
            {
                return "JavaErasedValue".to_string();
            }
            let arg = match arg {
                JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(bound)))
                    if facts.generic_parameter_is_input_only(name, index) =>
                {
                    bound.as_ref()
                }
                _ => arg,
            };
            crate::emit::emit_type(&rust_type_for_source_generic_argument(
                name,
                index,
                arg,
                facts,
                representations,
            ))
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{trait_path}<{args}>")
}

fn rust_type_for_source_generic_argument(
    owner: &str,
    index: usize,
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> RustType {
    let rendered =
        rust_type_for_java_generic_arg_with_representations(java_ty, facts, representations);
    let Some(required) = representations
        .source_generic_allocations
        .argument(owner, index)
    else {
        return rendered;
    };
    let mut shape = RustSlotShape::for_storage(java_ty.clone());
    shape.native_allocation = required.native_allocation;
    shape.nested_native_allocations = required.nested_native_allocations.clone();
    fn wrap(shape: &RustSlotShape, facts: &EffectiveFacts<'_>, rendered: RustType) -> RustType {
        if let RustType::Option(inner) = rendered {
            let mut element = shape.clone();
            element.java_ty = shape
                .optional_element_type()
                .unwrap_or_else(|| shape.java_ty.clone());
            return RustType::Option(Box::new(wrap(&element, facts, *inner)));
        }
        let rendered = project_nested_native_allocations(shape, facts, rendered);
        native_allocation_slot_rust_type(shape, facts, |_| rendered.clone()).unwrap_or(rendered)
    }
    wrap(&shape, facts, rendered)
}

pub(crate) fn parameterized_dyn_trait_path_with_facts(
    trait_name: &str,
    java_ty: &JavaType,
    facts: &FactSet,
) -> String {
    parameterized_dyn_trait_path_with_effective_facts(
        trait_name,
        java_ty,
        &EffectiveFacts::new(facts),
    )
}

fn parameterized_dyn_trait_path_with_effective_facts(
    trait_name: &str,
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> String {
    let java_ty = dyn_trait_dispatch_type(trait_name, java_ty);
    let trait_path = dyn_trait_rust_path_with_effective_facts(trait_name, facts);
    let JavaType::Named { name, args } = java_ty else {
        return trait_path;
    };
    if args.is_empty() {
        return trait_path;
    }
    let args = args
        .iter()
        .enumerate()
        .map(|(index, arg)| {
            let arg = match arg {
                JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(bound)))
                    if facts.generic_parameter_is_input_only(name, index) =>
                {
                    bound.as_ref()
                }
                _ => arg,
            };
            crate::emit::emit_type(&rust_type_for_java_generic_arg_with_effective_facts(
                arg, facts,
            ))
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{trait_path}<{args}>")
}

fn rust_type_for_facade_aware_representation_dispatch(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> Option<RustType> {
    if builtin_or_conservative_erased_external_representation(ty, representations) {
        return None;
    }
    let dispatch = representations.dispatch_for(ty);
    match dispatch {
        DispatchShape::ReferenceDynTrait { trait_name, sync } => {
            Some(rust_reference_dyn_trait_type(
                parameterized_dyn_trait_path(&trait_name, ty, facts, representations),
                sync,
            ))
        }
        DispatchShape::DynTrait { trait_name } => Some(rust_boxed_dyn_trait_type(
            parameterized_dyn_trait_path(&trait_name, ty, facts, representations),
        )),
        _ => rust_type_for_representation_dispatch(ty, facts, representations),
    }
}

fn closed_runtime_case_split_enum_name(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> Option<String> {
    let JavaType::Named { name, args } = ty else {
        return None;
    };
    if !args.is_empty() || facts.closed_hierarchy_fact_for_name(name) != Some(true) {
        return None;
    }
    let requirements = facts.dispatch_requirements_for(name);
    if !requirements.iter().any(|requirement| {
        matches!(
            requirement,
            DispatchRequirementKind::NeedsClosedRuntimeCaseSplit
                | DispatchRequirementKind::NeedsInstanceof
                | DispatchRequirementKind::NeedsDowncast
                | DispatchRequirementKind::NeedsUnknownRuntimeObject
        )
    }) {
        return None;
    }
    let permitted = facts.sealed_permitted_subtypes_for(name)?.into_value();
    if permitted.is_empty()
        || !permitted.iter().all(|subtype| {
            facts.is_final_type(&JavaType::Named {
                name: subtype.clone(),
                args: Vec::new(),
            })
        })
    {
        return None;
    }
    Some(format!("{name}Value"))
}

fn java_type_variable_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit())
}

pub(crate) fn render_native_java_rust_type_policy_with_effective_facts(
    policy: &NativeJavaRustTypePolicy,
    facts: &EffectiveFacts<'_>,
) -> RustType {
    match policy {
        NativeJavaRustTypePolicy::Sequence {
            container,
            element_ty,
        } => {
            let element_ty = rust_type_for_java_generic_arg_with_effective_facts(element_ty, facts);
            match container {
                NativeSequenceContainer::Vec => RustType::Vec(Box::new(element_ty)),
                NativeSequenceContainer::HashFreeSet => RustType::Vec(Box::new(element_ty)),
                NativeSequenceContainer::HashSet => {
                    rust_generic_type("std::collections::HashSet", vec![element_ty])
                }
                NativeSequenceContainer::BTreeSet => {
                    rust_generic_type("std::collections::BTreeSet", vec![element_ty])
                }
                NativeSequenceContainer::LinkedHashSet => {
                    rust_generic_type("JavaLinkedHashSet", vec![element_ty])
                }
            }
        }
        NativeJavaRustTypePolicy::Optional { element_ty } => RustType::Option(Box::new(
            rust_type_for_optional_element_arg(element_ty, facts),
        )),
        NativeJavaRustTypePolicy::Direct { ty } => ty.clone(),
        NativeJavaRustTypePolicy::Named { name } => RustType::Named(name.clone()),
        NativeJavaRustTypePolicy::WildcardObject => erased_object_rust_type(),
        NativeJavaRustTypePolicy::UnknownRaw { raw } => rust_type_arg_projection_or_raw(raw),
        NativeJavaRustTypePolicy::ArrayRequiresSequenceElement => {
            unreachable!("arrays are handled by sequence_element_type")
        }
    }
}

pub(crate) fn java_set_uses_hash_free_carrier(ty: &JavaType, facts: &FactSet) -> bool {
    let effective_facts = EffectiveFacts::new(facts);
    java_set_uses_hash_free_carrier_with_effective_facts(ty, &effective_facts)
}

pub(crate) fn java_set_uses_hash_free_carrier_with_representations(
    ty: &JavaType,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> bool {
    let effective_facts = EffectiveFacts::new(facts);
    java_set_uses_hash_free_carrier_with_effective_facts(ty, &effective_facts)
        || java_set_element_uses_hash_free_carrier_with_representations(
            java_set_element_type_or_erased(ty).as_ref(),
            &effective_facts,
            representations,
        )
}

pub(crate) fn java_set_uses_semantic_equality_carrier_with_representations(
    ty: &JavaType,
    representations: &RepresentationPlan,
) -> bool {
    java_set_element_type_or_erased(ty).is_some_and(|element_ty| {
        java_set_element_uses_semantic_equality_carrier(&element_ty, representations)
    })
}

fn java_set_element_uses_semantic_equality_carrier(
    element_ty: &JavaType,
    representations: &RepresentationPlan,
) -> bool {
    let optional_element = optional_element_type(element_ty);
    let element_ty = optional_element.as_ref().unwrap_or(element_ty);
    matches!(
        representations.dispatch_for(element_ty),
        DispatchShape::ClosedEnum { .. }
    ) || source_element_has_shared_reference_carrier(element_ty, representations)
}

fn source_element_has_shared_reference_carrier(
    ty: &JavaType,
    representations: &RepresentationPlan,
) -> bool {
    let JavaType::Named { name, .. } = ty else {
        return false;
    };
    representations
        .type_representation(name)
        .is_some_and(|planned| {
            matches!(
                planned.provenance(),
                RepresentationProvenance::ParsedSource | RepresentationProvenance::CorpusAnalysis
            ) && representations.storage_placement_for(name).shared()
        })
}

pub(crate) fn java_set_uses_hash_free_carrier_with_effective_facts(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> bool {
    let JavaType::Named { args, .. } = ty else {
        return false;
    };
    if !matches!(
        crate::stdlib::java_collection_kind_for_type(ty),
        Some(
            crate::stdlib::JavaCollectionKind::Set
                | crate::stdlib::JavaCollectionKind::SortedSet
                | crate::stdlib::JavaCollectionKind::InsertionOrderedSet
        )
    ) {
        return false;
    }
    let Some(element_ty) = args.first() else {
        return false;
    };
    java_set_element_uses_hash_free_carrier(&element_ty, facts)
}

fn java_set_element_type_or_erased(ty: &JavaType) -> Option<JavaType> {
    let JavaType::Named { args, .. } = ty else {
        return None;
    };
    if !matches!(
        crate::stdlib::java_collection_kind_for_type(ty),
        Some(
            crate::stdlib::JavaCollectionKind::Set
                | crate::stdlib::JavaCollectionKind::SortedSet
                | crate::stdlib::JavaCollectionKind::InsertionOrderedSet
        )
    ) {
        return None;
    }
    args.first().cloned()
}

fn java_set_element_uses_hash_free_carrier(
    element_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> bool {
    if matches!(element_ty, JavaType::String | JavaType::Primitive(_)) {
        return false;
    }
    let representation_policy =
        crate::signature::signature_boundary_representation_policy_with_effective_facts(
            element_ty, facts,
        );
    representation_policy.preserve_object_representation()
        && !external_final_facade_should_keep_named_type(element_ty, facts)
        && !crate::javalibs::generic_facade_interface_erases_to_named_rust_type(element_ty)
}

fn java_set_element_uses_hash_free_carrier_with_representations(
    element_ty: Option<&JavaType>,
    facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> bool {
    let Some(element_ty) = element_ty else {
        return false;
    };
    let optional_element = optional_element_type(element_ty);
    let element_ty = optional_element.as_ref().unwrap_or(element_ty);
    if builtin_or_conservative_erased_external_representation(element_ty, representations) {
        return false;
    }
    java_set_element_uses_hash_free_carrier(element_ty, facts)
        || rust_type_for_representation_dispatch(element_ty, facts, representations).is_some()
        || source_element_has_shared_reference_carrier(element_ty, representations)
}

pub(crate) fn builtin_or_conservative_erased_external_representation(
    ty: &JavaType,
    representations: &RepresentationPlan,
) -> bool {
    let type_name = match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        _ => return false,
    };
    let Some(representation) = representations.type_representation(type_name) else {
        return false;
    };
    matches!(
        representations.dispatch_for(ty),
        DispatchShape::ErasedObject
    ) && matches!(
        representation.provenance(),
        RepresentationProvenance::BuiltinExternalModel
            | RepresentationProvenance::ConservativeFallback
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nullable_string_views_are_sized_in_both_renderers() {
        let parsed = crate::parser::parse_java_source("class Owner {}");
        let facts = FactSet::default();
        let representations = crate::representation::plan_representations(&parsed.program, &facts);
        for ownership in [
            OwnershipShape::Owned,
            OwnershipShape::CopyValue,
            OwnershipShape::SharedBorrow,
            OwnershipShape::MutableBorrow,
        ] {
            let mut shape =
                RustSlotShape::for_storage(crate::types::optional_java_type(JavaType::String));
            shape.ownership = ownership;
            shape.view = ViewShape::Str;
            let expected = match ownership {
                OwnershipShape::Owned | OwnershipShape::CopyValue => "Option<String>",
                OwnershipShape::SharedBorrow => "Option<&str>",
                OwnershipShape::MutableBorrow => "Option<&mut str>",
            };
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_slot_with_facts(&shape, &facts)),
                expected
            );
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_slot_with_facts_and_representations(
                    &shape,
                    &facts,
                    &representations
                )),
                expected
            );
        }
    }

    #[test]
    fn generic_carrier_borrow_keeps_rc_arc_and_nullable_outer_order() {
        let parsed = crate::parser::parse_java_source("class Packet {}");
        for threaded in [false, true] {
            let mut facts = FactSet::default();
            facts.storage_strategies.insert(
                "Packet".into(),
                crate::storage_strategy::TypeStorageFacts::default()
                    .with_locally_aliased(true)
                    .with_mutated(true)
                    .with_threaded(threaded),
            );
            let representations =
                crate::representation::plan_representations(&parsed.program, &facts);
            for mutable in [false, true] {
                for optional in [false, true] {
                    let java_ty = crate::parser::parse_hierarchy_type("Packet");
                    let mut shape = RustSlotShape::for_storage(if optional {
                        crate::types::optional_java_type(java_ty)
                    } else {
                        java_ty
                    });
                    shape.ownership = if mutable {
                        OwnershipShape::MutableBorrow
                    } else {
                        OwnershipShape::SharedBorrow
                    };
                    shape.borrow_target = crate::slot_shape::BorrowTarget::Carrier;
                    let value = facts.storage_placement_for("Packet").wrap("Packet".into());
                    let mut expected = format!("&{}{value}", if mutable { "mut " } else { "" });
                    if optional {
                        expected = format!("Option<{expected}>");
                    }
                    assert_eq!(
                        crate::emit::emit_type(&rust_type_for_slot_with_facts(&shape, &facts)),
                        expected
                    );
                    assert_eq!(
                        crate::emit::emit_type(&rust_type_for_slot_with_facts_and_representations(
                            &shape,
                            &facts,
                            &representations
                        )),
                        expected
                    );
                }
            }
        }
    }

    #[test]
    fn trait_dispatch_arguments_ignore_only_the_outer_nullable_carrier() {
        let facts = FactSet::default();
        let effective = EffectiveFacts::new(&facts);
        let representations = RepresentationPlan::default();
        let parse = crate::parser::parse_hierarchy_type;
        for (trait_name, spelling, expected) in [
            (
                "Pair",
                "Optional<Pair<String, Integer>>",
                "Pair<String, i32>",
            ),
            (
                "Pair",
                "java.util.Optional<Pair<String, Integer>>",
                "Pair<String, i32>",
            ),
            (
                "Pair",
                "Optional<Optional<Pair<String, Integer>>>",
                "Pair<String, i32>",
            ),
            ("Optional", "java.util.Optional<String>", "Optional<String>"),
        ] {
            let ty = parse(spelling);
            assert_eq!(
                parameterized_dyn_trait_path_with_facts(trait_name, &ty, &facts),
                expected
            );
            assert_eq!(
                parameterized_dyn_trait_path(trait_name, &ty, &effective, &representations),
                expected
            );
        }
        for spelling in ["left.Optional<String>", "Pair<Optional<String>, Integer>"] {
            let payload = parse(spelling);
            let trait_name = if spelling.starts_with("left.") {
                "left.Optional"
            } else {
                "Pair"
            };
            let nullable = crate::types::optional_java_type(payload.clone());
            assert_eq!(
                parameterized_dyn_trait_path(trait_name, &nullable, &effective, &representations),
                parameterized_dyn_trait_path(trait_name, &payload, &effective, &representations),
            );
        }
    }

    #[test]
    fn nested_callback_slots_select_their_canonical_synchronized_facades() {
        let facts = FactSet::default();
        let representations = RepresentationPlan::default();
        let callback = JavaType::Named {
            name: "IntConsumer".into(),
            args: vec![],
        };
        let mut array = RustSlotShape::for_storage(JavaType::Array(Box::new(callback.clone())));
        array.require_native_allocation_at(&[0], StoragePlacement::new(true, true, true));
        let mut function = RustSlotShape::for_storage(JavaType::Named {
            name: "Function".into(),
            args: vec![JavaType::String, callback],
        });
        function.require_native_allocation_at(&[1], StoragePlacement::new(true, true, true));
        for (shape, expected) in [
            (array, "Vec<Option<__JavaSyncIntConsumer>>"),
            (function, "Function<String, __JavaSyncIntConsumer>"),
        ] {
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_slot_with_facts(&shape, &facts)),
                expected
            );
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_slot_with_facts_and_representations(
                    &shape,
                    &facts,
                    &representations,
                )),
                expected
            );
        }
    }

    #[test]
    fn retained_list_defaults_match_local_sync_and_nullable_slot_payloads() {
        let facts = FactSet::default();
        for name in [
            "java.util.List",
            "java.util.ArrayList",
            "java.util.LinkedList",
        ] {
            for sync in [false, true] {
                let mut shape = RustSlotShape::for_storage(JavaType::Named {
                    name: name.into(),
                    args: vec![JavaType::Unknown("T".into())],
                });
                shape.native_allocation = StoragePlacement::new(true, true, sync);
                let rendered =
                    crate::emit::emit_expr(&default_expr_for_slot_with_facts(&shape, &facts));
                let runtime = if sync {
                    "JavaSyncNativeCollection"
                } else {
                    "JavaNativeCollection"
                };
                let constructor = if name.ends_with("LinkedList") {
                    format!("{runtime}::list_with_empty_add_policy(Vec::new(), true, false)")
                } else {
                    format!("{runtime}::list(Vec::new(), true)")
                };
                let expected = if sync {
                    format!("std::sync::Arc::new(std::sync::Mutex::new({constructor}))")
                } else {
                    format!("std::rc::Rc::new(std::cell::RefCell::new({constructor}))")
                };
                assert_eq!(rendered, expected);
                shape.java_ty = crate::types::optional_java_type(shape.java_ty);
                assert_eq!(
                    crate::emit::emit_expr(&default_expr_for_slot_with_facts(&shape, &facts)),
                    "Default::default()"
                );
            }
        }
        let mut array = RustSlotShape::for_storage(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Int,
        ))));
        array.native_allocation = StoragePlacement::new(true, true, false);
        assert_eq!(
            crate::emit::emit_expr(&default_expr_for_slot_with_facts(&array, &facts)),
            "std::rc::Rc::new(std::cell::RefCell::new(Vec::new()))"
        );
    }

    #[test]
    fn nullable_covariant_native_children_keep_their_solved_allocation() {
        let facts = FactSet::default();
        let representations = RepresentationPlan::default();
        for sync in [false, true] {
            let row = JavaType::Named {
                name: "List".into(),
                args: vec![JavaType::Primitive(PrimitiveType::Int)],
            };
            let row = crate::types::optional_java_type(JavaType::Wildcard(Some(
                crate::java_ir::WildcardBound::Extends(Box::new(row)),
            )));
            let mut shape = RustSlotShape::for_storage(JavaType::Named {
                name: "List".into(),
                args: vec![row],
            });
            shape.require_native_allocation_at(&[0], StoragePlacement::new(true, true, sync));
            let expected = if sync {
                "Vec<Option<std::sync::Arc<std::sync::Mutex<JavaSyncNativeCollection<i32>>>>>"
            } else {
                "Vec<Option<std::rc::Rc<std::cell::RefCell<JavaNativeCollection<i32>>>>>"
            };
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_slot_with_facts_and_representations(
                    &shape,
                    &facts,
                    &representations,
                )),
                expected
            );
        }
    }

    #[test]
    fn retained_native_collection_allocation_payloads_share_the_runtime_without_erasing_elements() {
        let facts = FactSet::default();
        let representations = RepresentationPlan::default();
        for name in [
            "java.util.List",
            "java.util.HashSet",
            "java.util.LinkedHashSet",
        ] {
            for sync in [false, true] {
                let mut shape = RustSlotShape::for_storage(JavaType::Named {
                    name: name.into(),
                    args: vec![JavaType::Primitive(PrimitiveType::Int)],
                });
                shape.native_allocation = StoragePlacement::new(true, true, sync);
                let expected = if sync {
                    "std::sync::Arc<std::sync::Mutex<JavaSyncNativeCollection<i32>>>"
                } else {
                    "std::rc::Rc<std::cell::RefCell<JavaNativeCollection<i32>>>"
                };
                assert_eq!(
                    crate::emit::emit_type(&rust_type_for_slot_with_facts_and_representations(
                        &shape,
                        &facts,
                        &representations
                    )),
                    expected
                );
            }
        }
    }

    #[test]
    fn native_allocation_slots_keep_sized_payloads_and_outer_nullability() {
        let facts = FactSet::default();
        let representations = RepresentationPlan::default();
        let mut shape = RustSlotShape::for_storage(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Int,
        ))));
        let element = shape.java_ty.clone();
        for (ownership, nullable, sync, expected) in [
            (
                OwnershipShape::Owned,
                false,
                false,
                "std::rc::Rc<std::cell::RefCell<Vec<i32>>>",
            ),
            (
                OwnershipShape::SharedBorrow,
                false,
                false,
                "&std::rc::Rc<std::cell::RefCell<Vec<i32>>>",
            ),
            (
                OwnershipShape::MutableBorrow,
                true,
                false,
                "Option<&mut std::rc::Rc<std::cell::RefCell<Vec<i32>>>>",
            ),
            (
                OwnershipShape::Owned,
                true,
                true,
                "Option<std::sync::Arc<std::sync::Mutex<Vec<i32>>>>",
            ),
        ] {
            shape.native_allocation = StoragePlacement::new(true, true, sync);
            shape.ownership = ownership;
            shape.view = ViewShape::Slice;
            shape.java_ty = if nullable {
                crate::types::optional_java_type(element.clone())
            } else {
                element.clone()
            };
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_slot_with_facts(&shape, &facts)),
                expected
            );
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_slot_with_facts_and_representations(
                    &shape,
                    &facts,
                    &representations,
                )),
                expected
            );
        }
    }

    #[test]
    fn nested_native_collection_ignores_conservative_erased_hierarchy_entry() {
        let facts = FactSet::default();
        let mut representations = RepresentationPlan::default();
        for owner in ["Set", "List", "Map"] {
            representations.insert_test_type_representation(
                crate::representation::TypeRepresentation::new(
                    owner,
                    crate::representation::ObjectRepresentation::ErasedObject,
                    RepresentationProvenance::ConservativeFallback,
                    Vec::new(),
                ),
            );
        }
        for (java_type, expected) in [
            (
                "List<Set<String>>",
                "Vec<std::collections::HashSet<String>>",
            ),
            ("List<List<String>>", "Vec<Vec<String>>"),
            (
                "List<Map<String, Integer>>",
                "Vec<std::collections::HashMap<String, i32>>",
            ),
        ] {
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_java_type_with_facts_and_representations(
                    &crate::parser::parse_hierarchy_type(java_type),
                    &facts,
                    &representations,
                )),
                expected
            );
        }
        let set = crate::parser::parse_hierarchy_type("Set<String>");
        assert_eq!(representations.dispatch_for(&set), DispatchShape::Concrete);
        let shape = crate::shape_projection::return_shape_for_java_type(&representations, &set)
            .expect("collection return");
        assert_eq!(
            crate::emit::emit_type(&rust_type_for_slot_with_facts_and_representations(
                &shape,
                &facts,
                &representations,
            )),
            "std::collections::HashSet<String>"
        );
    }

    #[test]
    fn nested_carriers_preserve_source_interface_payload_representations() {
        let parsed = crate::parser::parse_java_source(
            "interface Port<T> { int size(); } class Packet<T> { T value; }",
        );
        assert!(!parsed.diagnostics.has_errors());
        let facts = FactSet::default();
        let representations = crate::representation::plan_representations(&parsed.program, &facts);
        let effective = EffectiveFacts::new(&facts);
        let mut failures = Vec::new();
        for (java_type, expected) in [
            (
                "List<List<Port<String>>>",
                "Vec<Vec<Box<dyn Port<String>>>>",
            ),
            (
                "Map<String, List<Port<String>>>",
                "std::collections::HashMap<String, Vec<Box<dyn Port<String>>>>",
            ),
            (
                "Optional<List<Port<String>>>",
                "Option<Vec<Box<dyn Port<String>>>>",
            ),
            (
                "List<Packet<Port<String>>>",
                "Vec<Packet<Box<dyn Port<String>>>>",
            ),
            (
                "List<Port<String>[]>",
                "Vec<Vec<Option<Box<dyn Port<String>>>>>",
            ),
            (
                "Future<List<Port<String>>>",
                "Future<Vec<Box<dyn Port<String>>>>",
            ),
            (
                "BiFunction<Port<String>, String, Port<String>>",
                "BiFunction<Box<dyn Port<String>>, String, Box<dyn Port<String>>>",
            ),
            (
                "List<Map.Entry<String, Port<String>>>",
                "Vec<(String, Box<dyn Port<String>>)>",
            ),
        ] {
            let ty = crate::parser::parse_hierarchy_type(java_type);
            for rendered in [
                rust_type_for_java_type_with_facts_and_representations(
                    &ty,
                    &facts,
                    &representations,
                ),
                rust_type_for_java_generic_arg_with_representations(
                    &ty,
                    &effective,
                    &representations,
                ),
            ] {
                let actual = crate::emit::emit_type(&rendered);
                if actual != expected {
                    failures.push(format!("{java_type}: expected {expected}, got {actual}"));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn shared_generic_payload_retains_nested_interface_representation() {
        let parsed = crate::parser::parse_java_source(
            "interface Port<T> { int size(); } class Packet<T> { T value; }",
        );
        let mut facts = FactSet::default();
        facts.storage_strategies.insert(
            "Packet".into(),
            crate::storage_strategy::TypeStorageFacts::default()
                .with_locally_aliased(true)
                .with_mutated(true),
        );
        let reps = crate::representation::plan_representations(&parsed.program, &facts);
        let ty = crate::parser::parse_hierarchy_type("List<Packet<Port<String>>>");
        let expected = format!(
            "Vec<{}>",
            facts
                .storage_placement_for("Packet")
                .wrap("Packet<Box<dyn Port<String>>>".to_string())
        );
        assert_eq!(
            crate::emit::emit_type(&rust_type_for_java_type_with_facts_and_representations(
                &ty, &facts, &reps,
            )),
            expected,
        );
    }

    #[test]
    fn explicit_entry_representations_are_not_platform_entry_tuples() {
        let parsed = crate::parser::parse_java_source(
            "class Entry<K, V> { K key; V value; } interface Port<T> { int size(); }",
        );
        let facts = FactSet::default();
        let mut reps = crate::representation::plan_representations(&parsed.program, &facts);
        let ty = crate::parser::parse_hierarchy_type("List<Entry<String, Port<String>>>");
        assert_eq!(
            crate::emit::emit_type(&rust_type_for_java_type_with_facts_and_representations(
                &ty, &facts, &reps,
            )),
            "Vec<Entry<String, Box<dyn Port<String>>>>",
        );
        reps.insert_test_type_representation(crate::representation::TypeRepresentation::new(
            "Map.Entry",
            crate::representation::ObjectRepresentation::ErasedObject,
            RepresentationProvenance::UserOverride,
            Vec::new(),
        ));
        let ty = crate::parser::parse_hierarchy_type("List<Map.Entry<String, Port<String>>>");
        assert_eq!(
            crate::emit::emit_type(&rust_type_for_java_type_with_facts_and_representations(
                &ty, &facts, &reps,
            )),
            "Vec<Box<dyn std::any::Any>>",
        );
    }

    #[test]
    fn parameterized_nested_dyn_trait_uses_canonical_rust_name() {
        let java_ty = JavaType::Named {
            name: "Outer.Inner".to_string(),
            args: vec![JavaType::Primitive(PrimitiveType::Int)],
        };

        assert_eq!(
            parameterized_dyn_trait_path_with_facts("Outer.Inner", &java_ty, &FactSet::default(),),
            "Outer_Inner<i32>"
        );
    }

    #[test]
    fn fact_only_trait_slots_preserve_declared_generic_arguments() {
        for (dispatch, ownership, expected) in [
            (
                DispatchShape::BoxedDynTrait {
                    trait_name: "Port".into(),
                },
                OwnershipShape::Owned,
                "Box<dyn Port<String>>",
            ),
            (
                DispatchShape::DynTrait {
                    trait_name: "Port".into(),
                },
                OwnershipShape::SharedBorrow,
                "&dyn Port<String>",
            ),
        ] {
            let mut shape =
                RustSlotShape::for_storage(crate::parser::parse_hierarchy_type("Port<String>"));
            shape.dispatch = dispatch;
            shape.ownership = ownership;
            for view in [ViewShape::Native, ViewShape::DynTraitView] {
                shape.view = view;
                assert_eq!(
                    crate::emit::emit_type(&rust_type_for_slot_with_facts(
                        &shape,
                        &FactSet::default()
                    )),
                    expected
                );
                let mut nullable = shape.clone();
                nullable.java_ty = crate::types::optional_java_type(nullable.java_ty);
                assert_eq!(
                    crate::emit::emit_type(&rust_type_for_slot_with_facts(
                        &nullable,
                        &FactSet::default()
                    )),
                    format!("Option<{expected}>")
                );
            }
        }
    }

    #[test]
    fn bounded_wildcard_dispatch_preserves_nominal_generic_identity() {
        let facts = FactSet::default();
        let reps = RepresentationPlan::default();
        for source in ["? super Port<String>", "? extends Port<String>"] {
            let ty = crate::parser::parse_hierarchy_type(source);
            for (dispatch, ownership, expected) in [
                (
                    DispatchShape::BoxedDynTrait {
                        trait_name: "Port".into(),
                    },
                    OwnershipShape::Owned,
                    "Box<dyn Port<String>>",
                ),
                (
                    DispatchShape::DynTrait {
                        trait_name: "Port".into(),
                    },
                    OwnershipShape::SharedBorrow,
                    "&dyn Port<String>",
                ),
            ] {
                let mut shape = RustSlotShape::for_storage(ty.clone());
                shape.dispatch = dispatch;
                shape.ownership = ownership;
                for view in [ViewShape::Native, ViewShape::DynTraitView] {
                    shape.view = view;
                    assert_eq!(
                        crate::emit::emit_type(&rust_type_for_slot_with_facts(&shape, &facts)),
                        expected,
                        "{source}"
                    );
                    assert_eq!(
                        crate::emit::emit_type(&rust_type_for_slot_with_facts_and_representations(
                            &shape, &facts, &reps
                        )),
                        expected,
                        "{source}"
                    );
                }
            }
            assert_eq!(
                parameterized_closed_enum_type_name("PortValue", &ty),
                "PortValue<String>"
            );
        }
    }

    #[test]
    fn shared_external_index_preserves_representation_aware_slot_rendering() {
        let facts = FactSet::default();
        let external_index = facts.external_lookup_index();
        let representations = RepresentationPlan::default();
        let shape = RustSlotShape::for_storage(JavaType::Named {
            name: "Example".to_string(),
            args: Vec::new(),
        });

        assert_eq!(
            rust_type_for_slot_with_facts_representations_and_external_index(
                &shape,
                &facts,
                &representations,
                &external_index,
            ),
            rust_type_for_slot_with_facts_and_representations(&shape, &facts, &representations,)
        );
    }

    #[test]
    fn rectangular_primitive_array_default_uses_array2_carrier_default() {
        let shape = RustSlotShape::for_storage(JavaType::Array(Box::new(JavaType::Array(
            Box::new(JavaType::Primitive(PrimitiveType::Double)),
        ))));

        assert_eq!(
            slot_default_value_policy_with_effective_facts(
                &shape,
                &EffectiveFacts::new(&FactSet::default()),
            ),
            SlotDefaultValuePolicy::EmptyRectangularPrimitiveArray {
                element: PrimitiveType::Double,
            }
        );
        assert_eq!(
            crate::emit::emit_expr(&default_expr_for_slot_with_facts(
                &shape,
                &FactSet::default(),
            )),
            "ndarray::Array2::from_elem((0usize, 0usize), 0f64)"
        );
    }

    #[test]
    fn raw_source_generic_closed_carrier_uses_declared_erased_bound() {
        let parsed = crate::parser::parse_java_source(
            r#"
class Item {}
abstract class Reader<T extends Item> {}
class ConcreteReader extends Reader<Item> {}
"#,
        );
        assert!(!parsed.diagnostics.has_errors());
        let representations =
            crate::representation::plan_representations(&parsed.program, &FactSet::default());
        let shape = RustSlotShape {
            native_allocation: Default::default(),
            nested_native_allocations: Default::default(),
            nested_collection_views: Default::default(),
            borrow_target: Default::default(),
            java_ty: JavaType::Named {
                name: "Reader".to_string(),
                args: Vec::new(),
            },
            ownership: OwnershipShape::Owned,
            view: ViewShape::Native,
            dispatch: DispatchShape::ClosedEnum {
                enum_name: "ReaderValue".to_string(),
            },
            mutability: crate::slot_shape::MutabilityShape::Immutable,
        };

        assert_eq!(
            crate::emit::emit_type(&rust_type_for_slot_with_facts_and_representations(
                &shape,
                &FactSet::default(),
                &representations,
            )),
            "ReaderValue<Item>"
        );
    }

    #[test]
    fn raw_source_generic_arguments_use_the_same_declared_erasure_as_outer_slots() {
        let parsed = crate::parser::parse_java_source(
            "class Item {} class Packet<T extends Item> { T value; }",
        );
        assert!(!parsed.diagnostics.has_errors());
        let representations =
            crate::representation::plan_representations(&parsed.program, &FactSet::default());
        for (java_type, expected) in [
            ("List<Packet>", "Vec<Packet<Item>>"),
            ("List<Optional<Packet>>", "Vec<Option<Packet<Item>>>"),
            (
                "Map<String, Packet>",
                "std::collections::HashMap<String, Packet<Item>>",
            ),
        ] {
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_java_type_with_facts_and_representations(
                    &crate::parser::parse_hierarchy_type(java_type),
                    &FactSet::default(),
                    &representations,
                )),
                expected,
                "{java_type}"
            );
        }
    }

    #[test]
    fn fact_only_closed_carrier_is_not_wrapped_outside_variant_handles() {
        let mut facts = FactSet::default();
        facts.storage_strategies.insert(
            "Hmm".to_string(),
            crate::storage_strategy::TypeStorageFacts::default()
                .with_locally_aliased(true)
                .with_mutated(true),
        );
        let shape = RustSlotShape {
            native_allocation: Default::default(),
            nested_native_allocations: Default::default(),
            nested_collection_views: Default::default(),
            borrow_target: Default::default(),
            java_ty: JavaType::Named {
                name: "Hmm".to_string(),
                args: vec![JavaType::Wildcard(None)],
            },
            ownership: OwnershipShape::Owned,
            view: ViewShape::Native,
            dispatch: DispatchShape::ClosedEnum {
                enum_name: "HmmValue".to_string(),
            },
            mutability: crate::slot_shape::MutabilityShape::Immutable,
        };

        assert_eq!(
            crate::emit::emit_type(&rust_type_for_slot_with_facts(&shape, &facts)),
            "HmmValue<Box<dyn std::any::Any>>"
        );
    }

    #[test]
    fn optional_array_rendering_preserves_borrowed_slice_ownership() {
        let facts = FactSet::default();
        let representations = RepresentationPlan::default();
        let optional_array = crate::types::optional_java_type(JavaType::Array(Box::new(
            JavaType::Primitive(PrimitiveType::Byte),
        )));

        for (ownership, expected) in [
            (OwnershipShape::Owned, "Option<Vec<i8>>"),
            (OwnershipShape::SharedBorrow, "Option<&[i8]>"),
            (OwnershipShape::MutableBorrow, "Option<&mut [i8]>"),
        ] {
            let mut shape = RustSlotShape::conservative(optional_array.clone());
            shape.ownership = ownership;
            shape.view = ViewShape::Slice;
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_slot_with_facts_and_representations(
                    &shape,
                    &facts,
                    &representations,
                )),
                expected
            );
        }
    }

    #[test]
    fn representation_aware_collection_slots_preserve_borrow_ownership() {
        let facts = FactSet::default();
        let representations = RepresentationPlan::default();
        let list = JavaType::Named {
            name: "java.util.List".to_string(),
            args: vec![JavaType::String],
        };

        for (ownership, expected) in [
            (OwnershipShape::Owned, "Vec<String>"),
            (OwnershipShape::SharedBorrow, "&Vec<String>"),
            (OwnershipShape::MutableBorrow, "&mut Vec<String>"),
        ] {
            let mut shape = RustSlotShape::conservative(list.clone());
            shape.ownership = ownership;
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_slot_with_facts_and_representations(
                    &shape,
                    &facts,
                    &representations,
                )),
                expected
            );
        }
    }

    #[test]
    fn nullable_matrix_nested_in_array_keeps_ndarray_payload() {
        let facts = FactSet::default();
        let matrix = JavaType::Array(Box::new(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Double,
        )))));
        let outer = JavaType::Array(Box::new(crate::types::optional_java_type(matrix)));

        assert_eq!(
            crate::emit::emit_type(&rust_type_for_java_type_with_effective_facts(
                &outer,
                &EffectiveFacts::new(&facts),
            )),
            "Vec<Option<ndarray::Array2<f64>>>"
        );
    }

    #[test]
    fn rank_three_primitive_array_keeps_nested_vec_carrier() {
        let facts = FactSet::default();
        let cube = JavaType::Array(Box::new(JavaType::Array(Box::new(JavaType::Array(
            Box::new(JavaType::Primitive(PrimitiveType::Double)),
        )))));

        assert_eq!(
            crate::emit::emit_type(&rust_type_for_java_type_with_effective_facts(
                &cube,
                &EffectiveFacts::new(&facts),
            )),
            "Vec<Vec<Vec<f64>>>"
        );
    }

    #[test]
    fn ndarray_slice_slots_apply_borrow_ownership_once() {
        let facts = FactSet::default();
        let representations = RepresentationPlan::default();
        let array2 = JavaType::Array(Box::new(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Double,
        )))));

        for (ownership, expected) in [
            (OwnershipShape::SharedBorrow, "&ndarray::Array2<f64>"),
            (OwnershipShape::MutableBorrow, "&mut ndarray::Array2<f64>"),
        ] {
            let mut shape = RustSlotShape::conservative(array2.clone());
            shape.ownership = ownership;
            shape.view = ViewShape::Slice;
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_slot_with_facts_and_representations(
                    &shape,
                    &facts,
                    &representations,
                )),
                expected
            );
        }
    }

    #[test]
    fn no_default_message_policy_stays_inside_slot_render() {
        let policy = SlotDefaultPolicy::NoDefaultForClosedEnum {
            enum_name: "AnimalValue".to_string(),
        };
        let no_default = slot_no_default_message_policy(&policy);

        assert_eq!(
            no_default,
            SlotNoDefaultMessagePolicy {
                message: "no default value is available for closed enum slot AnimalValue"
                    .to_string(),
            }
        );
        assert_eq!(
            no_default.compile_error_expr(),
            crate::unsupported::compile_error_expr(
                "no default value is available for closed enum slot AnimalValue"
            )
        );
    }

    #[test]
    fn generic_carrier_borrow_of_qualified_object_uses_the_erased_carrier() {
        let facts = FactSet::default();
        let representations = RepresentationPlan::default();
        for name in ["Object", "java.lang.Object"] {
            let mut shape =
                RustSlotShape::for_storage(crate::types::optional_java_type(JavaType::Named {
                    name: name.into(),
                    args: Vec::new(),
                }));
            shape.ownership = OwnershipShape::SharedBorrow;
            shape.borrow_target = crate::slot_shape::BorrowTarget::Carrier;
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_slot_with_facts_and_representations(
                    &shape,
                    &facts,
                    &representations,
                )),
                "Option<&Box<dyn std::any::Any>>",
            );
        }
    }

    #[test]
    fn unknown_raw_native_type_policy_recovers_simple_type_text() {
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);

        assert_eq!(
            render_native_java_rust_type_policy_with_effective_facts(
                &NativeJavaRustTypePolicy::UnknownRaw {
                    raw: "Object".to_string(),
                },
                &effective_facts,
            ),
            RustType::Named("Object".to_string())
        );
        assert_eq!(
            render_native_java_rust_type_policy_with_effective_facts(
                &NativeJavaRustTypePolicy::UnknownRaw {
                    raw: "None::<Void>".to_string(),
                },
                &effective_facts,
            ),
            RustType::Raw("None::<Void>".to_string())
        );
    }

    #[test]
    fn storage_placement_construction_builds_nested_calls_structurally() {
        let expr = storage_placement_construction_expr(
            StoragePlacement::new(true, true, false),
            RustExpr::Var("value".to_string()),
        );

        assert_eq!(
            crate::emit::emit_expr(&expr),
            "std::rc::Rc::new(std::cell::RefCell::new(value))"
        );
        assert_eq!(
            expr,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "rc", "Rc", "new"])),
                args: vec![RustExpr::Call {
                    function: Box::new(RustExpr::path(["std", "cell", "RefCell", "new"])),
                    args: vec![RustExpr::Var("value".to_string())],
                }],
            }
        );

        let already_wrapped = storage_placement_construction_expr(
            StoragePlacement::new(true, false, true),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "sync", "Arc", "new"])),
                args: vec![RustExpr::Var("value".to_string())],
            },
        );
        assert_eq!(
            already_wrapped,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "sync", "Arc", "new"])),
                args: vec![RustExpr::Var("value".to_string())],
            }
        );

        let raw_already_wrapped = storage_placement_construction_expr(
            StoragePlacement::new(true, false, false),
            RustExpr::Raw("std::rc::Rc::new(value)".to_string()),
        );
        assert_eq!(
            raw_already_wrapped,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "rc", "Rc", "new"])),
                args: vec![RustExpr::Var("value".to_string())],
            }
        );

        let generic_already_wrapped = storage_placement_construction_expr(
            StoragePlacement::new(true, false, false),
            RustExpr::Call {
                function: Box::new(RustExpr::GenericPath {
                    base: vec!["std".to_string(), "rc".to_string(), "Rc".to_string()],
                    args: vec![RustType::Named("Value".to_string())],
                    member: Some("new".to_string()),
                }),
                args: vec![RustExpr::Var("value".to_string())],
            },
        );
        assert_eq!(
            crate::emit::emit_expr(&generic_already_wrapped),
            "std::rc::Rc::<Value>::new(value)"
        );

        let opaque_raw_payload = storage_placement_construction_expr(
            StoragePlacement::new(true, false, false),
            RustExpr::Raw("left + right".to_string()),
        );
        assert_eq!(
            opaque_raw_payload,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "rc", "Rc", "new"])),
                args: vec![RustExpr::Raw("left + right".to_string())],
            }
        );
    }

    #[test]
    fn storage_placement_moves_result_projection_to_the_fallible_payload() {
        let placement = StoragePlacement::new(true, true, false);
        let wrapped = storage_placement_construction_expr(
            placement,
            RustExpr::Var("make_value()".to_string()),
        );
        let projected_after_storage = RustExpr::shaped(
            RustExprShape::ResultPropagated,
            RustExpr::Try(Box::new(wrapped)),
        );

        let normalized = storage_placement_construction_expr(placement, projected_after_storage);

        assert_eq!(
            crate::emit::emit_expr(&normalized),
            "std::rc::Rc::new(std::cell::RefCell::new(make_value()?))"
        );

        let payload_already_projected = storage_placement_construction_expr(
            placement,
            RustExpr::Try(Box::new(RustExpr::Var("make_value()".to_string()))),
        );
        let redundantly_projected = RustExpr::shaped(
            RustExprShape::ResultPropagated,
            RustExpr::Try(Box::new(payload_already_projected)),
        );
        assert_eq!(
            crate::emit::emit_expr(&storage_placement_construction_expr(
                placement,
                redundantly_projected,
            )),
            "std::rc::Rc::new(std::cell::RefCell::new(make_value()?))"
        );
    }

    #[test]
    fn storage_placement_type_builds_nested_generics_structurally() {
        let ty = storage_placement_type(
            StoragePlacement::new(true, true, false),
            RustType::Named("Value".to_string()),
        );

        assert_eq!(
            crate::emit::emit_type(&ty),
            "std::rc::Rc<std::cell::RefCell<Value>>"
        );
        assert_eq!(
            ty,
            rust_generic_type(
                "std::rc::Rc",
                vec![rust_generic_type(
                    "std::cell::RefCell",
                    vec![RustType::Named("Value".to_string())]
                )]
            )
        );
    }

    #[test]
    fn map_entry_renders_as_tuple_at_slot_and_generic_positions() {
        let entry = JavaType::Named {
            name: "java.util.Map.Entry".to_string(),
            args: vec![
                JavaType::String,
                JavaType::Primitive(crate::java_ir::PrimitiveType::Int),
            ],
        };

        assert_eq!(
            crate::emit::emit_type(&rust_type_for_java_type(&entry)),
            "(String, i32)"
        );

        let values = JavaType::Named {
            name: "java.util.List".to_string(),
            args: vec![entry],
        };
        assert_eq!(
            crate::emit::emit_type(&rust_type_for_java_type(&values)),
            "Vec<(String, i32)>"
        );
    }

    #[test]
    fn boxed_primitive_collection_elements_render_as_primitive_slots() {
        let stdlib_facts = crate::stdlib::rust_stdlib_signature_facts();
        for (java_name, expected) in [
            ("Byte", "Vec<i8>"),
            ("Short", "Vec<i16>"),
            ("Integer", "Vec<i32>"),
            ("Long", "Vec<i64>"),
            ("Float", "Vec<f32>"),
            ("Double", "Vec<f64>"),
            ("Boolean", "Vec<bool>"),
            ("Character", "Vec<JavaCodeUnit>"),
        ] {
            let values = JavaType::Named {
                name: "java.util.ArrayList".to_string(),
                args: vec![JavaType::Named {
                    name: java_name.to_string(),
                    args: Vec::new(),
                }],
            };

            assert_eq!(
                crate::emit::emit_type(&rust_type_for_java_type_with_facts(&values, &stdlib_facts)),
                expected,
                "{java_name}"
            );

            let nullable_values = JavaType::Named {
                name: "java.util.ArrayList".to_string(),
                args: vec![crate::types::optional_java_type(JavaType::Named {
                    name: java_name.to_string(),
                    args: Vec::new(),
                })],
            };
            let nullable_expected = format!(
                "Vec<Option<{}>>",
                expected.trim_start_matches("Vec<").trim_end_matches('>')
            );
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_java_type_with_facts(
                    &nullable_values,
                    &stdlib_facts
                )),
                nullable_expected,
                "nullable {java_name}"
            );

            let nullable_value = crate::types::optional_java_type(JavaType::Named {
                name: java_name.to_string(),
                args: Vec::new(),
            });
            let element_expected = expected
                .trim_start_matches("Vec<")
                .trim_end_matches('>')
                .to_string();
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_java_type_with_facts(
                    &nullable_value,
                    &stdlib_facts
                )),
                format!("Option<{element_expected}>"),
                "optional {java_name}"
            );
        }
    }

    #[test]
    fn nullable_unqualified_boxed_primitive_slot_renders_as_optional_scalar() {
        let shape = RustSlotShape {
            native_allocation: Default::default(),
            nested_native_allocations: Default::default(),
            nested_collection_views: Default::default(),
            borrow_target: Default::default(),
            java_ty: crate::types::optional_java_type(JavaType::Named {
                name: "Integer".to_string(),
                args: Vec::new(),
            }),
            ownership: OwnershipShape::Owned,
            view: ViewShape::Native,
            dispatch: DispatchShape::ErasedObject,
            mutability: crate::slot_shape::MutabilityShape::Immutable,
        };

        assert_eq!(
            rust_type_for_slot_with_facts_and_representations(
                &shape,
                &FactSet::default(),
                &RepresentationPlan::default(),
            ),
            RustType::Option(Box::new(RustType::I32))
        );
    }

    #[test]
    fn nullable_borrowed_boxed_trait_storage_renders_as_trait_view() {
        let shape = RustSlotShape {
            native_allocation: Default::default(),
            nested_native_allocations: Default::default(),
            nested_collection_views: Default::default(),
            borrow_target: Default::default(),
            java_ty: crate::types::optional_java_type(JavaType::Named {
                name: "BlockData".to_string(),
                args: Vec::new(),
            }),
            ownership: OwnershipShape::SharedBorrow,
            view: ViewShape::DynTraitView,
            dispatch: DispatchShape::BoxedDynTrait {
                trait_name: "BlockData".to_string(),
            },
            mutability: crate::slot_shape::MutabilityShape::Immutable,
        };

        assert_eq!(
            crate::emit::emit_type(&rust_type_for_slot_with_facts_and_representations(
                &shape,
                &FactSet::default(),
                &RepresentationPlan::default(),
            )),
            "Option<&dyn BlockData>"
        );
    }

    #[test]
    fn source_type_named_like_boxed_primitive_keeps_source_identity() {
        let mut facts = crate::stdlib::rust_stdlib_signature_facts();
        facts
            .type_capability_overrides
            .push(crate::facts::TypeCapabilityOverride {
                type_name: "Byte".to_string(),
                copy: None,
                clone: None,
                string_like: None,
                sequence_like: None,
                implements: Vec::new(),
                extends: None,
                final_type: Some(false),
                object_safe_trait: None,
                provenance: FactProvenance::ParsedSource,
            });

        let source_byte = JavaType::Named {
            name: "Byte".to_string(),
            args: Vec::new(),
        };
        let nullable_source_byte = crate::types::optional_java_type(source_byte.clone());
        let values = JavaType::Named {
            name: "java.util.ArrayList".to_string(),
            args: vec![nullable_source_byte.clone()],
        };

        assert_eq!(
            crate::emit::emit_type(&rust_type_for_java_type_with_facts(
                &nullable_source_byte,
                &facts
            )),
            "Option<Byte>"
        );
        assert_eq!(
            crate::emit::emit_type(&rust_type_for_java_type_with_facts(&values, &facts)),
            "Vec<Option<Byte>>"
        );
        let platform_byte = JavaType::Named {
            name: "java.lang.Byte".to_string(),
            args: Vec::new(),
        };
        for representations in [None, Some(RepresentationPlan::default())] {
            let rendered = match representations {
                None => rust_type_for_java_type_with_facts(&platform_byte, &facts),
                Some(plan) => rust_type_for_java_type_with_facts_and_representations(
                    &platform_byte,
                    &facts,
                    &plan,
                ),
            };
            assert_eq!(crate::emit::emit_type(&rendered), "i8");
        }
    }

    #[test]
    fn exception_like_java_types_render_to_java_exception_carrier() {
        assert_eq!(
            rust_type_for_java_type(&JavaType::Named {
                name: "Throwable".to_string(),
                args: Vec::new(),
            }),
            RustType::Named("JavaException".to_string())
        );
        assert_eq!(
            rust_type_for_java_type(&JavaType::Named {
                name: "RuntimeException".to_string(),
                args: Vec::new(),
            }),
            RustType::Named("JavaException".to_string())
        );
        assert_eq!(
            rust_type_for_java_type(&JavaType::Array(Box::new(JavaType::Named {
                name: "java.lang.Throwable".to_string(),
                args: Vec::new(),
            }))),
            RustType::Vec(Box::new(RustType::Option(Box::new(RustType::Named(
                "JavaException".to_string()
            )))))
        );
        assert_eq!(
            rust_type_for_java_type(&JavaType::Named {
                name: "CauseStateException".to_string(),
                args: Vec::new(),
            }),
            RustType::Named("CauseStateException".to_string())
        );
    }

    #[test]
    fn identity_string_representation_is_consistent_for_direct_and_collection_slots() {
        let mut facts = FactSet::default();
        facts.java_string_identity_representation = true;
        assert_eq!(
            rust_type_for_java_type_with_facts(&JavaType::String, &facts),
            RustType::Named("JavaString".to_string())
        );
        assert_eq!(
            crate::emit::emit_type(&rust_type_for_java_type_with_facts(
                &JavaType::Named {
                    name: "java.util.List".to_string(),
                    args: vec![JavaType::String],
                },
                &facts,
            )),
            "Vec<JavaString>"
        );
        for (ownership, expected) in [
            (OwnershipShape::Owned, "Option<JavaString>"),
            (OwnershipShape::CopyValue, "Option<JavaString>"),
            (OwnershipShape::SharedBorrow, "Option<&JavaString>"),
            (OwnershipShape::MutableBorrow, "Option<&mut JavaString>"),
        ] {
            let mut shape =
                RustSlotShape::for_storage(crate::types::optional_java_type(JavaType::String));
            shape.ownership = ownership;
            shape.view = ViewShape::Str;
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_slot_with_facts(&shape, &facts)),
                expected
            );
        }
    }

    #[test]
    fn char_slot_default_is_java_nul_character() {
        let shape = RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Char));

        assert_eq!(
            crate::emit::emit_expr(&default_expr_for_slot_with_facts(
                &shape,
                &FactSet::default(),
            )),
            "JavaCodeUnit(0u16)"
        );
    }
}

#[cfg(test)]
mod qualified_native_sam_tests {
    #[test]
    fn qualified_native_sam_declarations_and_capture_constructors_share_the_carrier() {
        let facts = crate::facts::FactSet::default();
        let effective = crate::effective_facts::EffectiveFacts::new(&facts);
        let ty = crate::java_ir::JavaType::Named {
            name: "java.util.function.Consumer".into(),
            args: vec![crate::java_ir::JavaType::String],
        };
        let shape = crate::slot_shape::RustSlotShape::for_storage(ty.clone());
        assert_eq!(
            super::functional_slot_carrier_name(&shape, &effective).as_deref(),
            Some("Consumer")
        );
        let rendered = super::rust_type_for_java_type_with_effective_facts(&ty, &effective);
        assert!(
            matches!(rendered, crate::rust_ir::RustType::Generic { ref base, ref args } if base == "Consumer" && args.len() == 1),
            "{rendered:?}"
        );
    }
}
