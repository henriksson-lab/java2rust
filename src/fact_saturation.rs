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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FiniteFactDomain {
    pub slot_count: usize,
    pub fact_kind_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FiniteSaturationPolicy {
    pub domain: FiniteFactDomain,
}

impl FiniteFactDomain {
    pub fn new(slot_count: usize, fact_kind_count: usize) -> Self {
        Self {
            slot_count,
            fact_kind_count,
        }
    }

    pub fn iteration_budget(self) -> usize {
        self.slot_count
            .saturating_mul(self.fact_kind_count)
            .saturating_add(1)
    }
}

impl FiniteSaturationPolicy {
    pub fn new(slot_count: usize, fact_kind_count: usize) -> Self {
        Self {
            domain: FiniteFactDomain::new(slot_count, fact_kind_count),
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

pub fn finite_fact_iteration_budget(slot_count: usize, fact_kind_count: usize) -> usize {
    FiniteFactDomain::new(slot_count, fact_kind_count).iteration_budget()
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
    use super::{
        FiniteFactDomain, FiniteSaturationPolicy, SaturationStep, finite_fact_iteration_budget,
        saturate_finite,
    };

    #[test]
    fn finite_fact_domain_budget_bounds_slots_times_fact_kinds() {
        assert_eq!(FiniteFactDomain::new(3, 4).iteration_budget(), 13);
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
}
