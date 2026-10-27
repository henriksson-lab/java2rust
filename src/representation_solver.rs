//! Bounded finite-domain optimization over already admitted implementations.
//! Semantic analysis and lowering recipes live outside this module. Forbidden
//! relation rows are hard constraints, never expensive alternatives.

#[derive(Clone, Debug)]
pub(crate) struct Variable {
    pub costs: Vec<u64>,
    pub incumbent: usize,
}

/// A row admits an assignment to all participating variables and charges its
/// boundary actions once. Missing tuples are infeasible. Duplicate rows are
/// rejected so the selected action/cost cannot depend on iteration order.
#[derive(Clone, Debug)]
pub(crate) struct Relation {
    pub variables: Vec<usize>,
    pub rows: Vec<(Vec<usize>, u64)>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Problem {
    pub variables: Vec<Variable>,
    pub relations: Vec<Relation>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Budget {
    pub search_nodes: usize,
    pub group_variables: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            search_nodes: 4096,
            group_variables: 12,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Statistics {
    pub search_nodes: usize,
    pub pruned_candidates: usize,
    pub improved_groups: usize,
    pub skipped_groups: usize,
    pub exhausted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Solution {
    pub assignment: Vec<usize>,
    pub incumbent_cost: u128,
    pub cost: u128,
    pub statistics: Statistics,
}

impl Problem {
    fn check_schema(&self) -> Result<(), String> {
        for (index, variable) in self.variables.iter().enumerate() {
            if variable.costs.is_empty() || variable.incumbent >= variable.costs.len() {
                return Err(format!(
                    "invalid candidate domain/incumbent for variable {index}"
                ));
            }
        }
        for (index, relation) in self.relations.iter().enumerate() {
            let mut unique = std::collections::HashSet::new();
            for &var in &relation.variables {
                if var >= self.variables.len() || !unique.insert(var) {
                    return Err(format!("invalid variable in relation {index}"));
                }
            }
            let mut tuples = std::collections::HashSet::new();
            for (values, _) in &relation.rows {
                if values.len() != relation.variables.len()
                    || !tuples.insert(values)
                    || values
                        .iter()
                        .zip(&relation.variables)
                        .any(|(&v, &var)| v >= self.variables[var].costs.len())
                {
                    return Err(format!("invalid or duplicate tuple in relation {index}"));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn evaluate(&self, assignment: &[usize]) -> Result<u128, String> {
        if assignment.len() != self.variables.len() {
            return Err("assignment arity mismatch".into());
        }
        let mut cost = 0u128;
        for (index, (&value, variable)) in assignment.iter().zip(&self.variables).enumerate() {
            cost += u128::from(
                *variable
                    .costs
                    .get(value)
                    .ok_or_else(|| format!("invalid candidate for variable {index}"))?,
            );
        }
        for (index, relation) in self.relations.iter().enumerate() {
            let row = relation.rows.iter().find(|(values, _)| {
                values
                    .iter()
                    .zip(&relation.variables)
                    .all(|(&value, &var)| assignment[var] == value)
            });
            cost += u128::from(
                row.ok_or_else(|| format!("hard relation {index} violated"))?
                    .1,
            );
        }
        Ok(cost)
    }

    /// Deterministic group search with domain propagation and an additive lower
    /// bound. A feasible incumbent is mandatory. Budget exhaustion retains it.
    pub(crate) fn solve(&self, budget: Budget) -> Result<Solution, String> {
        self.check_schema()?;
        let assignment = self
            .variables
            .iter()
            .map(|v| v.incumbent)
            .collect::<Vec<_>>();
        let incumbent_cost = self.evaluate(&assignment)?;
        let mut solution = Solution {
            assignment,
            incumbent_cost,
            cost: incumbent_cost,
            statistics: Statistics::default(),
        };
        let mut adjacency = vec![Vec::new(); self.variables.len()];
        let mut relation_ids = vec![Vec::new(); self.variables.len()];
        for (id, relation) in self.relations.iter().enumerate() {
            for &var in &relation.variables {
                relation_ids[var].push(id);
            }
            if let Some((&first, rest)) = relation.variables.split_first() {
                for &other in rest {
                    adjacency[first].push(other);
                    adjacency[other].push(first);
                }
            }
        }
        let mut seen = vec![false; self.variables.len()];
        let mut groups = Vec::new();
        for first in 0..seen.len() {
            if seen[first] {
                continue;
            }
            let mut group = vec![first];
            seen[first] = true;
            let mut cursor = 0;
            while cursor < group.len() {
                for &other in &adjacency[group[cursor]] {
                    if !seen[other] {
                        seen[other] = true;
                        group.push(other);
                    }
                }
                cursor += 1;
            }
            group.sort_unstable();
            groups.push(group);
        }
        // Highest possible unary saving first; relation costs are still checked
        // exactly during search. This ordering is a heuristic, not a proof.
        groups.sort_by_key(|group| {
            (
                std::cmp::Reverse(
                    group
                        .iter()
                        .map(|&v| {
                            let variable = &self.variables[v];
                            variable.costs[variable.incumbent]
                                - *variable.costs.iter().min().unwrap()
                        })
                        .map(u128::from)
                        .sum::<u128>(),
                ),
                group[0],
            )
        });
        for group in groups {
            if group.len() > budget.group_variables {
                solution.statistics.skipped_groups += 1;
                continue;
            }
            if solution.statistics.search_nodes >= budget.search_nodes {
                solution.statistics.exhausted = true;
                break;
            }
            // Solve each connected component in local coordinates. Disconnected
            // observers must not rescan every other observer's domains/relations.
            let local_ids = group
                .iter()
                .enumerate()
                .map(|(local, &global)| (global, local))
                .collect::<std::collections::HashMap<_, _>>();
            let mut used_relations = group
                .iter()
                .flat_map(|&var| relation_ids[var].iter().copied())
                .collect::<Vec<_>>();
            used_relations.sort_unstable();
            used_relations.dedup();
            let local = Problem {
                variables: group
                    .iter()
                    .map(|&global| Variable {
                        costs: self.variables[global].costs.clone(),
                        incumbent: solution.assignment[global],
                    })
                    .collect(),
                relations: used_relations
                    .iter()
                    .map(|&id| Relation {
                        variables: self.relations[id]
                            .variables
                            .iter()
                            .map(|var| local_ids[var])
                            .collect(),
                        rows: self.relations[id].rows.clone(),
                    })
                    .collect(),
            };
            let assignment = local
                .variables
                .iter()
                .map(|v| v.incumbent)
                .collect::<Vec<_>>();
            let before = local.evaluate(&assignment)?;
            let mut trial = Solution {
                assignment,
                incumbent_cost: before,
                cost: before,
                statistics: Statistics::default(),
            };
            let mut domains = local
                .variables
                .iter()
                .map(|v| (0..v.costs.len()).collect())
                .collect();
            let local_group = (0..group.len()).collect::<Vec<_>>();
            local.search(
                &mut domains,
                &local_group,
                Budget {
                    search_nodes: budget.search_nodes - solution.statistics.search_nodes,
                    group_variables: budget.group_variables,
                },
                &mut trial,
            );
            solution.statistics.search_nodes += trial.statistics.search_nodes;
            solution.statistics.pruned_candidates += trial.statistics.pruned_candidates;
            solution.statistics.exhausted |= trial.statistics.exhausted;
            if trial.cost < before {
                solution.statistics.improved_groups += 1;
            }
            solution.cost = solution.cost - before + trial.cost;
            for (local, &global) in group.iter().enumerate() {
                solution.assignment[global] = trial.assignment[local];
            }
        }
        // Independent assignment validation after search, including boundaries
        // to pinned variables and groups that were skipped.
        if self.evaluate(&solution.assignment)? != solution.cost {
            return Err("solution cost mismatch".into());
        }
        Ok(solution)
    }

    fn propagate(&self, domains: &mut [Vec<usize>], statistics: &mut Statistics) -> bool {
        loop {
            let mut changed = false;
            for relation in &self.relations {
                let supported = relation
                    .rows
                    .iter()
                    .filter(|(values, _)| {
                        values
                            .iter()
                            .zip(&relation.variables)
                            .all(|(&value, &var)| domains[var].contains(&value))
                    })
                    .collect::<Vec<_>>();
                if supported.is_empty() {
                    return false;
                }
                for (position, &var) in relation.variables.iter().enumerate() {
                    let before = domains[var].len();
                    domains[var].retain(|value| {
                        supported
                            .iter()
                            .any(|(values, _)| values[position] == *value)
                    });
                    statistics.pruned_candidates += before - domains[var].len();
                    changed |= before != domains[var].len();
                }
            }
            if !changed {
                return true;
            }
        }
    }

    fn search(
        &self,
        domains: &mut Vec<Vec<usize>>,
        group: &[usize],
        budget: Budget,
        solution: &mut Solution,
    ) {
        if solution.statistics.search_nodes >= budget.search_nodes {
            solution.statistics.exhausted = true;
            return;
        }
        solution.statistics.search_nodes += 1;
        if !self.propagate(domains, &mut solution.statistics) {
            return;
        }
        let mut lower = self
            .variables
            .iter()
            .enumerate()
            .map(|(var, variable)| {
                domains[var]
                    .iter()
                    .map(|&v| u128::from(variable.costs[v]))
                    .min()
                    .unwrap()
            })
            .sum::<u128>();
        for relation in &self.relations {
            let Some(cost) = relation
                .rows
                .iter()
                .filter(|(values, _)| {
                    values
                        .iter()
                        .zip(&relation.variables)
                        .all(|(&value, &var)| domains[var].contains(&value))
                })
                .map(|(_, cost)| u128::from(*cost))
                .min()
            else {
                return;
            };
            lower += cost;
        }
        if lower >= solution.cost {
            return;
        }
        let variable = group
            .iter()
            .copied()
            .filter(|&v| domains[v].len() > 1)
            .min_by_key(|&v| (domains[v].len(), v));
        let Some(variable) = variable else {
            let assignment = domains.iter().map(|d| d[0]).collect::<Vec<_>>();
            if let Ok(cost) = self.evaluate(&assignment) {
                if cost < solution.cost {
                    solution.cost = cost;
                    solution.assignment = assignment;
                }
            }
            return;
        };
        let mut candidates = domains[variable].clone();
        candidates.sort_by_key(|&v| (self.variables[variable].costs[v], v));
        for candidate in candidates {
            if solution.statistics.search_nodes >= budget.search_nodes {
                solution.statistics.exhausted = true;
                break;
            }
            let mut trial = domains.clone();
            trial[variable] = vec![candidate];
            self.search(&mut trial, group, budget, solution);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coupled_changes_cross_a_single_variable_cost_barrier() {
        let problem = Problem {
            variables: vec![
                Variable {
                    costs: vec![10, 0],
                    incumbent: 0
                };
                2
            ],
            relations: vec![Relation {
                variables: vec![0, 1],
                rows: vec![
                    (vec![0, 0], 0),
                    (vec![1, 0], 30),
                    (vec![0, 1], 30),
                    (vec![1, 1], 3),
                ],
            }],
        };
        let solution = problem.solve(Budget::default()).unwrap();
        assert_eq!(solution.assignment, vec![1, 1]);
        assert_eq!(solution.cost, 3);
        let exact = (0..2)
            .flat_map(|a| (0..2).map(move |b| vec![a, b]))
            .map(|a| problem.evaluate(&a).unwrap())
            .min()
            .unwrap();
        assert_eq!(solution.cost, exact);
    }
    #[test]
    fn forbidden_candidates_remain_forbidden_at_any_cost() {
        let problem = Problem {
            variables: vec![Variable {
                costs: vec![u64::MAX, 0],
                incumbent: 0,
            }],
            relations: vec![Relation {
                variables: vec![0],
                rows: vec![(vec![0], 0)],
            }],
        };
        let solution = problem.solve(Budget::default()).unwrap();
        assert_eq!(solution.assignment, vec![0]);
        assert!(solution.statistics.pruned_candidates > 0);
        let mut bad = problem;
        bad.variables[0].incumbent = 1;
        assert!(bad.solve(Budget::default()).is_err());
    }
    #[test]
    fn bounded_search_keeps_a_valid_incumbent_and_is_repeatable() {
        let problem = Problem {
            variables: vec![
                Variable {
                    costs: vec![3, 0],
                    incumbent: 0
                };
                3
            ],
            relations: vec![],
        };
        let budget = Budget {
            search_nodes: 1,
            group_variables: 2,
        };
        let a = problem.solve(budget).unwrap();
        assert_eq!(a, problem.solve(budget).unwrap());
        assert!(a.statistics.exhausted);
        assert_eq!(problem.evaluate(&a.assignment).unwrap(), a.cost);
    }
    #[test]
    fn multi_input_constraints_match_exhaustive_optimum() {
        let mut rows = Vec::new();
        for a in 0..3 {
            for b in 0..3 {
                for c in 0..3 {
                    if a + b + c >= 3 {
                        rows.push((vec![a, b, c], (a == b) as u64));
                    }
                }
            }
        }
        let problem = Problem {
            variables: vec![
                Variable {
                    costs: vec![0, 4, 7],
                    incumbent: 2
                };
                3
            ],
            relations: vec![Relation {
                variables: vec![0, 1, 2],
                rows,
            }],
        };
        let exact = (0..3)
            .flat_map(|a| (0..3).flat_map(move |b| (0..3).map(move |c| vec![a, b, c])))
            .filter_map(|v| problem.evaluate(&v).ok())
            .min()
            .unwrap();
        assert_eq!(problem.solve(Budget::default()).unwrap().cost, exact);
    }
}
