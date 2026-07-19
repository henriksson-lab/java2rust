use std::collections::{HashSet, VecDeque};
use std::hash::Hash;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaturationStep {
    Changed,
    Stable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiniteSaturationResult<T, D> {
    pub state: T,
    pub iterations: usize,
    pub diagnostics: Vec<D>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorklistStats {
    pub processed_items: usize,
    pub inserted_facts: usize,
    pub max_queue_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorklistSaturationResult<T, D> {
    pub state: T,
    pub stats: WorklistStats,
    pub diagnostics: Vec<D>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorklistSaturationPolicy {
    pub max_processed_items: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorklistInserter<I> {
    pending: Vec<I>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FiniteFactDomain {
    pub declared_slots: usize,
    pub value_lattice_height: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FiniteSaturationPolicy {
    pub domain: FiniteFactDomain,
}

impl FiniteFactDomain {
    pub fn new(declared_slots: usize, value_lattice_height: usize) -> Self {
        Self {
            declared_slots,
            value_lattice_height,
        }
    }

    pub fn iteration_budget(self) -> usize {
        self.declared_slots
            .saturating_mul(self.value_lattice_height)
            .saturating_add(1)
    }
}

impl FiniteSaturationPolicy {
    pub fn new(declared_slots: usize, value_lattice_height: usize) -> Self {
        Self {
            domain: FiniteFactDomain::new(declared_slots, value_lattice_height),
        }
    }

    pub fn iteration_budget(self) -> usize {
        self.domain.iteration_budget()
    }

    pub fn saturate<T, D>(
        self,
        state: T,
        step: impl FnMut(&mut T) -> SaturationStep,
        budget_diagnostic: impl FnOnce(usize) -> D,
    ) -> FiniteSaturationResult<T, D> {
        saturate_finite(state, self.iteration_budget(), step, budget_diagnostic)
    }
}

impl WorklistStats {
    fn saw_queue_len(&mut self, len: usize) {
        self.max_queue_len = self.max_queue_len.max(len);
    }
}

impl WorklistSaturationPolicy {
    pub fn new(max_processed_items: usize) -> Self {
        Self {
            max_processed_items,
        }
    }
}

impl<I> WorklistInserter<I> {
    pub fn insert(&mut self, item: I) {
        self.pending.push(item);
    }
}

pub fn finite_fact_iteration_budget(declared_slots: usize, value_lattice_height: usize) -> usize {
    FiniteFactDomain::new(declared_slots, value_lattice_height).iteration_budget()
}

pub fn saturate_worklist<T, I, D>(
    state: T,
    initial_items: impl IntoIterator<Item = I>,
    policy: WorklistSaturationPolicy,
    step: impl FnMut(&mut T, &I, &mut WorklistInserter<I>),
    budget_diagnostic: impl FnOnce(WorklistStats) -> D,
) -> WorklistSaturationResult<T, D>
where
    I: Clone + Eq + Hash,
{
    saturate_worklist_with_seen(state, initial_items, policy, step, budget_diagnostic)
}

pub fn saturate_worklist_with_seen<T, I, D>(
    mut state: T,
    initial_items: impl IntoIterator<Item = I>,
    policy: WorklistSaturationPolicy,
    mut step: impl FnMut(&mut T, &I, &mut WorklistInserter<I>),
    budget_diagnostic: impl FnOnce(WorklistStats) -> D,
) -> WorklistSaturationResult<T, D>
where
    I: Clone + Eq + Hash,
{
    let mut seen = HashSet::new();
    let mut queue = VecDeque::new();
    let mut stats = WorklistStats::default();

    for item in initial_items {
        if seen.insert(item.clone()) {
            queue.push_back(item);
            stats.inserted_facts += 1;
        }
    }
    stats.saw_queue_len(queue.len());

    while let Some(item) = queue.pop_front() {
        stats.processed_items += 1;
        if stats.processed_items > policy.max_processed_items {
            return WorklistSaturationResult {
                state,
                stats,
                diagnostics: vec![budget_diagnostic(stats)],
            };
        }

        let mut inserter = WorklistInserter {
            pending: Vec::new(),
        };
        step(&mut state, &item, &mut inserter);

        for next in inserter.pending {
            if seen.insert(next.clone()) {
                queue.push_back(next);
                stats.inserted_facts += 1;
            }
        }
        stats.saw_queue_len(queue.len());
    }

    WorklistSaturationResult {
        state,
        stats,
        diagnostics: Vec::new(),
    }
}

pub fn saturate_finite<T, D>(
    mut state: T,
    max_iterations: usize,
    mut step: impl FnMut(&mut T) -> SaturationStep,
    budget_diagnostic: impl FnOnce(usize) -> D,
) -> FiniteSaturationResult<T, D> {
    let mut iterations = 0;
    loop {
        iterations += 1;
        match step(&mut state) {
            SaturationStep::Stable => {
                return FiniteSaturationResult {
                    state,
                    iterations,
                    diagnostics: Vec::new(),
                };
            }
            SaturationStep::Changed => {
                if iterations >= max_iterations {
                    return FiniteSaturationResult {
                        state,
                        iterations,
                        diagnostics: vec![budget_diagnostic(max_iterations)],
                    };
                }
            }
        }
    }
}

impl From<bool> for SaturationStep {
    fn from(changed: bool) -> Self {
        if changed {
            SaturationStep::Changed
        } else {
            SaturationStep::Stable
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::{
        FiniteFactDomain, FiniteSaturationPolicy, SaturationStep, WorklistSaturationPolicy,
        finite_fact_iteration_budget, saturate_finite, saturate_worklist,
    };

    #[test]
    fn finite_fact_domain_budget_bounds_declared_slots_times_lattice_height() {
        let domain = FiniteFactDomain::new(3, 4);

        assert_eq!(domain.declared_slots, 3);
        assert_eq!(domain.value_lattice_height, 4);
        assert_eq!(domain.iteration_budget(), 13);
        assert_eq!(finite_fact_iteration_budget(0, 4), 1);
        assert_eq!(finite_fact_iteration_budget(3, 0), 1);
    }

    #[test]
    fn finite_saturation_policy_owns_budgeted_saturation_entrypoint() {
        let policy = FiniteSaturationPolicy::new(2, 3);
        let result = policy.saturate(
            0,
            |state| {
                if *state < 4 {
                    *state += 1;
                    SaturationStep::Changed
                } else {
                    SaturationStep::Stable
                }
            },
            |budget| format!("budget {budget}"),
        );

        assert_eq!(policy.iteration_budget(), 7);
        assert_eq!(result.state, 4);
        assert_eq!(result.iterations, 5);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn finite_saturation_stops_when_step_is_stable() {
        let result = saturate_finite(
            0,
            4,
            |state| {
                if *state < 2 {
                    *state += 1;
                    SaturationStep::Changed
                } else {
                    SaturationStep::Stable
                }
            },
            |budget| format!("budget {budget}"),
        );

        assert_eq!(result.state, 2);
        assert_eq!(result.iterations, 3);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn finite_saturation_reports_budget_exhaustion() {
        let result = saturate_finite(
            0,
            3,
            |state| {
                *state += 1;
                SaturationStep::Changed
            },
            |budget| format!("budget {budget}"),
        );

        assert_eq!(result.state, 3);
        assert_eq!(result.iterations, 3);
        assert_eq!(result.diagnostics, vec!["budget 3".to_string()]);
    }

    #[test]
    fn worklist_saturation_propagates_chain_once_per_fact() {
        let mut edges = HashMap::new();
        edges.insert("a", vec!["b"]);
        edges.insert("b", vec!["c"]);
        edges.insert("c", vec!["d"]);

        let result = saturate_worklist(
            HashSet::<&str>::new(),
            ["a"],
            WorklistSaturationPolicy::new(8),
            |seen, item, out| {
                seen.insert(*item);
                for next in edges.get(item).into_iter().flatten() {
                    out.insert(*next);
                }
            },
            |stats| format!("budget after {}", stats.processed_items),
        );

        assert_eq!(result.state, HashSet::from(["a", "b", "c", "d"]));
        assert_eq!(result.stats.processed_items, 4);
        assert_eq!(result.stats.inserted_facts, 4);
        assert!(result.stats.max_queue_len >= 1);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn worklist_saturation_deduplicates_diamond_facts() {
        let mut edges = HashMap::new();
        edges.insert("root", vec!["left", "right"]);
        edges.insert("left", vec!["joined"]);
        edges.insert("right", vec!["joined"]);

        let result = saturate_worklist(
            Vec::<&str>::new(),
            ["root"],
            WorklistSaturationPolicy::new(8),
            |visited, item, out| {
                visited.push(*item);
                for next in edges.get(item).into_iter().flatten() {
                    out.insert(*next);
                }
            },
            |stats| format!("budget after {}", stats.processed_items),
        );

        assert_eq!(result.state, vec!["root", "left", "right", "joined"]);
        assert_eq!(result.stats.processed_items, 4);
        assert_eq!(result.stats.inserted_facts, 4);
        assert_eq!(result.stats.max_queue_len, 2);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn worklist_saturation_terminates_on_cycles() {
        let mut edges = HashMap::new();
        edges.insert("a", vec!["b"]);
        edges.insert("b", vec!["c"]);
        edges.insert("c", vec!["a"]);

        let result = saturate_worklist(
            HashSet::<&str>::new(),
            ["a"],
            WorklistSaturationPolicy::new(8),
            |seen, item, out| {
                seen.insert(*item);
                for next in edges.get(item).into_iter().flatten() {
                    out.insert(*next);
                }
            },
            |stats| format!("budget after {}", stats.processed_items),
        );

        assert_eq!(result.state, HashSet::from(["a", "b", "c"]));
        assert_eq!(result.stats.processed_items, 3);
        assert_eq!(result.stats.inserted_facts, 3);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn worklist_saturation_reports_budget_exhaustion_with_stats() {
        let result = saturate_worklist(
            Vec::<usize>::new(),
            [0],
            WorklistSaturationPolicy::new(2),
            |visited, item, out| {
                visited.push(*item);
                out.insert(item + 1);
            },
            |stats| format!("budget after {}", stats.processed_items),
        );

        assert_eq!(result.state, vec![0, 1]);
        assert_eq!(result.stats.processed_items, 3);
        assert_eq!(result.stats.inserted_facts, 3);
        assert_eq!(result.diagnostics, vec!["budget after 3".to_string()]);
    }
}
