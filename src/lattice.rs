use crate::facts::{DispatchRequirementKind, NullabilityState, ReceiverMutabilityFact};
use crate::slot_shape::{DispatchShape, MutabilityShape, OwnershipShape, ViewShape};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatticeDescriptor<T>
where
    T: Clone + 'static,
{
    pub name: &'static str,
    pub bottom: T,
    pub top: T,
    pub values: &'static [T],
    pub merge_rule: &'static str,
    pub precedence_rule: &'static str,
    pub diagnostic_rule: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedLatticeJoin<T> {
    pub kept: T,
    pub rejected: Option<T>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchLatticeValue {
    Concrete,
    BorrowedTraitView,
    OwnedPolymorphicStorage,
    ClosedRuntimeCaseSplit,
    SubtypeDispatch,
    Instanceof,
    Downcast,
    PublicExtensibleApi,
    UnknownRuntimeObject,
    RejectTraitObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeLatticeValue {
    LocalOnly,
    Returned,
    Stored,
    UnknownBoundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationLatticeValue {
    Uninitialized,
    ConstructorAssignedNonNull,
    MaybeNull,
    InitializedNonNull,
}

pub const NULLABILITY_VALUES: &[NullabilityState] = &[
    NullabilityState::DefinitelyNonNull,
    NullabilityState::DefinitelyNull,
    NullabilityState::MaybeNull,
    NullabilityState::Unknown,
];

pub const OWNERSHIP_VALUES: &[OwnershipShape] = &[
    OwnershipShape::SharedBorrow,
    OwnershipShape::MutableBorrow,
    OwnershipShape::CopyValue,
    OwnershipShape::Owned,
];

pub const VIEW_VALUES: &[ViewShape] = &[
    ViewShape::Str,
    ViewShape::Slice,
    ViewShape::DynTraitView,
    ViewShape::EnumView,
    ViewShape::ErasedObjectView,
    ViewShape::Native,
];

pub const MUTABILITY_VALUES: &[MutabilityShape] =
    &[MutabilityShape::Immutable, MutabilityShape::Mutable];

pub const RECEIVER_MUTABILITY_VALUES: &[ReceiverMutabilityFact] = &[
    ReceiverMutabilityFact::ReadOnly,
    ReceiverMutabilityFact::MutatesReceiver,
    ReceiverMutabilityFact::Unknown,
];

pub const DISPATCH_VALUES: &[DispatchLatticeValue] = &[
    DispatchLatticeValue::Concrete,
    DispatchLatticeValue::BorrowedTraitView,
    DispatchLatticeValue::OwnedPolymorphicStorage,
    DispatchLatticeValue::ClosedRuntimeCaseSplit,
    DispatchLatticeValue::SubtypeDispatch,
    DispatchLatticeValue::Instanceof,
    DispatchLatticeValue::Downcast,
    DispatchLatticeValue::PublicExtensibleApi,
    DispatchLatticeValue::UnknownRuntimeObject,
    DispatchLatticeValue::RejectTraitObject,
];

pub const ESCAPE_VALUES: &[EscapeLatticeValue] = &[
    EscapeLatticeValue::LocalOnly,
    EscapeLatticeValue::Returned,
    EscapeLatticeValue::Stored,
    EscapeLatticeValue::UnknownBoundary,
];

pub const INITIALIZATION_VALUES: &[InitializationLatticeValue] = &[
    InitializationLatticeValue::Uninitialized,
    InitializationLatticeValue::ConstructorAssignedNonNull,
    InitializationLatticeValue::MaybeNull,
    InitializationLatticeValue::InitializedNonNull,
];

pub fn nullability_lattice() -> LatticeDescriptor<NullabilityState> {
    LatticeDescriptor {
        name: "nullability",
        bottom: NullabilityState::DefinitelyNonNull,
        top: NullabilityState::Unknown,
        values: NULLABILITY_VALUES,
        merge_rule: "MaybeNull wins over contradictory null/non-null evidence; Unknown is preserved when evidence is incomplete.",
        precedence_rule: "DefinitelyNonNull < DefinitelyNull < MaybeNull < Unknown for conservative storage requirements.",
        diagnostic_rule: "Emit diagnostics when a finite solver reaches the budget or when user overrides conflict with inferred nullability.",
    }
}

pub fn ownership_lattice() -> LatticeDescriptor<OwnershipShape> {
    LatticeDescriptor {
        name: "ownership",
        bottom: OwnershipShape::SharedBorrow,
        top: OwnershipShape::Owned,
        values: OWNERSHIP_VALUES,
        merge_rule: "Keep the requirement with the highest ownership priority.",
        precedence_rule: "Owned > CopyValue > MutableBorrow > SharedBorrow.",
        diagnostic_rule: "Record the rejected requirement when an ownership join discards weaker evidence.",
    }
}

pub fn view_lattice() -> LatticeDescriptor<ViewShape> {
    LatticeDescriptor {
        name: "view",
        bottom: ViewShape::Str,
        top: ViewShape::Native,
        values: VIEW_VALUES,
        merge_rule: "Keep the view with the highest rendering capability requirement.",
        precedence_rule: "Native > ErasedObjectView > DynTraitView/EnumView > Slice/Str.",
        diagnostic_rule: "Record rejected view evidence when a native or erased view prevents a more idiomatic borrowed view.",
    }
}

pub fn dispatch_lattice() -> LatticeDescriptor<DispatchLatticeValue> {
    LatticeDescriptor {
        name: "dispatch",
        bottom: DispatchLatticeValue::Concrete,
        top: DispatchLatticeValue::RejectTraitObject,
        values: DISPATCH_VALUES,
        merge_rule: "Project dispatch facts to slot roles, then keep the strongest runtime-representation requirement.",
        precedence_rule: "RejectTraitObject and UnknownRuntimeObject dominate, followed by public extension, casts, subtype dispatch, closed enums, and trait views.",
        diagnostic_rule: "Emit structured diagnostics when representation requirements force erased object or reject trait-object lowering.",
    }
}

pub fn receiver_mutability_lattice() -> LatticeDescriptor<ReceiverMutabilityFact> {
    LatticeDescriptor {
        name: "receiver-mutability",
        bottom: ReceiverMutabilityFact::ReadOnly,
        top: ReceiverMutabilityFact::Unknown,
        values: RECEIVER_MUTABILITY_VALUES,
        merge_rule: "MutatesReceiver wins over ReadOnly; Unknown is retained when the call boundary is not modeled.",
        precedence_rule: "Unknown > MutatesReceiver > ReadOnly.",
        diagnostic_rule: "Emit diagnostics when modeled external API facts disagree with source-observed receiver mutation.",
    }
}

pub fn escape_lattice() -> LatticeDescriptor<EscapeLatticeValue> {
    LatticeDescriptor {
        name: "escape",
        bottom: EscapeLatticeValue::LocalOnly,
        top: EscapeLatticeValue::UnknownBoundary,
        values: ESCAPE_VALUES,
        merge_rule: "Escapes widen monotonically from local to returned, stored, and unknown boundary.",
        precedence_rule: "UnknownBoundary > Stored > Returned > LocalOnly.",
        diagnostic_rule: "Record the edge that forces owned storage or unknown-boundary fallback.",
    }
}

pub fn initialization_lattice() -> LatticeDescriptor<InitializationLatticeValue> {
    LatticeDescriptor {
        name: "initialization",
        bottom: InitializationLatticeValue::Uninitialized,
        top: InitializationLatticeValue::MaybeNull,
        values: INITIALIZATION_VALUES,
        merge_rule: "Constructor-proven non-null remains non-null until any nullable assignment or unknown write widens it.",
        precedence_rule: "MaybeNull > InitializedNonNull > ConstructorAssignedNonNull > Uninitialized.",
        diagnostic_rule: "Emit diagnostics when cyclic constructor initialization exceeds the finite field-domain budget.",
    }
}

pub fn dispatch_lattice_value(requirement: DispatchRequirementKind) -> DispatchLatticeValue {
    match requirement {
        DispatchRequirementKind::NeedsSubtypeDispatch => DispatchLatticeValue::SubtypeDispatch,
        DispatchRequirementKind::NeedsClosedRuntimeCaseSplit => {
            DispatchLatticeValue::ClosedRuntimeCaseSplit
        }
        DispatchRequirementKind::NeedsInstanceof => DispatchLatticeValue::Instanceof,
        DispatchRequirementKind::NeedsDowncast => DispatchLatticeValue::Downcast,
        DispatchRequirementKind::NeedsOwnedPolymorphicStorage => {
            DispatchLatticeValue::OwnedPolymorphicStorage
        }
        DispatchRequirementKind::NeedsBorrowedTraitView => DispatchLatticeValue::BorrowedTraitView,
        DispatchRequirementKind::NeedsPublicExtensibleApi => {
            DispatchLatticeValue::PublicExtensibleApi
        }
        DispatchRequirementKind::RejectTraitObject => DispatchLatticeValue::RejectTraitObject,
        DispatchRequirementKind::NeedsUnknownRuntimeObject => {
            DispatchLatticeValue::UnknownRuntimeObject
        }
    }
}

pub fn join_ranked_lattice<T>(
    current: Option<T>,
    next: T,
    rank: impl Fn(&T) -> u8,
) -> RankedLatticeJoin<T>
where
    T: Clone + Eq,
{
    let Some(current) = current else {
        return RankedLatticeJoin {
            kept: next,
            rejected: None,
        };
    };
    if current == next {
        return RankedLatticeJoin {
            kept: current,
            rejected: None,
        };
    }
    if rank(&current) >= rank(&next) {
        RankedLatticeJoin {
            kept: current,
            rejected: Some(next),
        }
    } else {
        RankedLatticeJoin {
            kept: next,
            rejected: Some(current),
        }
    }
}

pub fn ownership_lattice_rank(requirement: &OwnershipShape) -> u8 {
    match requirement {
        OwnershipShape::Owned => 40,
        OwnershipShape::CopyValue => 30,
        OwnershipShape::MutableBorrow => 20,
        OwnershipShape::SharedBorrow => 10,
    }
}

pub fn view_lattice_rank(requirement: &ViewShape) -> u8 {
    match requirement {
        ViewShape::Native => 50,
        ViewShape::ErasedObjectView => 40,
        ViewShape::EnumView | ViewShape::DynTraitView => 30,
        ViewShape::Slice | ViewShape::Str => 10,
    }
}

pub fn dispatch_shape_lattice_rank(requirement: &DispatchShape) -> u8 {
    match requirement {
        DispatchShape::Concrete => 50,
        DispatchShape::ErasedObject => 40,
        DispatchShape::BoxedDynTrait { .. } => 30,
        DispatchShape::ClosedEnum { .. } | DispatchShape::DynTrait { .. } => 20,
    }
}

pub fn storage_nullability_lattice_rank(requirement: &NullabilityState) -> u8 {
    match requirement {
        NullabilityState::MaybeNull => 40,
        NullabilityState::DefinitelyNull => 30,
        NullabilityState::Unknown => 20,
        NullabilityState::DefinitelyNonNull => 10,
    }
}

pub fn field_nullability_join(left: NullabilityState, right: NullabilityState) -> NullabilityState {
    use NullabilityState::{DefinitelyNonNull, DefinitelyNull, MaybeNull, Unknown};

    match (left, right) {
        (same_left, same_right) if same_left == same_right => same_left,
        (MaybeNull, _) | (_, MaybeNull) => MaybeNull,
        (DefinitelyNull, DefinitelyNonNull) | (DefinitelyNonNull, DefinitelyNull) => MaybeNull,
        (DefinitelyNull, Unknown) | (Unknown, DefinitelyNull) => MaybeNull,
        (Unknown, _) | (_, Unknown) => Unknown,
        (DefinitelyNonNull, DefinitelyNonNull) => DefinitelyNonNull,
        (DefinitelyNull, DefinitelyNull) => DefinitelyNull,
    }
}

pub fn receiver_mutability_lattice_rank(requirement: &ReceiverMutabilityFact) -> u8 {
    match requirement {
        ReceiverMutabilityFact::Unknown => 30,
        ReceiverMutabilityFact::MutatesReceiver => 20,
        ReceiverMutabilityFact::ReadOnly => 10,
    }
}

pub fn escape_lattice_rank(requirement: &EscapeLatticeValue) -> u8 {
    match requirement {
        EscapeLatticeValue::UnknownBoundary => 40,
        EscapeLatticeValue::Stored => 30,
        EscapeLatticeValue::Returned => 20,
        EscapeLatticeValue::LocalOnly => 10,
    }
}

pub fn initialization_lattice_rank(requirement: &InitializationLatticeValue) -> u8 {
    match requirement {
        InitializationLatticeValue::MaybeNull => 40,
        InitializationLatticeValue::InitializedNonNull => 30,
        InitializationLatticeValue::ConstructorAssignedNonNull => 20,
        InitializationLatticeValue::Uninitialized => 10,
    }
}
