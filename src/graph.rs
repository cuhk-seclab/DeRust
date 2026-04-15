use crate::ir::*;
use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use rustc_span::Span;
use std::{cmp::min, collections::VecDeque};

use crate::analysis::{FunctionSources, TypeBehaviorFlag};
use crate::ir;

pub trait Graph {
    fn len(&self) -> usize;
    fn next(&self, id: usize) -> Vec<usize>;
    fn node_len(&self) -> usize;
}

impl<'tcx> Graph for ir::Body<'tcx> {
    fn len(&self) -> usize {
        self.basic_blocks.len()
    }

    fn next(&self, id: usize) -> Vec<usize> {
        self.basic_blocks[id]
            .terminator
            .original
            .successors()
            .map(|block| block.index())
            // .map(|block| block.as_usize())
            .collect()
    }

    fn node_len(&self) -> usize {
        self.local_nodes.len()
    }
}

pub trait GraphTaint: Clone + Default {
    fn is_empty(&self) -> bool;
    fn contains(&self, taint: &Self) -> bool;
    fn join(&mut self, taint: &Self);
}

// ToDO: replace TaintAnalyzerPath name
#[derive(Debug, Clone)]
pub struct TaintAnalyzerPath<'a, G: Graph, T> {
    graph: &'a G,
    len: usize,
    sources: Vec<Vec<T>>, // record localnodes with several source operations
    sinks: Vec<Vec<T>>,   // record localnodes with sink (chain) operations
                          // ToDO: consider sink chain, e.g., Vec<Vec<usize>>...
                          // sinks: Vec<bool>,     // record bool with sink (chain) operations -> deprecated
}

impl<'a, G: Graph, T: Clone + PartialEq + std::fmt::Debug> TaintAnalyzerPath<'a, G, T> {
    pub fn new(graph: &'a G) -> Self {
        let node_len = graph.node_len();
        TaintAnalyzerPath {
            graph,
            len: node_len,
            sources: vec![vec![]; node_len],
            sinks: vec![vec![]; node_len],
        }
    }

    pub fn graph(&self) -> &G {
        &self.graph
    }

    // id: LocalNode index, taint: the type state machine, or TypeBehaviorFlag?
    // ToDO: if it's the owned/initialized, stack/heap, or it doesn't matter? Or lattice?
    pub fn mark_source(&mut self, id: usize, taint: T) {
        self.sources[id].push(taint);
    }

    pub fn clear_source(&mut self, id: usize) {
        self.sources[id] = Vec::<T>::new();
        // self.sources[id] = vec![];
    }

    pub fn mark_sink(&mut self, id: usize, sink: T) {
        self.sinks[id].push(sink);
    }

    pub fn unmark_sink(&mut self, id: usize) {
        self.sinks[id] = Vec::<T>::new();
    }

    // Unmark all sources and sinks
    pub fn clear(&mut self) {
        self.sources = vec![vec![]; self.len];
        self.sinks = vec![vec![]; self.len];
    }

    // Checks reachability between `self.sources` & `self.sinks`.
    // Retuen tuple vectors, indexed by sink ids -> Vec<(sink, Vec<TypeBehaviorFlag>, source, Vec<TypeBehaviorFlag>)>
    pub fn propagate(
        &mut self,
        local_nodes: Vec<LocalNode>,
        function_sources: &FunctionSources,
        // source_operations: &FxHashMap<usize, Vec<Span>>,
        arg_size: usize,
    ) -> (
        Vec<(usize, Vec<T>, usize, Vec<T>)>,
        FunctionSources,
        FunctionSources,
    ) {
        let mut ret: Vec<(usize, Vec<T>, usize, Vec<T>)> = Vec::new();
        let mut taint_state: Vec<(usize, Vec<T>)> = vec![(0, vec![]); self.len]; // usize is to record original source index, no need to output alias span
                                                                                 // let mut taint_state: Vec<FxHashMap<usize, Vec<T>>> = vec![FxHashMap::default(); self.len];   // Replace HashMap with Tuple
        let mut work_list = VecDeque::new();

        // Initialize work list
        for id in 0..self.len {
            if !self.sources[id].is_empty() {
                taint_state[id] = (id, self.sources[id].clone());
                // taint_state[id].insert(id, self.sources[id].clone());
                work_list.push_back(id);
            }
        }

        // Propagation sources
        while let Some(current) = work_list.pop_front() {
            let new_alias: Vec<usize> = local_nodes[current]
                .alias
                .iter()
                .skip_while(|&index| *index != current)
                .skip(1) // Skip current localnode, get its later alias -> ToDO: is it correct in fixed-point iteration?
                .cloned()
                .collect();
            for alias in new_alias {
                let mut next_state = std::mem::take(&mut taint_state[alias]);
                let taint = &taint_state[current];
                let taint_values = taint.1.clone();
                let mut next_state_values = next_state.1.clone();
                // let taint_values: Vec<T> = taint.values().flatten().cloned().collect();
                // let mut next_state_values: Vec<T> = next_state.values().flatten().cloned().collect();
                if !taint_values
                    .iter()
                    .all(|elem| next_state_values.contains(elem))
                {
                    next_state_values.extend(taint_values.clone());
                    next_state = (current, next_state_values);
                    // next_state = FxHashMap::default();
                    // next_state.insert(current, next_state_values);
                }
                taint_state[alias] = next_state;
            }
        }

        // Construct propagated function sources -> Same procedure as the above
        //      function_sources usize is any size
        //      function_sources_propagated usize is within arg_size + 1
        // let mut function_sources_propagated: FxHashMap<usize, Vec<(Span, TypeBehaviorFlag)>> = function_sources.clone();
        let mut function_sources_taint: FxHashMap<usize, Vec<(Span, TypeBehaviorFlag)>> =
            function_sources.clone();
        // let mut source_operations_taint: FxHashMap<usize, Vec<Span>> = source_operations.clone();

        let mut source_list =
            VecDeque::from(function_sources.keys().cloned().collect::<Vec<usize>>());

        while let Some(current) = source_list.pop_front() {
            let new_alias: Vec<usize> = local_nodes[current]
                .alias
                .iter()
                .skip_while(|&index| *index != current)
                .skip(1) // Skip current localnode, get its later alias -> Including current localnode -> ToDO: Can even iterate all the localnode
                .cloned()
                .collect();
            for alias in new_alias {
                if function_sources_taint.contains_key(&alias) {
                    let mut next_state =
                        std::mem::take(function_sources_taint.get_mut(&alias).unwrap());
                    let taint = &function_sources_taint[&current];
                    if !taint.iter().all(|elem| next_state.contains(elem)) {
                        next_state.extend(taint.clone());
                    }
                    function_sources_taint.insert(alias, next_state);
                } else {
                    let taint = function_sources_taint[&current].clone();
                    function_sources_taint.insert(alias, taint);
                }

                // if source_operations_taint.contains_key(&alias) {
                //     let mut next_state =
                //         std::mem::take(source_operations_taint.get_mut(&alias).unwrap());
                //     let taint = &source_operations_taint[&current];
                //     if !taint.iter().all(|elem| next_state.contains(elem)) {
                //         next_state.extend(taint.clone());
                //     }
                //     source_operations_taint.insert(alias, next_state);
                // } else {
                //     let taint = source_operations_taint[&current].clone();
                //     source_operations_taint.insert(alias, taint);
                // }
            }
        }

        let mut function_sources_propagated: FxHashMap<usize, Vec<(Span, TypeBehaviorFlag)>> =
            FunctionSources::default();
        for (idx, value) in function_sources_taint.iter() {
            if *idx <= arg_size {
                function_sources_propagated.insert(*idx, value.clone());
            }
        }

        // Join all taints in the sink nodes
        // let mut ret = T::default();
        for id in 0..self.len {
            if !self.sinks[id].is_empty() && !taint_state[id].1.is_empty() {
                // ret.join(&taint_state[id]);
                ret.push((
                    id,
                    self.sinks[id].clone(),
                    taint_state[id].0,
                    taint_state[id].1.clone(),
                ));
            }
        }

        (ret, function_sources_propagated, function_sources_taint)
    }
}

#[derive(Debug, Clone)]
pub struct TaintAnalyzer<'a, G: Graph, T: GraphTaint> {
    graph: &'a G,
    len: usize,
    sources: Vec<T>,
    sinks: Vec<bool>,
}

impl<'a, G: Graph, T: GraphTaint> TaintAnalyzer<'a, G, T> {
    pub fn new(graph: &'a G) -> Self {
        let graph_len = graph.len();
        TaintAnalyzer {
            graph,
            len: graph_len,
            sources: vec![T::default(); graph_len],
            sinks: vec![false; graph_len],
        }
    }

    pub fn graph(&self) -> &G {
        &self.graph
    }

    pub fn mark_source(&mut self, id: usize, taint: &T) {
        self.sources[id].join(taint);
    }

    pub fn clear_source(&mut self, id: usize) {
        self.sources[id] = T::default();
    }

    pub fn mark_sink(&mut self, id: usize) {
        self.sinks[id] = true;
    }

    pub fn unmark_sink(&mut self, id: usize) {
        self.sinks[id] = false;
    }

    // Unmark all sources and sinks
    pub fn clear(&mut self) {
        self.sources = vec![T::default(); self.len];
        self.sinks = vec![false; self.len];
    }

    // Checks reachability between `self.sources` & `self.sinks`.
    pub fn propagate(&self) -> T {
        let mut taint_state = vec![T::default(); self.len];
        let mut work_list = VecDeque::new();

        // Initialize work list
        for id in 0..self.len {
            if !self.sources[id].is_empty() {
                taint_state[id].join(&self.sources[id]);
                work_list.push_back(id);
            }
        }

        // Breadth-first propagation
        while let Some(current) = work_list.pop_front() {
            for next in self.graph.next(current) {
                let mut next_state = std::mem::take(&mut taint_state[next]);
                let taint = &taint_state[current];
                if !next_state.contains(taint) {
                    next_state.join(taint);
                    work_list.push_back(next);
                }
                taint_state[next] = next_state;
            }
        }

        // Join all taints in the sink nodes
        let mut ret = T::default();
        for id in 0..self.len {
            if self.sinks[id] && !taint_state[id].is_empty() {
                ret.join(&taint_state[id]);
            }
        }

        return ret;
    }
}

/// Strongly Connected Component (SCC) using Tarjan's algorithm
#[derive(Debug, Clone)]
pub struct Scc<'a, G: Graph> {
    graph: &'a G, // ir::Body, which implements Graph trait
    /// group number of each item (indexed by node)
    group_of_node: Vec<usize>,
    /// nodes in each SCC group (indexed by group)
    /// The first node in each SCC group: father_block; Other nodes in each SCC group: sub_blocks
    nodes_in_group: Vec<Vec<usize>>,
    /// adjacency list of groups (indexed by group). Spanning graph.
    group_graph: Vec<Vec<usize>>,
    /// SccState
    pub scc_num: usize,
    pub local_num: usize,
    pub scc_alias: Vec<Vec<Vec<usize>>>,
}

/// Temporary state variable used in SCC construction
#[derive(Debug, Clone)]
struct SccConstructionState {
    // intermediate state
    current_index: usize,
    stack: Vec<usize>, // Traversal paths
    index: Vec<usize>, // Node traversal order
    // result
    group_of_node: Vec<usize>,
    nodes_in_group: Vec<Vec<usize>>,
}

impl SccConstructionState {
    fn new(size: usize) -> Self {
        SccConstructionState {
            current_index: 0,
            stack: Vec::new(),
            index: vec![0; size],
            group_of_node: vec![0; size],
            nodes_in_group: Vec::new(),
        }
    }

    fn assign_id(&mut self, node: usize) {
        self.current_index += 1;
        self.index[node] = self.current_index;
    }
}

#[derive(Debug, Clone)]
struct SccTopologicalOrderState {
    visited: Vec<bool>,
    order: Vec<usize>,
}

impl SccTopologicalOrderState {
    fn new(size: usize) -> Self {
        SccTopologicalOrderState {
            visited: vec![false; size],
            order: Vec::new(),
        }
    }
}

impl<'a, G: Graph> Scc<'a, G> {
    pub fn construct(graph: &'a G) -> Self {
        let num_node = graph.len();
        let mut state = SccConstructionState::new(num_node);

        // construct SCC
        for node in 0..num_node {
            if state.index[node] == 0 {
                Scc::traverse(graph, &mut state, node);
            }
        }

        // collect all inter-group edges, no loop
        let num_group = state.nodes_in_group.len();
        let mut group_graph = vec![Vec::new(); num_group];
        for from in 0..num_node {
            for to in graph.next(from).into_iter() {
                // println!("from: {}, to: {}", from, to);
                let from_group = state.group_of_node[from];
                let to_group = state.group_of_node[to];
                // println!("from_group: {}, to_group: {}", from_group, to_group);
                if from_group != to_group {
                    group_graph[from_group].push(to_group);
                }
            }
        }

        // remove duplicated edges
        for group in 0..num_group {
            let edges = &mut group_graph[group];
            edges.sort();
            edges.dedup();
        }

        let SccConstructionState {
            group_of_node,
            nodes_in_group,
            ..
        } = state;

        let scc_num = num_group;
        let local_num = graph.node_len();
        let mut scc_alias = Vec::new();
        for _ in 0..scc_num {
            let mut scc_alias_set = Vec::new();
            for local_idx in 0..local_num {
                let mut alias = Vec::new();
                alias.push(local_idx);
                scc_alias_set.push(alias);
            }
            scc_alias.push(scc_alias_set);
        }

        Scc {
            graph,
            group_of_node,
            nodes_in_group,
            group_graph,
            scc_num: scc_num,
            local_num: local_num,
            scc_alias: scc_alias,
        }
    }

    // returns the lowest reachable node
    fn traverse(graph: &'a G, state: &mut SccConstructionState, node: usize) -> usize {
        state.assign_id(node);
        state.stack.push(node);

        // Node lowest reachable index
        let mut low_link = state.index[node];
        for next in graph.next(node).into_iter() {
            if state.index[next] == 0 {
                // not visited yet
                low_link = min(low_link, Scc::traverse(graph, state, next));
            } else if state.group_of_node[next] == 0 {
                // already in stack, not grouped yet
                low_link = min(low_link, state.index[next]);
            }
        }

        // SCC boundary found, node is the root node
        if low_link == state.index[node] {
            // all nodes in the stack after this node belongs to the same group
            let mut new_group = Vec::new();
            let group_num = state.nodes_in_group.len();
            // let group_num = state.nodes_in_group.len() + 1;
            loop {
                let now = state.stack.pop().unwrap();
                state.group_of_node[now] = group_num;
                new_group.push(now);

                if now == node {
                    break;
                }
            }
            // new_group order is the backtrack order in tarjan, should be reversed
            new_group.reverse();
            state.nodes_in_group.push(new_group);
        }

        low_link
    }

    fn topological_dfs(&self, state: &mut SccTopologicalOrderState, group: usize) {
        state.visited[group] = true;
        state.order.push(group);
        for &next_group in self.next_groups(group).iter() {
            if !state.visited[next_group] {
                self.topological_dfs(state, next_group)
            }
        }
    }

    pub fn topological_order(&self) -> Vec<usize> {
        let num_group = self.group_graph.len();
        let mut state = SccTopologicalOrderState::new(num_group);

        for group in 0..num_group {
            if !state.visited[group] {
                self.topological_dfs(&mut state, group);
            }
        }

        // One-dimensional vector
        let mut result = state.order;
        result.reverse();
        result
    }

    fn get_groups_with_zero_indegree(&self) -> Vec<usize> {
        let mut indegree = vec![0; self.group_graph.len()];
        for group in &self.group_graph {
            for &next_group in group.iter() {
                indegree[next_group] += 1;
            }
        }
        let mut result = Vec::new();
        for (idx, &degree) in indegree.iter().enumerate() {
            if degree == 0 {
                result.push(idx);
            }
        }
        result
    }

    fn path_dfs(
        &self,
        state: &mut SccTopologicalOrderState,
        group: usize,
        paths: &mut Vec<Vec<usize>>,
    ) {
        state.visited[group] = true;
        state.order.push(group);
        if self.next_groups(group).is_empty() {
            // Leaf node
            paths.push(state.order.clone());
        } else {
            for &next_group in self.next_groups(group).iter() {
                if !state.visited[next_group] {
                    self.path_dfs(state, next_group, paths);
                }
            }
        }
        // Backtrack
        state.order.pop();
        state.visited[group] = false;
    }

    pub fn scc_paths(&self) -> Vec<Vec<usize>> {
        let num_group = self.group_graph.len();
        let mut state = SccTopologicalOrderState::new(num_group);
        let mut scc_paths: Vec<Vec<usize>> = Vec::new();

        for group in self.get_groups_with_zero_indegree() {
            if !state.visited[group] {
                self.path_dfs(&mut state, group, &mut scc_paths);
            }
        }

        scc_paths
    }

    // Indexed by group/scc
    pub fn nodes_in_scc(&self, index: usize) -> (usize, &[usize]) {
        let nodes = &self.nodes_in_group[index];
        let root = nodes[0];
        let subs = &nodes[1..];
        (root, subs)
    }

    pub fn paths(&self) -> Vec<Vec<usize>> {
        let mut paths = Vec::new();
        for group in self.scc_paths() {
            let mut path = Vec::new();
            for &index in &group {
                let (root, subs) = self.nodes_in_scc(index);
                path.push(root);
                path.extend(subs);
            }
            paths.push(path);
        }
        paths
    }

    // Scc State utilities
    pub fn predecessors_of_scc(&self, index: usize) -> Vec<usize> {
        let mut result = Vec::new();
        for (group_idx, group) in self.group_graph.iter().enumerate() {
            if group.contains(&index) {
                result.push(group_idx);
            }
        }
        result
    }

    pub fn successors_of_scc(&self, index: usize) -> Vec<usize> {
        self.group_graph[index].clone()
    }

    /*
    pub fn scc_alias_by_index(&self, index: usize) -> &Vec<Vec<usize>> {
        &self.scc_alias[index]
    }

    pub fn scc_alias(&self) -> &Vec<Vec<Vec<usize>>> {
        &self.scc_alias
    }

    pub fn update_scc_alias_by_index(&mut self, index: usize, alias: &Vec<Vec<usize>>) {
        self.scc_alias[index] = alias.clone();
    }
     */

    // Indexed by node
    pub fn get_scc_root_node(&self, index: usize) -> usize {
        let group_index = self.group_of_node(index);
        self.nodes_in_group[group_index][0]
    }

    pub fn graph(&self) -> &G {
        &self.graph
    }

    pub fn group_of_node(&self, idx: usize) -> usize {
        self.group_of_node[idx]
    }

    pub fn nodes_in_group(&self, idx: usize) -> &[usize] {
        &self.nodes_in_group[idx]
    }

    pub fn nodes_in_group_len(&self, idx: usize) -> usize {
        self.nodes_in_group[idx].len()
    }

    pub fn next_groups(&self, group_idx: usize) -> &[usize] {
        &self.group_graph[group_idx]
    }

    pub fn group_len(&self) -> usize {
        self.nodes_in_group.len()
    }
}

impl<'tcx> ir::Body<'tcx> {
    pub fn solve_scc(&self) -> Scc<'_, Self> {
        Scc::construct(self)
    }
}
