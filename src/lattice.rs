use crate::facts::NullabilityState;
use crate::slot_shape::{DispatchShape, OwnershipShape, ViewShape};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedLatticeJoin<T> {
    pub kept: T,
    pub rejected: Option<T>,
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

pub(crate) fn field_nullability_join(
    left: NullabilityState,
    right: NullabilityState,
) -> NullabilityState {
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

#[cfg(test)]
mod tests {
    use super::*;
    use NullabilityState::{DefinitelyNonNull, DefinitelyNull, MaybeNull, Unknown};

    #[test]
    fn field_nullability_join_handles_conflicting_evidence() {
        assert_eq!(
            field_nullability_join(DefinitelyNull, DefinitelyNonNull),
            MaybeNull
        );
        assert_eq!(field_nullability_join(Unknown, DefinitelyNonNull), Unknown);
        assert_eq!(field_nullability_join(Unknown, DefinitelyNull), MaybeNull);
    }
}
