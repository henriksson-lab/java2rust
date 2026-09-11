use std::collections::{HashMap, HashSet};
use std::hash::Hash;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StronglyConnectedComponents<N>
where
    N: Eq + Hash,
{
    pub(crate) components: Vec<Vec<N>>,
    pub(crate) component_by_node: HashMap<N, usize>,
    pub(crate) component_successors: Vec<Vec<usize>>,
}

#[derive(Debug, Clone)]
struct TarjanState<N> {
    index_by_node: HashMap<N, usize>,
    lowlink_by_node: HashMap<N, usize>,
    stack: Vec<N>,
    on_stack: HashMap<N, bool>,
    next_index: usize,
    components: Vec<Vec<N>>,
}

pub(crate) fn strongly_connected_components<N>(
    nodes: impl IntoIterator<Item = N>,
    successors: impl Fn(&N) -> Vec<N>,
) -> StronglyConnectedComponents<N>
where
    N: Clone + Eq + Hash,
{
    let mut state = TarjanState {
        index_by_node: HashMap::new(),
        lowlink_by_node: HashMap::new(),
        stack: Vec::new(),
        on_stack: HashMap::new(),
        next_index: 0,
        components: Vec::new(),
    };

    for node in nodes {
        if !state.index_by_node.contains_key(&node) {
            strong_connect(node, &successors, &mut state);
        }
    }

    let mut component_by_node = HashMap::new();
    for (component_index, component) in state.components.iter().enumerate() {
        for node in component {
            component_by_node.insert(node.clone(), component_index);
        }
    }

    let component_successors =
        component_successors(&state.components, &component_by_node, &successors);

    StronglyConnectedComponents {
        components: state.components,
        component_by_node,
        component_successors,
    }
}

fn component_successors<N>(
    components: &[Vec<N>],
    component_by_node: &HashMap<N, usize>,
    successors: &impl Fn(&N) -> Vec<N>,
) -> Vec<Vec<usize>>
where
    N: Clone + Eq + Hash,
{
    components
        .iter()
        .enumerate()
        .map(|(component_index, component)| {
            let mut outgoing = HashSet::new();
            for node in component {
                for successor in successors(node) {
                    if let Some(successor_component) = component_by_node.get(&successor)
                        && *successor_component != component_index
                    {
                        outgoing.insert(*successor_component);
                    }
                }
            }

            let mut outgoing = outgoing.into_iter().collect::<Vec<_>>();
            outgoing.sort_unstable();
            outgoing
        })
        .collect()
}

fn strong_connect<N>(node: N, successors: &impl Fn(&N) -> Vec<N>, state: &mut TarjanState<N>)
where
    N: Clone + Eq + Hash,
{
    let node_index = state.next_index;
    state.next_index += 1;
    state.index_by_node.insert(node.clone(), node_index);
    state.lowlink_by_node.insert(node.clone(), node_index);
    state.stack.push(node.clone());
    state.on_stack.insert(node.clone(), true);

    for successor in successors(&node) {
        if !state.index_by_node.contains_key(&successor) {
            strong_connect(successor.clone(), successors, state);
            let successor_lowlink = state.lowlink_by_node[&successor];
            let node_lowlink = state.lowlink_by_node[&node];
            state
                .lowlink_by_node
                .insert(node.clone(), node_lowlink.min(successor_lowlink));
        } else if state.on_stack.get(&successor).copied().unwrap_or(false) {
            let successor_index = state.index_by_node[&successor];
            let node_lowlink = state.lowlink_by_node[&node];
            state
                .lowlink_by_node
                .insert(node.clone(), node_lowlink.min(successor_index));
        }
    }

    if state.lowlink_by_node[&node] == state.index_by_node[&node] {
        let mut component = Vec::new();
        while let Some(member) = state.stack.pop() {
            state.on_stack.insert(member.clone(), false);
            component.push(member.clone());
            if member == node {
                break;
            }
        }
        state.components.push(component);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::strongly_connected_components;

    fn component_sets<N>(components: Vec<Vec<N>>) -> HashSet<Vec<N>>
    where
        N: Ord + std::hash::Hash + Eq,
    {
        components
            .into_iter()
            .map(|mut component| {
                component.sort();
                component
            })
            .collect()
    }

    #[test]
    fn strongly_connected_components_keeps_acyclic_nodes_separate() {
        let edges = HashMap::from([("a", vec!["b"]), ("b", vec!["c"]), ("c", vec![])]);
        let scc = strongly_connected_components(["a"], |node| edges[node].clone());

        assert_eq!(
            component_sets(scc.components),
            HashSet::from([vec!["a"], vec!["b"], vec!["c"]])
        );
        assert_ne!(scc.component_by_node["a"], scc.component_by_node["b"]);
        assert_ne!(scc.component_by_node["b"], scc.component_by_node["c"]);
        assert_eq!(
            scc.component_successors[scc.component_by_node["a"]],
            vec![scc.component_by_node["b"]]
        );
        assert_eq!(
            scc.component_successors[scc.component_by_node["b"]],
            vec![scc.component_by_node["c"]]
        );
    }

    #[test]
    fn strongly_connected_components_groups_direct_recursion() {
        let edges = HashMap::from([("loop", vec!["loop"])]);
        let scc = strongly_connected_components(["loop"], |node| edges[node].clone());

        assert_eq!(scc.components, vec![vec!["loop"]]);
        assert_eq!(scc.component_by_node["loop"], 0);
        assert_eq!(scc.component_successors, vec![vec![]]);
    }

    #[test]
    fn strongly_connected_components_groups_mutual_recursion() {
        let edges = HashMap::from([
            ("even", vec!["odd"]),
            ("odd", vec!["even"]),
            ("entry", vec!["even"]),
        ]);
        let scc = strongly_connected_components(["entry"], |node| edges[node].clone());

        assert_eq!(
            component_sets(scc.components),
            HashSet::from([vec!["entry"], vec!["even", "odd"]])
        );
        assert_eq!(scc.component_by_node["even"], scc.component_by_node["odd"]);
        assert_ne!(
            scc.component_by_node["entry"],
            scc.component_by_node["even"]
        );
        assert_eq!(
            scc.component_successors[scc.component_by_node["entry"]],
            vec![scc.component_by_node["even"]]
        );
        assert!(scc.component_successors[scc.component_by_node["even"]].is_empty());
    }

    #[test]
    fn strongly_connected_components_follows_successors_not_listed_as_roots() {
        let edges = HashMap::from([("a", vec!["b"]), ("b", vec!["c"]), ("c", vec!["b"])]);
        let scc = strongly_connected_components(["a"], |node| edges[node].clone());

        assert_eq!(
            component_sets(scc.components),
            HashSet::from([vec!["a"], vec!["b", "c"]])
        );
        assert_eq!(scc.component_by_node["b"], scc.component_by_node["c"]);
    }
}
