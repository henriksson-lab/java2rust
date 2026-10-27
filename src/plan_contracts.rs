//! Scoped native allocation contracts. Generic qualifier relations are not value
//! flow: instantiating a binder does not mean moving one runtime object to every
//! use. This checker keeps those relations separate from initializer boundaries.
use crate::{
    conversion_plan::{Slot, SlotConversionPlan, slot_conversion_plan_with_facts},
    facts::FactSet,
    signature::{SignatureDependencyNode, SignaturePlan},
    slot_shape::RustSlotShape,
    source_generic_allocations::{GenericAllocationEdge, native_argument_allocation_target},
    storage_strategy::StoragePlacement,
};
use std::{collections::HashSet, sync::Arc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeSlotKey {
    pub node: SignatureDependencyNode,
    pub path: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeRequirement {
    pub slot: NativeSlotKey,
    pub placement: StoragePlacement,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NativeContractInventory {
    pub generic_edges: Arc<Vec<GenericAllocationEdge>>,
    pub initializer_edges: Vec<(SignatureDependencyNode, SignatureDependencyNode)>,
    pub requirements: Vec<NativeRequirement>,
    pub ambiguous_inherited_fields: Vec<NativeSlotKey>,
    pub symbolic_occurrences: Vec<NativeSlotKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FinalizationKind {
    DerivedArtifact,
    DeclarationProjection,
    AdmittedDispatchProjection,
    SolvedAllocationObligation,
}

pub(crate) const FINALIZATION_PASSES: &[(&str, FinalizationKind)] = &[
    ("hierarchy_products", FinalizationKind::DerivedArtifact),
    ("inherited_fields", FinalizationKind::DeclarationProjection),
    (
        "source_interface_references",
        FinalizationKind::AdmittedDispatchProjection,
    ),
    (
        "source_generic_allocations",
        FinalizationKind::SolvedAllocationObligation,
    ),
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ContractCoverage {
    pub checked: usize,
    pub unsupported: usize,
    pub ambiguous: usize,
    pub symbolic: usize,
    /// Native functional facades carry their own environment; root qualifier
    /// bits do not imply an additional Rc<RefCell<...>> allocation around it.
    pub implicit_functional: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContractDiagnostic {
    pub code: &'static str,
    pub source: NativeSlotKey,
    pub target: Option<NativeSlotKey>,
    pub rule: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NativeContractReport {
    pub coverage: ContractCoverage,
    pub diagnostics: Vec<ContractDiagnostic>,
    /// Edges name the premises of each derived qualifier. They are retained
    /// separately from costs and can be followed back to the originating slot.
    pub reasons: Vec<NativeContractReason>,
    pub checked_nodes: HashSet<SignatureDependencyNode>,
    pub invalid_nodes: HashSet<SignatureDependencyNode>,
    pub initializer_boundaries: Vec<NativeBoundaryRecipe>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeContractReason {
    pub premise: NativeRequirement,
    pub conclusion: NativeRequirement,
    pub rule: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeBoundaryRecipe {
    pub source: NativeSlotKey,
    pub target: NativeSlotKey,
    actual: RustSlotShape,
    expected: RustSlotShape,
    conversion: SlotConversionPlan,
}

/// One admitted producer-to-storage recipe. Construction of the producer
/// already obeys its return ABI; this boundary never adds another allocation.
pub(crate) struct FrozenNativeBoundary<'a> {
    recipe: &'a NativeBoundaryRecipe,
}
impl<'a> FrozenNativeBoundary<'a> {
    pub(crate) fn for_initializer(
        plan: &'a SignaturePlan,
        source: &NativeSlotKey,
        target: &NativeSlotKey,
    ) -> Result<Option<Self>, &'static str> {
        let Some(report) = plan.native_contract_report() else {
            return Ok(None);
        };
        let Some(recipe) = report
            .initializer_boundaries
            .iter()
            .find(|recipe| recipe.source == *source && recipe.target == *target)
        else {
            return Ok(None);
        };
        let (SlotResolution::Resolved(actual), SlotResolution::Resolved(expected)) = (
            plan.native_contract_shape(&source.node),
            plan.native_contract_shape(&target.node),
        ) else {
            return Err("frozen initializer endpoint identity disappeared");
        };
        if report.invalid_nodes.contains(&source.node)
            || report.invalid_nodes.contains(&target.node)
            || actual != &recipe.actual
            || expected != &recipe.expected
        {
            return Err("frozen initializer endpoint changed after validation");
        }
        Ok(Some(Self { recipe }))
    }
    pub(crate) fn target_shape(&self) -> &RustSlotShape {
        &self.recipe.expected
    }
    pub(crate) fn render_target_type(
        &self,
        facts: &FactSet,
        representations: &crate::representation::RepresentationPlan,
    ) -> crate::rust_ir::RustType {
        crate::slot_render::rust_type_for_slot_with_facts_and_representations(
            &self.recipe.expected,
            facts,
            representations,
        )
    }
    pub(crate) fn construct_stored_value(
        &self,
        producer: crate::rust_ir::RustExpr,
    ) -> crate::rust_ir::RustExpr {
        // Use the exact checked recipe; no construction wrapper is inferred.
        crate::coercion::apply_slot_conversion_plan(producer, &self.recipe.conversion)
    }
}

fn primitive_array_boundary(
    actual: &RustSlotShape,
    expected: &RustSlotShape,
    facts: &FactSet,
) -> bool {
    let actual_base = actual
        .optional_element_type()
        .unwrap_or_else(|| actual.java_ty.clone());
    let target_base = expected
        .optional_element_type()
        .unwrap_or_else(|| expected.java_ty.clone());
    actual_base == target_base
        && matches!(&actual_base, crate::java_ir::JavaType::Array(child) if matches!(child.as_ref(), crate::java_ir::JavaType::Primitive(_)))
        && actual.ownership == crate::slot_shape::OwnershipShape::Owned
        && expected.ownership == crate::slot_shape::OwnershipShape::Owned
        && crate::slot_render::native_allocation_has_outer_carrier(
            &actual_base,
            &crate::effective_facts::EffectiveFacts::new(facts),
        )
}

pub(crate) enum SlotResolution<'a> {
    Resolved(&'a RustSlotShape),
    Missing,
    Ambiguous,
}

/// A borrowed view. Its scope is the declared inventory, not every expression,
/// Java alias, borrow lifetime, callback or operation in the generated program.
#[derive(Debug)]
pub(crate) struct ResolvedNativeContracts<'a> {
    signatures: &'a SignaturePlan,
    facts: &'a FactSet,
}

#[derive(Debug)]
pub(crate) struct ValidatedNativeContracts<'a> {
    view: ResolvedNativeContracts<'a>,
    report: NativeContractReport,
}

fn placement_at(shape: &RustSlotShape, path: &[usize]) -> StoragePlacement {
    if path.is_empty() {
        return shape.native_allocation;
    }
    shape
        .nested_native_allocations
        .iter()
        .filter(|(key, _)| key == path)
        .fold(StoragePlacement::default(), |placement, (_, next)| {
            placement.join(*next)
        })
}
fn satisfies(selected: StoragePlacement, required: StoragePlacement) -> bool {
    selected.join(required) == selected
}

impl<'a> ResolvedNativeContracts<'a> {
    pub(crate) fn new(signatures: &'a SignaturePlan, facts: &'a FactSet) -> Self {
        Self { signatures, facts }
    }
    pub(crate) fn shape(&self, slot: &NativeSlotKey) -> SlotResolution<'a> {
        self.signatures.native_contract_shape(&slot.node)
    }
    pub(crate) fn check(&self) -> NativeContractReport {
        let inventory = self.signatures.native_contract_inventory();
        let mut report = NativeContractReport::default();
        for slot in &inventory.ambiguous_inherited_fields {
            report.coverage.ambiguous += 1;
            diagnostic(
                &mut report,
                slot,
                None,
                "inherited_field_projection",
                "native_contract_ambiguous_inherited_field",
                "declaring-field evidence is tied; no unique substitution contract",
            );
        }
        for required in &inventory.requirements {
            let Some(shape) = resolve(
                self,
                &required.slot,
                None,
                "selected_requirement",
                &mut report,
            ) else {
                continue;
            };
            if matches!(
                required.slot.node,
                SignatureDependencyNode::GenericParameter { .. }
            ) || inventory.symbolic_occurrences.iter().any(|symbolic| {
                symbolic.node == required.slot.node
                    && required.slot.path.starts_with(&symbolic.path)
            }) {
                if !satisfies(placement_at(shape, &required.slot.path), required.placement) {
                    diagnostic(
                        &mut report,
                        &required.slot,
                        None,
                        "selected_binder_requirement",
                        "native_contract_missing_requirement",
                        "solved binder lacks accumulated allocation evidence",
                    );
                } else {
                    report.coverage.symbolic += 1;
                    report.checked_nodes.insert(required.slot.node.clone());
                }
            } else if RustSlotShape::native_child_type_at_path(&shape.java_ty, &required.slot.path)
                .is_none()
            {
                report.coverage.unsupported += 1;
                diagnostic(
                    &mut report,
                    &required.slot,
                    None,
                    "selected_requirement",
                    "native_contract_invalid_path",
                    "structural native allocation path does not exist",
                );
            } else if !RustSlotShape::native_child_type_at_path(&shape.java_ty, &required.slot.path)
                .is_some_and(|child| {
                    let child = crate::types::optional_element_type(&child).unwrap_or(child);
                    let effective = crate::effective_facts::EffectiveFacts::new(self.facts);
                    crate::slot_render::native_allocation_has_outer_carrier(&child, &effective)
                        || effective.native_functional_carrier(&child).is_some()
                })
            {
                diagnostic(
                    &mut report,
                    &required.slot,
                    None,
                    "selected_requirement",
                    "native_contract_non_native_position",
                    "allocation evidence addresses a concrete nonnative leaf, not a supported native outer allocation or implicit functional environment",
                );
            } else if !satisfies(placement_at(shape, &required.slot.path), required.placement) {
                diagnostic(
                    &mut report,
                    &required.slot,
                    None,
                    "selected_requirement",
                    "native_contract_missing_requirement",
                    "selected placement does not satisfy accumulated allocation evidence",
                );
            } else {
                let child =
                    RustSlotShape::native_child_type_at_path(&shape.java_ty, &required.slot.path)
                        .unwrap();
                let child = crate::types::optional_element_type(&child).unwrap_or(child);
                if crate::effective_facts::EffectiveFacts::new(self.facts)
                    .native_functional_carrier(&child)
                    .is_some()
                {
                    report.coverage.implicit_functional += 1;
                } else {
                    report.coverage.checked += 1;
                }
                report.checked_nodes.insert(required.slot.node.clone());
            }
        }
        for edge in inventory.generic_edges.iter() {
            self.check_generic(edge, &mut report);
        }
        // Storage propagation edges are encoded (destination, producer).
        // Value conversion is directional and must run producer -> destination.
        for (target, source) in &inventory.initializer_edges {
            let source = NativeSlotKey {
                node: source.clone(),
                path: vec![],
            };
            let target = NativeSlotKey {
                node: target.clone(),
                path: vec![],
            };
            let Some(actual) = resolve(self, &source, Some(&target), "initializer", &mut report)
            else {
                continue;
            };
            let Some(expected) = resolve(self, &target, Some(&source), "initializer", &mut report)
            else {
                continue;
            };
            // Evaluate both endpoints together: concrete-native vs symbolic
            // generic canonicalization depends on the counterpart.
            let conversion = slot_conversion_plan_with_facts(
                actual,
                &Slot::Assignment(expected.clone()),
                self.facts,
            );
            let errors = conversion.unsupported_diagnostics();
            if errors.is_empty() {
                report.coverage.checked += 1;
                if matches!(source.node, SignatureDependencyNode::MethodReturn { .. })
                    && matches!(target.node, SignatureDependencyNode::Field { .. })
                    && primitive_array_boundary(actual, expected, self.facts)
                {
                    report.initializer_boundaries.push(NativeBoundaryRecipe {
                        source: source.clone(),
                        target: target.clone(),
                        actual: actual.clone(),
                        expected: expected.clone(),
                        conversion,
                    });
                }
            } else {
                report.coverage.unsupported += 1;
                if primitive_array_boundary(actual, expected, self.facts) {
                    report.invalid_nodes.insert(source.node.clone());
                    report.invalid_nodes.insert(target.node.clone());
                }
                diagnostic(
                    &mut report,
                    &source,
                    Some(&target),
                    "initializer",
                    "native_contract_unsupported_conversion",
                    &format!("{errors:?}"),
                );
            }
        }
        report
    }
    fn check_generic(&self, edge: &GenericAllocationEdge, report: &mut NativeContractReport) {
        let source = NativeSlotKey {
            node: edge.source.clone(),
            path: edge.source_path.clone(),
        };
        let target = NativeSlotKey {
            node: edge.target.clone(),
            path: edge.target_path.clone(),
        };
        let Some(actual) = resolve(self, &source, Some(&target), "generic_allocation", report)
        else {
            return;
        };
        let Some(expected) = resolve(self, &target, Some(&source), "generic_allocation", report)
        else {
            return;
        };
        for (path, required) in std::iter::once((&[][..], actual.native_allocation)).chain(
            actual
                .nested_native_allocations
                .iter()
                .map(|(path, p)| (path.as_slice(), *p)),
        ) {
            let Some(suffix) = path.strip_prefix(edge.source_path.as_slice()) else {
                continue;
            };
            if required == StoragePlacement::default() {
                continue;
            }
            if edge
                .source_argument
                .as_ref()
                .is_some_and(|ty| !native_argument_allocation_target(ty, suffix))
                || edge
                    .target_argument
                    .as_ref()
                    .is_some_and(|ty| !native_argument_allocation_target(ty, suffix))
            {
                continue;
            }
            let mut target_path = edge.target_path.clone();
            target_path.extend_from_slice(suffix);
            let premise = NativeRequirement {
                slot: NativeSlotKey {
                    node: source.node.clone(),
                    path: path.to_vec(),
                },
                placement: required,
            };
            let conclusion = NativeRequirement {
                slot: NativeSlotKey {
                    node: target.node.clone(),
                    path: target_path.clone(),
                },
                placement: required,
            };
            report.reasons.push(NativeContractReason {
                premise,
                conclusion,
                rule: "generic_allocation",
            });
            if !satisfies(placement_at(expected, &target_path), required) {
                diagnostic(
                    report,
                    &source,
                    Some(&target),
                    "generic_allocation",
                    "native_contract_generic_obligation",
                    "instantiated target lacks its source qualifier requirement",
                );
            } else {
                // An explicit argument is the instantiation context. Do not
                // invent a conversion between two unrelated binder uses.
                let concrete = edge
                    .target_argument
                    .as_ref()
                    .or(edge.source_argument.as_ref());
                if let Some(concrete) = concrete {
                    let Some(instantiated) =
                        RustSlotShape::native_child_type_at_path(concrete, suffix)
                    else {
                        report.coverage.unsupported += 1;
                        diagnostic(
                            report,
                            &source,
                            Some(&target),
                            "generic_allocation",
                            "native_contract_invalid_instantiation",
                            "generic substitution does not address a native child",
                        );
                        continue;
                    };
                    let mut actual_child = RustSlotShape::for_storage(instantiated.clone());
                    actual_child.native_allocation = required;
                    let mut expected_child = RustSlotShape::for_storage(instantiated);
                    expected_child.native_allocation = placement_at(expected, &target_path);
                    let conversion = slot_conversion_plan_with_facts(
                        &actual_child,
                        &Slot::Assignment(expected_child),
                        self.facts,
                    );
                    if !conversion.unsupported_diagnostics().is_empty() {
                        report.coverage.unsupported += 1;
                        diagnostic(
                            report,
                            &source,
                            Some(&target),
                            "generic_allocation",
                            "native_contract_instantiated_conversion",
                            "instantiated native endpoints require an unsupported identity-preserving allocation conversion",
                        );
                    } else {
                        report.coverage.checked += 1;
                    }
                } else {
                    report.coverage.symbolic += 1;
                }
            }
        }
    }
    pub(crate) fn validate(self) -> Result<ValidatedNativeContracts<'a>, NativeContractReport> {
        let report = self.check();
        if !report.diagnostics.is_empty()
            || report.coverage.unsupported != 0
            || report.coverage.ambiguous != 0
        {
            return Err(report);
        }
        Ok(ValidatedNativeContracts { view: self, report })
    }
}

impl<'a> ValidatedNativeContracts<'a> {
    pub(crate) fn report(&self) -> &NativeContractReport {
        &self.report
    }
    pub(crate) fn placement(&self, slot: &NativeSlotKey) -> Option<StoragePlacement> {
        let SlotResolution::Resolved(shape) = self.view.shape(slot) else {
            return None;
        };
        RustSlotShape::native_child_type_at_path(&shape.java_ty, &slot.path)?;
        Some(placement_at(shape, &slot.path))
    }
    /// Consumers use the same resolved endpoint pair checked at the freeze
    /// boundary; this API never guesses an owned clone or independently joins
    /// a competing storage plan.
    pub(crate) fn conversion(
        &self,
        source: &NativeSlotKey,
        target: &NativeSlotKey,
    ) -> Option<SlotConversionPlan> {
        if !source.path.is_empty() || !target.path.is_empty() {
            return None;
        }
        let (SlotResolution::Resolved(actual), SlotResolution::Resolved(expected)) =
            (self.view.shape(source), self.view.shape(target))
        else {
            return None;
        };
        let plan = slot_conversion_plan_with_facts(
            actual,
            &Slot::Assignment(expected.clone()),
            self.view.facts,
        );
        plan.unsupported_diagnostics().is_empty().then_some(plan)
    }
}
fn resolve<'a>(
    view: &ResolvedNativeContracts<'a>,
    slot: &NativeSlotKey,
    other: Option<&NativeSlotKey>,
    rule: &'static str,
    report: &mut NativeContractReport,
) -> Option<&'a RustSlotShape> {
    match view.shape(slot) {
        SlotResolution::Resolved(shape) => Some(shape),
        SlotResolution::Missing => {
            report.coverage.unsupported += 1;
            diagnostic(
                report,
                slot,
                other,
                rule,
                "native_contract_missing_slot",
                "no resolved declaration slot for this identity",
            );
            None
        }
        SlotResolution::Ambiguous => {
            report.coverage.ambiguous += 1;
            diagnostic(
                report,
                slot,
                other,
                rule,
                "native_contract_ambiguous_slot",
                "more than one declaration matches this identity",
            );
            None
        }
    }
}
fn diagnostic(
    report: &mut NativeContractReport,
    source: &NativeSlotKey,
    target: Option<&NativeSlotKey>,
    rule: &'static str,
    code: &'static str,
    message: &str,
) {
    if matches!(
        code,
        "native_contract_invalid_path"
            | "native_contract_non_native_position"
            | "native_contract_missing_requirement"
            | "native_contract_generic_obligation"
    ) {
        report.invalid_nodes.insert(source.node.clone());
        if let Some(target) = target {
            report.invalid_nodes.insert(target.node.clone());
        }
    }
    report.diagnostics.push(ContractDiagnostic {
        code,
        source: source.clone(),
        target: target.cloned(),
        rule,
        message: message.into(),
    });
}

/// Proof token for one covered selection, tied to an immutable signature plan.
/// It establishes accumulated native placement obligations only, never loans.
pub(crate) struct FrozenNativeSlot<'a> {
    shape: &'a RustSlotShape,
}
impl<'a> FrozenNativeSlot<'a> {
    pub(crate) fn from_checked(
        plan: &'a SignaturePlan,
        node: &SignatureDependencyNode,
    ) -> Option<Self> {
        let report = plan.native_contract_report()?;
        // A failed proof cannot create a token. Top-level lowering reports
        // these hard failures and returns a compile_error artifact before
        // rendering; querying this borrowed view itself must never panic.
        if report.invalid_nodes.contains(node) {
            return None;
        }
        if !report.checked_nodes.contains(node) {
            return None;
        }
        let SlotResolution::Resolved(shape) = plan.native_contract_shape(node) else {
            return None;
        };
        Some(Self { shape })
    }
    pub(crate) fn shape(&self) -> &'a RustSlotShape {
        self.shape
    }
}
