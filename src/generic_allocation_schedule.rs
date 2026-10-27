//! Indexed execution of the existing source-generic allocation transfer.
//!
//! This schedules one synchronous arm, not a whole signature fixpoint. Inputs
//! are observed before any pending output is applied, and output order remains
//! the original edge/requirement order. Other propagation arms must continue to
//! run until the combined solver is stable.
use std::collections::HashMap;
use std::sync::Arc;

use crate::signature::{SignatureConstraint, SignatureDependencyNode};
use crate::source_generic_allocations::{GenericAllocationEdge, native_argument_allocation_target};

type Pending = Vec<(SignatureDependencyNode, SignatureConstraint)>;

/// The old transfer's path projection and concrete-argument admission checks.
/// Both reference execution and indexed execution use this implementation.
pub(crate) fn project_allocation_requirement(
    edge: &GenericAllocationEdge,
    requirement: &SignatureConstraint,
) -> Option<SignatureConstraint> {
    let (path, placement) = match requirement {
        SignatureConstraint::RequiredNativeAllocation(placement) => (&[][..], *placement),
        SignatureConstraint::RequiredNestedNativeAllocation { path, placement } => {
            (path.as_slice(), *placement)
        }
        _ => return None,
    };
    let suffix = path.strip_prefix(edge.source_path.as_slice())?;
    if edge
        .source_argument
        .as_ref()
        .is_some_and(|argument| !native_argument_allocation_target(argument, suffix))
        || edge
            .target_argument
            .as_ref()
            .is_some_and(|argument| !native_argument_allocation_target(argument, suffix))
    {
        return None;
    }
    let mut path = edge.target_path.clone();
    path.extend_from_slice(suffix);
    Some(if path.is_empty() {
        SignatureConstraint::RequiredNativeAllocation(placement)
    } else {
        SignatureConstraint::RequiredNestedNativeAllocation { path, placement }
    })
}

/// Complete ordered reference arm. Its caller applies these pending writes
/// through the existing insertion gates, after this function has finished.
pub(crate) fn reference_pending(
    edges: &[GenericAllocationEdge],
    mut read: impl FnMut(&SignatureDependencyNode) -> Vec<SignatureConstraint>,
) -> Pending {
    let mut pending = Vec::new();
    for edge in edges {
        for requirement in read(&edge.source) {
            if let Some(projected) = project_allocation_requirement(edge, &requirement) {
                pending.push((edge.target.clone(), projected));
            }
        }
    }
    pending
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GenericAllocationScheduleStats {
    pub sweeps: usize,
    pub slots_read: usize,
    pub changed_sources: usize,
    pub changed_destinations: usize,
    pub edges_fired: usize,
    pub requirements_examined: usize,
    pub outputs_projected: usize,
}

#[derive(Clone, Debug)]
struct WatchedSlot {
    node: SignatureDependencyNode,
    outgoing: Vec<usize>,
    incoming: Vec<usize>,
    allocation_snapshot: Vec<SignatureConstraint>,
    /// Destination insertion can normalize ownership/view constraints. Watch
    /// all destination constraints, not only the transferred allocation axis.
    destination_snapshot: Option<Vec<SignatureConstraint>>,
}

/// An execution cache for one immutable edge topology, independent of semantic
/// state and ignored by signature-state equality. It must never be used across
/// scoped output application or a different solver trial state. The slot
/// universe and edge topology must remain fixed for the solve.
#[derive(Clone, Debug)]
pub(crate) struct GenericAllocationSchedule {
    edges: Arc<Vec<GenericAllocationEdge>>,
    slots: Vec<WatchedSlot>,
    source_slot_by_edge: Vec<usize>,
    dirty_edges: Vec<bool>,
    initialized: bool,
    stats: GenericAllocationScheduleStats,
}

impl GenericAllocationSchedule {
    pub(crate) fn new(edges: Arc<Vec<GenericAllocationEdge>>) -> Self {
        fn intern(
            node: &SignatureDependencyNode,
            indices: &mut HashMap<SignatureDependencyNode, usize>,
            slots: &mut Vec<WatchedSlot>,
        ) -> usize {
            *indices.entry(node.clone()).or_insert_with(|| {
                let index = slots.len();
                slots.push(WatchedSlot {
                    node: node.clone(),
                    outgoing: Vec::new(),
                    incoming: Vec::new(),
                    allocation_snapshot: Vec::new(),
                    destination_snapshot: None,
                });
                index
            })
        }
        let mut indices = HashMap::new();
        let mut slots: Vec<WatchedSlot> = Vec::new();
        let mut source_slot_by_edge = Vec::with_capacity(edges.len());
        for (edge_id, edge) in edges.iter().enumerate() {
            let source = intern(&edge.source, &mut indices, &mut slots);
            let target = intern(&edge.target, &mut indices, &mut slots);
            slots[source].outgoing.push(edge_id);
            slots[target].incoming.push(edge_id);
            source_slot_by_edge.push(source);
        }
        let dirty_edges = vec![false; edges.len()];
        Self {
            edges,
            slots,
            source_slot_by_edge,
            dirty_edges,
            initialized: false,
            stats: GenericAllocationScheduleStats::default(),
        }
    }

    /// Topologies are immutable Arc values; replacing one invalidates the cache
    /// even when its length happens to be unchanged.
    pub(crate) fn matches_edges(&self, edges: &Arc<Vec<GenericAllocationEdge>>) -> bool {
        Arc::ptr_eq(&self.edges, edges)
    }

    pub(crate) fn stats(&self) -> GenericAllocationScheduleStats {
        self.stats
    }

    /// Snapshot all watched endpoints before returning any output. Source
    /// allocation changes wake outgoing edges. Destination changes wake incoming
    /// edges, including when a previously applied output was removed elsewhere
    /// or a destination insertion policy's ownership/view premises changed.
    ///
    /// Callers must apply every returned output using their normal unscoped
    /// insertion gates before calling again. A scoped executor must use the
    /// reference arm instead: an unchanged destination does not indicate whether
    /// an earlier scoped write was permitted.
    pub(crate) fn pending(
        &mut self,
        mut read: impl FnMut(&SignatureDependencyNode) -> Vec<SignatureConstraint>,
    ) -> Pending {
        self.stats.sweeps = self.stats.sweeps.saturating_add(1);
        let dirty = &mut self.dirty_edges;
        let mut ready = Vec::new();
        let mut mark = |edge_id: usize| {
            if !dirty[edge_id] {
                dirty[edge_id] = true;
                ready.push(edge_id);
            }
        };
        for slot in &mut self.slots {
            self.stats.slots_read = self.stats.slots_read.saturating_add(1);
            let current = read(&slot.node);
            if !slot.outgoing.is_empty() {
                let allocations = current
                    .iter()
                    .filter(|requirement| {
                        matches!(
                            requirement,
                            SignatureConstraint::RequiredNativeAllocation(_)
                                | SignatureConstraint::RequiredNestedNativeAllocation { .. }
                        )
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if !self.initialized || allocations != slot.allocation_snapshot {
                    self.stats.changed_sources = self.stats.changed_sources.saturating_add(1);
                    slot.allocation_snapshot = allocations;
                    for &edge in &slot.outgoing {
                        mark(edge);
                    }
                }
            }
            if !slot.incoming.is_empty() && slot.destination_snapshot.as_ref() != Some(&current) {
                self.stats.changed_destinations = self.stats.changed_destinations.saturating_add(1);
                slot.destination_snapshot = Some(current);
                for &edge in &slot.incoming {
                    mark(edge);
                }
            }
        }
        self.initialized = true;
        // A dirty edge can have several changed premises. Execute it once in
        // the original stable order; the cached source fact order is unchanged.
        ready.sort_unstable();
        let mut pending = Vec::new();
        for edge_id in ready {
            self.dirty_edges[edge_id] = false;
            self.stats.edges_fired = self.stats.edges_fired.saturating_add(1);
            let edge = &self.edges[edge_id];
            for requirement in &self.slots[self.source_slot_by_edge[edge_id]].allocation_snapshot {
                self.stats.requirements_examined =
                    self.stats.requirements_examined.saturating_add(1);
                if let Some(projected) = project_allocation_requirement(edge, requirement) {
                    self.stats.outputs_projected = self.stats.outputs_projected.saturating_add(1);
                    pending.push((edge.target.clone(), projected));
                }
            }
        }
        pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java_ir::JavaType;
    use crate::signature::FieldKey;
    use crate::slot_shape::ViewShape;
    use crate::storage_strategy::StoragePlacement;

    fn node(name: &str) -> SignatureDependencyNode {
        SignatureDependencyNode::Field {
            field: FieldKey::rust_identity("Owner", name),
        }
    }

    fn edge(source: &str, target: &str) -> GenericAllocationEdge {
        GenericAllocationEdge {
            source: node(source),
            source_path: vec![],
            source_argument: None,
            target: node(target),
            target_path: vec![],
            target_argument: None,
        }
    }

    fn allocation(path: &[usize], sync: bool) -> SignatureConstraint {
        let placement = StoragePlacement::new(true, true, sync);
        if path.is_empty() {
            SignatureConstraint::RequiredNativeAllocation(placement)
        } else {
            SignatureConstraint::RequiredNestedNativeAllocation {
                path: path.into(),
                placement,
            }
        }
    }

    type State = HashMap<SignatureDependencyNode, Vec<SignatureConstraint>>;

    fn apply(state: &mut State, pending: Pending) -> bool {
        let mut changed = false;
        for (node, requirement) in pending {
            let requirements = state.entry(node).or_default();
            if !requirements.contains(&requirement) {
                requirements.push(requirement);
                changed = true;
            }
        }
        changed
    }

    #[test]
    fn transfer_preserves_paths_and_both_argument_guards() {
        let mut projection = edge("source", "target");
        projection.source_path = vec![1];
        projection.target_path = vec![0, 2];
        let rows = JavaType::Array(Box::new(JavaType::Array(Box::new(JavaType::String))));
        projection.source_argument = Some(rows.clone());
        projection.target_argument = Some(rows);
        assert_eq!(
            project_allocation_requirement(&projection, &allocation(&[1, 0], true)),
            Some(allocation(&[0, 2, 0], true))
        );
        assert_eq!(
            project_allocation_requirement(&projection, &allocation(&[0, 0], true)),
            None
        );
        assert_eq!(
            project_allocation_requirement(&projection, &allocation(&[1, 0, 0], true)),
            None
        );
        projection.source_argument = Some(JavaType::String);
        assert_eq!(
            project_allocation_requirement(&projection, &allocation(&[1], true)),
            None
        );
        projection.source_argument = None;
        projection.target_argument = Some(JavaType::String);
        assert_eq!(
            project_allocation_requirement(&projection, &allocation(&[1], true)),
            None
        );
        assert_eq!(
            project_allocation_requirement(&projection, &SignatureConstraint::StorageSlot),
            None
        );
    }

    #[test]
    fn pending_is_synchronous_and_matches_reference_edge_and_fact_order() {
        let edges = Arc::new(vec![edge("a", "b"), edge("b", "c"), edge("a", "d")]);
        let mut schedule = GenericAllocationSchedule::new(edges.clone());
        let mut state = State::from([(
            node("a"),
            vec![allocation(&[1], true), allocation(&[0], false)],
        )]);
        let first = schedule.pending(|node| state.get(node).cloned().unwrap_or_default());
        assert_eq!(
            first,
            reference_pending(&edges, |node| state.get(node).cloned().unwrap_or_default())
        );
        assert!(first.iter().all(|(target, _)| target != &node("c")));
        apply(&mut state, first);
        let second = schedule.pending(|node| state.get(node).cloned().unwrap_or_default());
        assert_eq!(
            second,
            reference_pending(&edges, |node| state.get(node).cloned().unwrap_or_default())
        );
        apply(&mut state, second);
        assert_eq!(state[&node("c")], state[&node("a")]);
    }

    #[test]
    fn idle_sources_do_not_repeat_transfer_work_and_late_inputs_wake_only_dependents() {
        let edges = Arc::new(vec![edge("a", "b"), edge("x", "y")]);
        let mut schedule = GenericAllocationSchedule::new(edges);
        let mut state = State::from([(node("a"), vec![allocation(&[], false)])]);
        let outputs = schedule.pending(|node| state.get(node).cloned().unwrap_or_default());
        apply(&mut state, outputs);
        // Observe the destination changes caused by our own preceding writes.
        let outputs = schedule.pending(|node| state.get(node).cloned().unwrap_or_default());
        apply(&mut state, outputs);
        let before = schedule.stats();
        assert!(
            schedule
                .pending(|node| state.get(node).cloned().unwrap_or_default())
                .is_empty()
        );
        assert_eq!(schedule.stats().edges_fired, before.edges_fired);
        assert_eq!(
            schedule.stats().requirements_examined,
            before.requirements_examined
        );
        state.insert(node("x"), vec![allocation(&[1], true)]);
        let before = schedule.stats();
        assert_eq!(
            schedule.pending(|node| state.get(node).cloned().unwrap_or_default()),
            vec![(node("y"), allocation(&[1], true))]
        );
        assert_eq!(schedule.stats().edges_fired - before.edges_fired, 1);
    }

    #[test]
    fn destination_policy_changes_and_removed_outputs_are_retried() {
        let edges = Arc::new(vec![edge("a", "b")]);
        let mut schedule = GenericAllocationSchedule::new(edges);
        let mut state = State::from([(node("a"), vec![allocation(&[], true)])]);
        let outputs = schedule.pending(|node| state.get(node).cloned().unwrap_or_default());
        apply(&mut state, outputs);
        let outputs = schedule.pending(|node| state.get(node).cloned().unwrap_or_default());
        apply(&mut state, outputs);
        assert!(
            schedule
                .pending(|node| state.get(node).cloned().unwrap_or_default())
                .is_empty()
        );
        state
            .get_mut(&node("b"))
            .unwrap()
            .push(SignatureConstraint::RequiredView(ViewShape::Slice));
        assert_eq!(
            schedule.pending(|node| state.get(node).cloned().unwrap_or_default()),
            vec![(node("b"), allocation(&[], true))]
        );
        state.insert(node("b"), Vec::new());
        assert_eq!(
            schedule.pending(|node| state.get(node).cloned().unwrap_or_default()),
            vec![(node("b"), allocation(&[], true))]
        );
    }

    #[test]
    fn mixed_legacy_and_indexed_rounds_reach_the_same_cyclic_closure() {
        let edges = Arc::new(vec![
            edge("a", "b"),
            edge("b", "c"),
            edge("c", "a"),
            edge("x", "d"),
        ]);
        let seed = State::from([
            (node("a"), vec![allocation(&[0], false)]),
            (node("x"), vec![allocation(&[1], true)]),
        ]);
        let mut indexed_state = seed.clone();
        let mut reference_state = seed;
        let mut schedule = GenericAllocationSchedule::new(edges.clone());
        let mut stable = false;
        for round in 0..16 {
            let indexed =
                schedule.pending(|node| indexed_state.get(node).cloned().unwrap_or_default());
            let reference = reference_pending(&edges, |node| {
                reference_state.get(node).cloned().unwrap_or_default()
            });
            let mut indexed_changed = apply(&mut indexed_state, indexed);
            let mut reference_changed = apply(&mut reference_state, reference);
            // Model a legacy arm after the indexed arm enabling another pass.
            if round == 1 {
                indexed_changed |= apply(
                    &mut indexed_state,
                    vec![(node("b"), allocation(&[2], true))],
                );
                reference_changed |= apply(
                    &mut reference_state,
                    vec![(node("b"), allocation(&[2], true))],
                );
            }
            assert_eq!(indexed_state, reference_state, "round {round}");
            assert_eq!(indexed_changed, reference_changed, "round {round}");
            if !indexed_changed {
                stable = true;
                break;
            }
        }
        assert!(stable);
        assert!(indexed_state[&node("a")].contains(&allocation(&[2], true)));
        assert!(schedule.stats().edges_fired < schedule.stats().sweeps * edges.len());
        assert!(schedule.matches_edges(&edges));
        assert!(!schedule.matches_edges(&Arc::new(edges.as_ref().clone())));
    }
}
