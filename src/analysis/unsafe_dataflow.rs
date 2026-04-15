use rustc_data_structures::fx::FxHashMap;
use rustc_data_structures::fx::FxHashSet;
use rustc_hir::{def_id::DefId, BodyId};
use rustc_infer::infer::InferCtxt;
use rustc_infer::infer::TyCtxtInferExt;
use rustc_middle::mir;
use rustc_middle::mir::{Operand, Place, ProjectionElem};
use rustc_middle::ty::layout::SizeSkeleton;
use rustc_middle::ty::{FloatTy, IntTy, Mutability, TypeAndMut, UintTy};
use rustc_middle::ty::{Instance, ParamEnv, Ty, TyCtxt, TyKind};
use rustc_span::Span;

use snafu::{Backtrace, Snafu};
use std::rc::Rc;
use std::{collections::VecDeque, mem};
use termcolor::Color;

use crate::graph;
use crate::graph::GraphTaint;
use crate::graph::*;
use crate::ir::*;
use crate::prelude::*;
use crate::{
    analysis::{AnalysisKind, IntoReportLevel},
    graph::TaintAnalyzer,
    ir,
    paths::{self, *},
    report::{Report, ReportLevel},
    traitlist::*,
    utils,
    visitor::ContainsUnsafe,
};

const MAX_ITERATION: u32 = 200;
const EXTEND_COEFFICIENT: f64 = 2.0;
// const EXTEND_COEFFICIENT: f64 = 3.0 / 2.0;
const MAX_INTER_DEPTH: u32 = 20;

// 1) Store any local_decls with taint sources
// 2) Store function signatures (args and returns) with taint sources; usize is within arg_size + 1 -> Main purpose
//      ToDO: Can also be changed to vec![]
pub type FunctionSources = FxHashMap<usize, Vec<(Span, TypeBehaviorFlag)>>;
// Store function input args and their aliases
// pub type FunctionInputState = FxHashMap<usize, Vec<usize>>;
pub type FunctionInputState = Vec<Vec<usize>>;

// ToDO: SccState and BbState can be merged. For fixed-point iteration
#[derive(Debug, Default, Clone)]
pub struct SccState {
    pub scc_num: usize,
    pub local_num: usize, // Be larger than local_decls since field-sensitive
    pub scc_alias: Vec<Vec<Vec<usize>>>, // Lattice is alias
}

impl SccState {
    pub fn new(scc_num: usize, local_num: usize) -> Self {
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

        SccState {
            scc_num,
            local_num,
            scc_alias,
        }
    }

    pub fn scc_alias_by_index(&self, index: usize) -> &Vec<Vec<usize>> {
        &self.scc_alias[index]
    }

    pub fn scc_alias(&self) -> &Vec<Vec<Vec<usize>>> {
        &self.scc_alias
    }

    pub fn update_scc_alias_by_index(&mut self, index: usize, alias: &Vec<Vec<usize>>) {
        self.scc_alias[index] = alias.clone();
    }
}

#[derive(Debug, Default, Clone)]
pub struct BbState {
    pub bb_num: usize,
    pub local_num: usize,
    pub bb_alias: FxHashMap<usize, Vec<Vec<usize>>>,
}

impl BbState {
    pub fn new(bb_list: Vec<usize>, local_num: usize) -> Self {
        let mut bb_alias = FxHashMap::default();
        for bb_index in bb_list.iter() {
            let mut bb_alias_set = Vec::new();
            for local_idx in 0..local_num {
                let mut alias = Vec::new();
                alias.push(local_idx);
                bb_alias_set.push(alias);
            }
            bb_alias.insert(*bb_index, bb_alias_set);
        }

        BbState {
            bb_num: bb_list.len(),
            local_num,
            bb_alias,
        }
    }

    // Ensure that all the indexes are valid / keys
    pub fn bb_alias_by_index(&self, index: usize) -> &Vec<Vec<usize>> {
        self.bb_alias.get(&index).unwrap()
    }

    pub fn bb_alias(&self) -> &FxHashMap<usize, Vec<Vec<usize>>> {
        &self.bb_alias
    }

    pub fn update_bb_alias_by_index(&mut self, index: usize, alias: &Vec<Vec<usize>>) {
        self.bb_alias.insert(index, alias.clone());
    }
}

#[derive(Debug, Snafu)]
pub enum UnsafeDataflowError {
    PushPopBlock { backtrace: Backtrace },
    ResolveError { backtrace: Backtrace },
    InvalidSpan { backtrace: Backtrace },
}

impl AnalysisError for UnsafeDataflowError {
    fn kind(&self) -> AnalysisErrorKind {
        use UnsafeDataflowError::*;
        match self {
            PushPopBlock { .. } => AnalysisErrorKind::Unreachable,
            ResolveError { .. } => AnalysisErrorKind::OutOfScope,
            InvalidSpan { .. } => AnalysisErrorKind::Unreachable,
        }
    }
}

pub struct UnsafeDataflowChecker<'tcx> {
    rcx: RudraCtxt<'tcx>, // HIR visitor and configs
}

impl<'tcx> UnsafeDataflowChecker<'tcx> {
    pub fn new(rcx: RudraCtxt<'tcx>) -> Self {
        UnsafeDataflowChecker { rcx }
    }

    /*
    pub fn analyze(self) {
        let tcx = self.rcx.tcx();
        let hir_map = tcx.hir();

        for (_ty_hir_id, (body_id, related_item_span)) in self.rcx.types_with_related_items() {
            if let Some(status) = inner::UnsafeDataflowBodyAnalyzer::analyze_body(self.rcx, body_id)
            {
                let behavior_flag = status.behavior_flag();
                if !behavior_flag.is_empty()
                    && behavior_flag.report_level() >= self.rcx.report_level()
                {
                    println!("behavior_flag: {:?}", behavior_flag);
                    let mut color_span = unwrap_or!(
                        utils::ColorSpan::new(tcx, related_item_span).context(InvalidSpan) => continue
                    );

                    for &span in status.strong_bypass_spans() {
                        color_span.add_sub_span(Color::Red, span);
                    }

                    for &span in status.weak_bypass_spans() {
                        color_span.add_sub_span(Color::Yellow, span);
                    }

                    for &span in status.unresolvable_generic_function_spans() {
                        color_span.add_sub_span(Color::Cyan, span);
                    }

                    for &span in status.type_conversion_spans() {
                        color_span.add_sub_span(Color::Blue, span);
                    }

                    rudra_report(Report::with_color_span(
                        tcx,
                        behavior_flag.report_level(),
                        AnalysisKind::UnsafeDataflow(behavior_flag),
                        format!(
                            "Potential unsafe dataflow issue in `{}`",
                            tcx.def_path_str(hir_map.body_owner_def_id(body_id).to_def_id())
                        ),
                        &color_span,
                    ))
                }
            }
        }
    }
     */

    pub fn bug_analyze(self) {
        let tcx = self.rcx.tcx();
        let hir_map = tcx.hir();

        let mut bug_count: u32 = 0;
        let mut safe_count: u32 = 0;
        let mut unsafe_count: u32 = 0;
        let mut func_summary = FunctionSummary::new();

        // Iterates all (type, related function) pairs.
        for (_ty_hir_id, (body_id, related_item_span)) in self.rcx.types_with_related_items() {
            /*
                       print!(
                           r"
               ____             __     ____              ________              __
               / __ \__  _______/ /_   / __ )__  ______ _/ ____/ /_  ___  _____/ /_____  _____
              / /_/ / / / / ___/ __/  / __  / / / / __ `/ /   / __ \/ _ \/ ___/ //_/ _ \/ ___/
             / _, _/ /_/ (__  ) /_   / /_/ / /_/ / /_/ / /___/ / / /  __/ /__/ ,< /  __/ /
            /_/ |_|\__,_/____/\__/  /_____/\__,_/\__, /\____/_/ /_/\___/\___/_/|_|\___/_/
                                                /____/

                       "
                       );
                        */
            let count = inner::UnsafeDataflowBodyAnalyzer::analyze_body(
                self.rcx,
                body_id,
                related_item_span,
                &mut func_summary,
            );

            if count > 0 {
                bug_count += count;
            }
            // Imitate the Report procedure
            /*
            if let Some(status) = inner::UnsafeDataflowBodyAnalyzer::analyze_body(self.rcx, body_id)
            {

                let behavior_flag = status.behavior_flag();
                if !behavior_flag.is_empty()
                    && behavior_flag.report_level() >= self.rcx.report_level()
                {
                    println!("behavior_flag: {:?}", behavior_flag);
                    let mut color_span = unwrap_or!(
                        utils::ColorSpan::new(tcx, related_item_span).context(InvalidSpan) => continue
                    );

                    for &span in status.strong_bypass_spans() {
                        color_span.add_sub_span(Color::Red, span);
                    }

                    for &span in status.weak_bypass_spans() {
                        color_span.add_sub_span(Color::Yellow, span);
                    }

                    for &span in status.unresolvable_generic_function_spans() {
                        color_span.add_sub_span(Color::Cyan, span);
                    }

                    for &span in status.type_conversion_spans() {
                        color_span.add_sub_span(Color::Blue, span);
                    }

                    rudra_report(Report::with_color_span(
                        tcx,
                        behavior_flag.report_level(),
                        AnalysisKind::UnsafeDataflow(behavior_flag),
                        format!(
                            "Potential unsafe dataflow issue in `{}`",
                            tcx.def_path_str(hir_map.body_owner_def_id(body_id).to_def_id())
                        ),
                        &color_span,
                    ))
                }
            }
            */
        }
    }

    pub fn lifetime_analyze(self) {}
}

mod inner {
    use super::*;

    #[derive(Debug, Default, Clone)]
    pub struct UnsafeDataflowStatus {
        strong_bypasses: Vec<Span>,
        weak_bypasses: Vec<Span>,
        type_conversion: Vec<Span>,
        unresolvable_generic_functions: Vec<Span>,
        behavior_flag: BehaviorFlag,
    }

    impl UnsafeDataflowStatus {
        pub fn behavior_flag(&self) -> BehaviorFlag {
            self.behavior_flag
        }

        pub fn strong_bypass_spans(&self) -> &Vec<Span> {
            &self.strong_bypasses
        }

        pub fn weak_bypass_spans(&self) -> &Vec<Span> {
            &self.weak_bypasses
        }

        pub fn type_conversion_spans(&self) -> &Vec<Span> {
            &self.type_conversion
        }

        pub fn unresolvable_generic_function_spans(&self) -> &Vec<Span> {
            &self.unresolvable_generic_functions
        }
    }

    // In the normal case, one usize node only has one span; Can be used in several statements, then it's Vec<Span>. Should be duplicated.
    #[derive(Debug, Default, Clone)]
    pub struct TypeDataflowStatus {
        source_operations: FxHashMap<usize, Vec<Span>>,
        sink_operations: FxHashMap<usize, Vec<Span>>,
    }

    impl TypeDataflowStatus {
        pub fn source_operation_spans(&self) -> &FxHashMap<usize, Vec<Span>> {
            &self.source_operations
        }

        pub fn sink_operation_spans(&self) -> &FxHashMap<usize, Vec<Span>> {
            &self.sink_operations
        }
    }

    pub struct UnsafeDataflowBodyAnalyzer<'a, 'tcx> {
        rcx: RudraCtxt<'tcx>,
        body: &'a mut ir::Body<'tcx>,
        scc: graph::Scc<'a, ir::Body<'tcx>>,
        // scc_state: SccState,
        func_summary: &'a mut FunctionSummary,
        param_env: ParamEnv<'tcx>,
        status: TypeDataflowStatus,
        // paths: Vec<Vec<usize>>,
        // status: UnsafeDataflowStatus,
    }

    impl<'a, 'tcx> UnsafeDataflowBodyAnalyzer<'a, 'tcx> {
        fn new(
            rcx: RudraCtxt<'tcx>,
            param_env: ParamEnv<'tcx>,
            body: &'a mut ir::Body<'tcx>,
            scc: graph::Scc<'a, ir::Body<'tcx>>,
            func_summary: &'a mut FunctionSummary,
            // scc_num: usize,
            // local_num: usize,
            // paths: Vec<Vec<usize>>,
        ) -> Self {
            UnsafeDataflowBodyAnalyzer {
                rcx,
                body,
                scc,
                // scc_state: SccState::new(scc_num, local_num),
                func_summary: func_summary,
                param_env,
                status: TypeDataflowStatus::default(),
            }
        }

        pub fn body_mut(&mut self) -> &ir::Body<'tcx> {
            &self.body
        }

        pub fn status_mut(&mut self) -> &mut TypeDataflowStatus {
            &mut self.status
        }

        pub fn scc(&mut self) -> &mut graph::Scc<'a, ir::Body<'tcx>> {
            &mut self.scc
        }

        pub fn scc_immut(&self) -> &graph::Scc<'a, ir::Body<'tcx>> {
            &self.scc
        }

        /*
        pub fn scc_state(&mut self) -> &mut SccState {
            &mut self.scc_state
        }
         */

        fn has_projection(&mut self, place: Place) -> bool {
            return if place.projection.len() > 0 {
                true
            } else {
                false
            };
        }

        fn has_projection_immut(&self, place: Place) -> bool {
            return if place.projection.len() > 0 {
                true
            } else {
                false
            };
        }

        fn has_deref_projection(&self, place: Place) -> bool {
            place
                .projection
                .iter()
                .any(|elem| matches!(elem, mir::ProjectionElem::Deref))
        }

        pub fn get_localnode_alias(&mut self, local_num: usize) -> Vec<Vec<usize>> {
            let mut alias: Vec<Vec<usize>> = Vec::with_capacity(local_num);
            for (idx, local_node) in self.body.local_nodes.iter().enumerate() {
                alias.push(local_node.alias.clone());
            }
            for idx in self.body.local_nodes.len()..local_num {
                let mut self_alias = Vec::new();
                self_alias.push(idx);
                alias.push(self_alias);
            }
            alias
        }

        pub fn update_localnode_alias(&mut self, alias_state: &Vec<Vec<usize>>) {
            for idx in 0..self.body.local_nodes.len() {
                self.body.local_nodes[idx].alias = alias_state[idx].clone();
            }
        }

        pub fn get_function_input_alias(&mut self) -> FunctionInputState {
            let mut function_input_state = FunctionInputState::default();
            for (idx, local_node) in self.body.local_nodes.iter().enumerate() {
                if idx > 0 && idx <= self.body.arg_size {
                    function_input_state.push(local_node.alias.clone());
                } else if idx == 0 {
                    function_input_state.push(Vec::new());
                }
            }
            function_input_state
        }

        // pub fn analyze_body(rcx: RudraCtxt<'tcx>, body_id: BodyId) -> Option<UnsafeDataflowStatus> {
        pub fn analyze_body(
            rcx: RudraCtxt<'tcx>,
            body_id: BodyId,
            related_item_span: Span,
            func_summary: &mut FunctionSummary,
        ) -> u32 {
            let hir_map = rcx.tcx().hir();
            // BodyId -> LocalDefId -> DefId
            let body_did = hir_map.body_owner_def_id(body_id).to_def_id();

            if rcx.tcx().ext().match_def_path(
                body_did,
                &["rudra_paths_discovery", "PathsDiscovery", "discover"], // Could add more Symbol paths
            ) {
                // Special case for paths discovery
                trace_calls_in_body(rcx, body_did);
                0
                // None
            } else {
                // else if ContainsUnsafe::contains_unsafe(rcx.tcx(), body_id) {
                match rcx.translate_body(body_did).as_ref() {
                    Err(e) => {
                        // MIR is not available for def - log it and continue
                        e.log();
                        0
                        // None
                    }
                    Ok(body) => {
                        let param_env = rcx.tcx().param_env(body_did);
                        let mut body_clone = body.clone(); // Pay attention to clone! -> It clone the object at a certain status
                        let body_mut = &mut body_clone;

                        let scc = body.solve_scc();
                        let scc_num = scc.group_len();
                        let local_num = body.local_nodes.len(); // Would be inserted
                        let local_extend_num = (local_num as f64 * EXTEND_COEFFICIENT) as usize; // Could be enlarged in large repo

                        let mut body_analyzer = UnsafeDataflowBodyAnalyzer::new(
                            rcx,
                            param_env,
                            body_mut,
                            scc,
                            func_summary,
                        );
                        let mut scc_state = SccState::new(scc_num, local_extend_num);

                        let mut current_depth: u32 = 1;
                        let paths = body_analyzer.scc().paths();
                        let scc_paths = body_analyzer.scc().scc_paths().clone();
                        /*
                        for (idx, path) in paths.iter().enumerate() {
                            println!("Path idx: {:?}, path: {:?}", idx, path);
                        }
                        for (idx, scc_path) in scc_paths.iter().enumerate() {
                            println!("SCC Path idx: {:?}, scc_path: {:?}", idx, scc_path);
                        }
                         */
                        // 1. Fixed-point iterations with SCCs, Worklist algorithm using Lattice
                        //      SCC -> Find predecessors and successors
                        //          -> Each SCC has state flag -> Determine whether the state has been updated
                        //    a. Initialize all the SCC worklist
                        //    b. Pop one SCC
                        //    c. Merge the predecessors, update; Update the current SCC state -> Update the state flag
                        //    e. Push the successors into the worklist
                        //    f. Until the worklist is empty, or iteration times exceed the threshold
                        // 2. Function summary
                        // 3. Bug checkers
                        //       Can just analyze all the bbs, al iterate all the paths -> TaintAnalyzer for all the LocalNodes

                        let mut work_list: VecDeque<usize> = VecDeque::new(); // SCC index
                        for idx in 0..scc_num {
                            work_list.push_back(idx);
                        }

                        let mut iteration = 0;
                        while let Some(scc_index) = work_list.pop_front() {
                            if iteration > 0 {
                                let body_analyzer_scc = body_analyzer.scc();
                                let old_state = scc_state.scc_alias_by_index(scc_index);
                                let predecessors = body_analyzer_scc.predecessors_of_scc(scc_index);
                                let successors = body_analyzer_scc.successors_of_scc(scc_index);

                                // Get and merge the predecessors -> Update the predecessors' state -> Would break the flow-sensitive order
                                let mut merge_predecessors: Vec<Vec<usize>> =
                                    Vec::with_capacity(local_extend_num);
                                for (idx, predecessor) in predecessors.iter().enumerate() {
                                    if idx == 0 {
                                        merge_predecessors =
                                            scc_state.scc_alias_by_index(*predecessor).to_vec();
                                    } else {
                                        merge_predecessors = merge_scc_state(
                                            local_extend_num,
                                            &merge_predecessors,
                                            scc_state.scc_alias_by_index(*predecessor),
                                        );
                                    }
                                }

                                if predecessors.len() > 0 {
                                    // Replace not merge
                                    body_analyzer.update_localnode_alias(&merge_predecessors);
                                }
                                // KEY: Alias analysis for the current SCC
                                body_analyzer.alias_analysis(
                                    scc_index,
                                    local_extend_num,
                                    body_id,
                                    current_depth,
                                );

                                // Get alias from local_nodes
                                let new_state = body_analyzer.get_localnode_alias(local_extend_num);
                                // println!("old_state: {:?}", old_state);
                                // println!("new_state: {:?}", new_state);
                                // Compare two states -> Update alias into scc_state, push the successors
                                if !compare_scc_state(local_extend_num, old_state, &new_state) {
                                    scc_state.update_scc_alias_by_index(scc_index, &new_state);
                                    for successor in successors.iter() {
                                        work_list.push_back(*successor);
                                    }
                                }
                            } else {
                                let body_analyzer_scc = body_analyzer.scc();
                                let old_state = scc_state.scc_alias_by_index(scc_index);
                                let successors = body_analyzer_scc.successors_of_scc(scc_index);

                                // Alias analysis for the current SCC
                                body_analyzer.alias_analysis(
                                    scc_index,
                                    local_extend_num,
                                    body_id,
                                    current_depth,
                                );

                                // Get alias from local_nodes
                                let new_state = body_analyzer.get_localnode_alias(local_extend_num);
                                // Compare two states -> Update alias into scc_state, push the successors
                                if !compare_scc_state(local_extend_num, old_state, &new_state) {
                                    scc_state.update_scc_alias_by_index(scc_index, &new_state);
                                    for successor in successors.iter() {
                                        work_list.push_back(*successor);
                                    }
                                }
                            }

                            // println!("iteration: {:?}", iteration);
                            iteration += 1;
                            if iteration > MAX_ITERATION {
                                break;
                            }
                        }

                        /*
                        // Debug for alias set
                        println!("@@@@@@===FLASH===@@@@@@");
                        println!(
                            "mir::BasicBlocks: {:?}",
                            body_analyzer
                                .body
                                .get_mir_all_basicblock(body_analyzer.rcx.tcx())
                        );

                        for local_node in body_analyzer.body.local_nodes.iter() {
                            println!("@@@@@@===FLASH===@@@@@@");
                            println!("local_node index: {:?}", local_node.index);
                            println!("local_node local: {:?}", local_node.local);
                            println!("local_node ty: {:?}", local_node.ty);
                            println!("local_node alias: {:?}", local_node.alias);
                            println!("local_node fields: {:?}", local_node.fields);
                            println!("local_node field_idx: {:?}", local_node.field_idx);
                        }
                         */

                        // let function_input_state = body_analyzer.get_function_input_alias();

                        let (args_alias, return_alias) = body_analyzer.function_alias_summary();
                        body_analyzer.func_summary.update_function_summary(
                            body_id,
                            args_alias,
                            return_alias,
                        );

                        // let (ret, function_sources) = body_analyzer.analyze_path(&body_analyzer.body, body_id);
                        let (ret, function_sources, function_sources_spans) =
                            body_analyzer.analyze_path(body_analyzer.body.clone(), body_id);
                        // Update the function sources
                        if body_analyzer
                            .func_summary
                            .is_body_function_sources_analyzed(body_id)
                            == false
                        {
                            body_analyzer
                                .func_summary
                                .update_function_sources(body_id, function_sources);
                        }

                        let mut bug_count: u32 = 0;
                        if let Some(result) = ret {
                            for (sink, sink_behaviors, source, source_behaviors) in result {
                                let mut color_span = unwrap_or!(
                                    utils::ColorSpan::new(rcx.tcx(), related_item_span).context(InvalidSpan) => continue
                                );
                                let mut sink_source_str = String::new();

                                sink_source_str.push_str("The sink: ");
                                for &span in body_analyzer
                                    .status
                                    .sink_operation_spans()
                                    .get(&sink)
                                    .unwrap()
                                {
                                    bug_count += 1;
                                    color_span.add_sub_span(Color::Red, span);
                                    sink_source_str.push_str(&format!(
                                        "behavior is {:?}, location is {:?}.",
                                        sink_behaviors, span
                                    ));
                                }

                                // for &span in body_analyzer
                                //     .status
                                //     .source_operation_spans()
                                //     .get(&source)
                                //     .unwrap()
                                sink_source_str.push_str("@@@@@@. The source: ");
                                for (span, _) in function_sources_spans.get(&source).unwrap() {
                                    color_span.add_sub_span(Color::Blue, *span);
                                    sink_source_str.push_str(&format!(
                                        "behavior is {:?}, location is {:?}.",
                                        source_behaviors, span
                                    ));
                                }

                                // ToDO: Output different spans
                                rudra_report(Report::with_color_span(
                                    rcx.tcx(),
                                    ReportLevel::Warning, // To change. Bug severity filtering for different ReportLevel
                                    AnalysisKind::UnsafeTypeDataflow(
                                        sink_behaviors,
                                        source_behaviors,
                                    ), // To change. Label it
                                    format!(
                                        "Potential unsafe type dataflow issue in `{}`",
                                        rcx.tcx().def_path_str(
                                            hir_map.body_owner_def_id(body_id).to_def_id()
                                        )
                                    ),
                                    sink_source_str,
                                    &color_span,
                                ))
                            }
                        }

                        bug_count
                        /*
                        // Path traversal without fixed-point iterations
                        for (path_idx, path) in scc_paths.iter().enumerate() {
                            println!("@@@@@@===FLASH===@@@@@@");
                            println!("START the alias set construction for scc_path.");
                            let mut bb_path = Vec::<usize>::new();
                            for &scc_index in path.iter() {
                                body_analyzer.alias_analysis(scc_index, local_extend_num);

                                // For bug checkers
                                let (root, subs) = body_analyzer.scc().nodes_in_scc(scc_index);
                                bb_path.push(root);
                                bb_path.extend(subs);
                            }
                            // Every path update one function summary. ToDO: Add function summary flag to avoid duplicate analysis and summarize all paths.
                            let (args_alias, return_alias) = body_analyzer.function_alias_summary();
                            body_analyzer.func_summary =
                                FunctionSummary::update(args_alias, return_alias);

                            println!("END the alias set construction for scc_path.");
                            println!("@@@@@@===FLASH===@@@@@@");

                            println!("@@@@@@===FLASH===@@@@@@");
                            println!("START the taint, source-sink analysis.");

                            /*
                            println!("Analyze the bb_path: {:?}", bb_path); // Would analyze bbs with repeated sequence. ToDO: to improve.
                            for bb_index in bb_path.iter() {
                                println!(
                                    "bb_index: {:?}, mir::BasicBlockData: {:?}",
                                    bb_index,
                                    body_analyzer.body.get_mir_basicblock_by_index(
                                        body_analyzer.rcx.tcx(),
                                        *bb_index
                                    )
                                );
                            }
                             */

                            let ret = body_analyzer.analyze_path(bb_path, &body.clone(), body_id);

                            println!("END the taint, source-sink analysis.");
                            println!("@@@@@@===FLASH===@@@@@@");

                            println!("@@@@@@===FLASH===@@@@@@");
                            println!("START the bug report.");
                            if let Some(result) = ret {
                                for (sink, sink_behavior, source, source_behavior) in result {
                                    let mut color_span = unwrap_or!(
                                        utils::ColorSpan::new(rcx.tcx(), related_item_span).context(InvalidSpan) => continue
                                    );

                                    println!("@@@@@@===FLASH===@@@@@@");
                                    println!("@@===Analyze the scc_path idx: {:?}===@@", path_idx);

                                    for &span in body_analyzer
                                        .status
                                        .sink_operation_spans()
                                        .get(&sink)
                                        .unwrap()
                                    {
                                        color_span.add_sub_span(Color::Red, span);
                                        println!("The sink localnode {:?} performs behaviors: {:?}. Its location is {:?}.", sink, sink_behavior, span);
                                    }

                                    for &span in body_analyzer
                                        .status
                                        .source_operation_spans()
                                        .get(&source)
                                        .unwrap()
                                    {
                                        color_span.add_sub_span(Color::Blue, span);
                                        println!("The source localnode {:?} performs behaviors: {:?}. Its location is {:?}.", source, source_behavior, span);
                                    }

                                    // ToDO: Output different spans
                                    rudra_report(Report::with_color_span(
                                        rcx.tcx(),
                                        ReportLevel::Warning, // To change. Bug severity filtering for different ReportLevel
                                        AnalysisKind::UnsafeDataflow(BehaviorFlag::TRANSMUTE), // To change. Label it
                                        format!(
                                            "Potential unsafe type dataflow issue in `{}`",
                                            rcx.tcx().def_path_str(
                                                hir_map.body_owner_def_id(body_id).to_def_id()
                                            )
                                        ),
                                        &color_span,
                                    ))
                                }
                            }
                            println!("END the bug report.");
                            println!("@@@@@@===FLASH===@@@@@@");
                        }
                         */

                        // Combine each path TypeDataflowStatus into a unified one?
                        // Some(body_analyzer.analyze())
                        // Some(body_analyzer.status)
                    }
                }
            }
            // else {
            //     // FLASH: ToDO: Classify and add the functions into different categories for analysis
            //     // Consider interprocedural analysis
            //     // Consider safe functions that call unsafe functions or safe functions that contain unsafe code blocks.
            //     //      -> safe functions that call safe functions that include raw pointers; others are ignored
            //     // Consider unsafe annotated functions
            //     // Similar procedure as above. Only need to filter the required safe functions out.
            //     println!("ToDO: Safe function body_did: {:?}", body_did);
            //     // Some(Default::default())
            //     0
            // }
        }

        // Field-sensitive for a local variable.
        // is_right: 2 = 1.0; 0 = 2.0; => 0 = 1.0.0;
        pub fn handle_projection_index(&mut self, local: usize, place: Place<'tcx>) -> usize {
            let mut root_index = local; // The root index in field struct
            let mut current_index = local; // The current index in field struct

            for projetion in place.projection {
                match projetion {
                    // _1.f, _1.1.f between the base and the 'projection element'.
                    ProjectionElem::Field(field, ty) => {
                        let field_index = field.index();

                        // Construct field LocalNode
                        if self.body.local_nodes[current_index]
                            .fields
                            .contains_key(&field_index)
                            == false
                        {
                            let kind = type_kind_classify(ty);
                            let mut local_node =
                                LocalNode::new(self.body.local_nodes.len(), root_index, ty, kind);
                            local_node.field_idx =
                                self.body.local_nodes[current_index].field_idx.clone();
                            local_node.field_idx.push(field_index); // Achieve the 1.0.0 case
                            self.body.local_nodes[current_index]
                                .fields
                                .insert(field_index, local_node.index);
                            self.body.local_nodes.push(local_node);
                        }

                        // Update current_index into field LocalNode
                        current_index = self.body.local_nodes[current_index].fields[&field_index];
                    }
                    _ => (),
                }
            }

            // Return the added field LocalNode index, then build alias set for it
            return current_index;
        }

        pub fn handle_projection_index_immut(&self, local: usize, place: Place<'tcx>) -> usize {
            let mut root_index = local; // The root index in field struct
            let mut current_index = local; // The current index in field struct

            for projetion in place.projection {
                match projetion {
                    // _1.f, _1.1.f between the base and the 'projection element'.
                    ProjectionElem::Field(field, ty) => {
                        let field_index = field.index();

                        // Update current_index into field LocalNode recursively
                        if self.body.local_nodes[current_index]
                            .fields
                            .contains_key(&field_index)
                        {
                            current_index =
                                self.body.local_nodes[current_index].fields[&field_index];
                        }
                    }
                    _ => (),
                }
            }

            // Return the added field LocalNode index, then build alias set for it
            return current_index;
        }

        pub fn merge_alias(&mut self, llocal_index: usize, rlocal_index: usize) {
            if llocal_index == rlocal_index
                || self.body.local_nodes[llocal_index].alias.clone()
                    == self.body.local_nodes[rlocal_index].alias.clone()
            {
                return;
            }

            let mut lalias_new: Vec<usize> = Vec::new();
            lalias_new.extend(
                self.body.local_nodes[llocal_index]
                    .alias
                    .clone()
                    .iter()
                    .filter(|&x| *x != llocal_index),
            );
            let mut ralias_new = {
                let mut ralias = self.body.local_nodes[rlocal_index].alias.clone();
                if !ralias.contains(&llocal_index) {
                    ralias.push(llocal_index);
                }
                ralias
            };

            // Update alias set and propagate, field...
            // ToDO: Based on SSA, there could be a simplification. Potentially would be path explosion -> WRONG! Not SSA in MIR!
            for &lalias in lalias_new.iter() {
                self.body.local_nodes[lalias].alias = lalias_new.clone();
            }
            for &ralias in ralias_new.iter() {
                self.body.local_nodes[ralias].alias = ralias_new.clone();
            }

            // Case handle (_0.0 = move _2), add LocalNode _0.0.0
            // (_2.0, _3, _1) -> (_0.0, _2), (_0.0.0, _2.0, _3, _1)
            for field in self.body.local_nodes[rlocal_index]
                .fields
                .clone()
                .into_iter()
            {
                if self.body.local_nodes[llocal_index]
                    .fields
                    .contains_key(&field.0)
                    == false
                {
                    let new_ty = self.body.local_nodes[field.1].ty;
                    let kind = type_kind_classify(new_ty);
                    let mut local_node =
                        LocalNode::new(self.body.local_nodes.len(), llocal_index, new_ty, kind);
                    local_node.field_idx = self.body.local_nodes[llocal_index].field_idx.clone();
                    local_node.field_idx.push(field.0);
                    self.body.local_nodes[llocal_index]
                        .fields
                        .insert(field.0, local_node.index);
                    self.body.local_nodes.push(local_node);
                }
                let llocal_field_index = *self.body.local_nodes[llocal_index]
                    .fields
                    .get(&field.0)
                    .unwrap();
                self.merge_alias(llocal_field_index, field.1);
            }
        }

        pub fn alias_check(&mut self, bb_index: usize) {
            let basic_block = self.body.basic_blocks[bb_index].clone();
            for statement in basic_block.statements {
                match statement.kind {
                    ir::StatementKind::Assign {
                        lplace,
                        rplace,
                        kind,
                        ltype,
                        rtype,
                        castkind,
                    } => {
                        // Handle projection LocalNode index
                        let llocal_index = if self.has_projection(lplace) {
                            self.handle_projection_index(lplace.local.as_usize(), lplace)
                        } else {
                            lplace.local.as_usize()
                        };

                        let rlocal_index = if self.has_projection(rplace) {
                            self.handle_projection_index(rplace.local.as_usize(), rplace)
                        } else {
                            rplace.local.as_usize()
                        };

                        // Merge alias set
                        self.merge_alias(llocal_index, rlocal_index);
                    }
                    _ => (),
                }
            }
        }

        // Inter-procedural alias set construction procedure:
        //            only need to analyze the args and return locals
        //            function-level alias set summary
        //                  -> summarize the alias relationship between args and return locals in callee function
        //                  -> update caller llocal and args alias by using the callee return alias set
        //                  -> the ids of LocalNode in different functions are different
        //                  -> NEW! ToDO: if the inter-function is analyzed, the function summary is updated. -> stop analyze
        pub fn inter_alias_check(
            &mut self,
            bb_index: usize,
            caller_body_id: BodyId,
            inter_depth: u32,
        ) {
            let basic_block = self.body.basic_blocks[bb_index].clone();
            let terminator = basic_block.terminator;
            match terminator.kind {
                ir::TerminatorKind::StaticCall {
                    callee_did,
                    callee_substs,
                    func,
                    args,
                    destination,
                } => {
                    if let mir::Operand::Constant(box constoperand) = func {
                        // ToDO: consider add another boundary check on inter-function calling itself
                        if inter_depth > MAX_INTER_DEPTH {
                            return;
                        }

                        let (lplace, target) = destination.unwrap();
                        let llocal_index = if self.has_projection(lplace) {
                            self.handle_projection_index(lplace.local.as_usize(), lplace)
                        } else {
                            lplace.local.as_usize()
                        };

                        // Map the local indexes of the return and args between caller and callee
                        let mut local_map: FxHashMap<usize, usize> = FxHashMap::default();
                        local_map.insert(0, llocal_index);
                        let mut r_index: usize = 1;
                        for arg in args {
                            match arg {
                                Operand::Copy(rplace) => {
                                    let rlocal_index = if self.has_projection(rplace) {
                                        self.handle_projection_index(
                                            rplace.local.as_usize(),
                                            rplace,
                                        )
                                    } else {
                                        rplace.local.as_usize()
                                    };
                                    local_map.insert(r_index, rlocal_index);
                                    r_index += 1;
                                }
                                Operand::Move(rplace) => {
                                    let rlocal_index = if self.has_projection(rplace) {
                                        self.handle_projection_index(
                                            rplace.local.as_usize(),
                                            rplace,
                                        )
                                    } else {
                                        rplace.local.as_usize()
                                    };
                                    local_map.insert(r_index, rlocal_index);
                                    r_index += 1;
                                }
                                Operand::Constant(_) => {
                                    r_index += 1;
                                }
                            }
                        }

                        // Repeat fn analyze_body steps for the callee
                        let hir_map = self.rcx.tcx().hir();
                        // DefId -> LocalDefId -> BodyId
                        let local_defid = if let Some(local_def_id) = callee_did.as_local() {
                            local_def_id
                        } else {
                            return;
                        };
                        match hir_map.maybe_body_owned_by(local_defid) {
                            Some(body_id) => {
                                let body_id = body_id;
                                // let body_id = hir_map.body_owned_by(local_defid);
                                // let body_id = hir_map.body_owned_by(callee_did.expect_local());

                                if self.rcx.tcx().ext().match_def_path(
                                    callee_did,
                                    &["rudra_paths_discovery", "PathsDiscovery", "discover"],
                                ) {
                                    // Special case for paths discovery
                                    // trace_calls_in_body(self.rcx, callee_did);
                                } else {
                                    // else if ContainsUnsafe::contains_unsafe(self.rcx.tcx(), body_id) {
                                    self.func_summary.update_inter_functions(
                                        caller_body_id.clone(),
                                        body_id.clone(),
                                        local_map.clone(),
                                    );

                                    // let function_input_state = self.get_function_input_alias();

                                    let mut args_alias: Vec<Vec<usize>>;
                                    let mut return_alias: Vec<Vec<usize>>;
                                    // if self.func_summary.is_body_function_summary_analyzed(
                                    //     body_id,
                                    //     &function_input_state,
                                    // ) {
                                    if self.func_summary.is_body_analyzed(body_id) {
                                        // ToDO: Not support field-sensitive alias analysis like the else branch
                                        // (args_alias, return_alias) = self
                                        //     .func_summary
                                        //     .get_function_summary(body_id, &function_input_state);
                                        (args_alias, return_alias) =
                                            self.func_summary.get_function_summary(body_id);

                                        // Merge the args_alias
                                        for alias_set in args_alias.iter() {
                                            if alias_set.contains(&0) {
                                                // Skip the return alias
                                                continue;
                                            }
                                            let mut caller_alias = Vec::<usize>::new();
                                            let mut args_idx = Vec::<usize>::new();

                                            // handle (_1, _2), (_1, _2.0) case
                                            for &callee_idx in alias_set.iter() {
                                                if local_map.contains_key(&callee_idx) {
                                                    let caller_idx =
                                                        local_map.get(&callee_idx).unwrap();
                                                    caller_alias.extend(
                                                        self.body.local_nodes[*caller_idx]
                                                            .alias
                                                            .clone(),
                                                    );
                                                    args_idx.push(*caller_idx);
                                                }
                                            }

                                            // Remove duplicates
                                            let mut caller_alias_new = Vec::<usize>::new();
                                            let mut order_set: FxHashSet<usize> =
                                                FxHashSet::default();
                                            for &idx in caller_alias.iter() {
                                                if order_set.insert(idx) {
                                                    caller_alias_new.push(idx);
                                                }
                                            }
                                            // caller_alias = caller_alias
                                            //     .clone()
                                            //     .into_iter()
                                            //     .collect::<FxHashSet<usize>>()
                                            //     .into_iter()
                                            //     .collect();

                                            // args_idx may be duplicates, but still ok
                                            for &caller_idx in caller_alias_new.iter() {
                                                self.body.local_nodes[caller_idx].alias =
                                                    caller_alias_new.clone();
                                            }
                                        }

                                        // Merge the return_alias
                                        for alias_set in return_alias.iter() {
                                            let mut caller_alias = Vec::<usize>::new();
                                            let mut return_idx = Vec::<usize>::new();

                                            // Update the llocal or as well as its fields?
                                            let mut llocal_idx_list = Vec::<usize>::new();

                                            // handle _0.0, _0.0.0 -> (_0.0, _1), (_0.0, _1.0) case
                                            for &callee_idx in alias_set.iter() {
                                                if local_map.contains_key(&callee_idx) {
                                                    let caller_idx =
                                                        local_map.get(&callee_idx).unwrap();
                                                    caller_alias.extend(
                                                        self.body.local_nodes[*caller_idx]
                                                            .alias
                                                            .clone(),
                                                    );
                                                    return_idx.push(*caller_idx);
                                                }
                                            }

                                            let mut lalias_new: Vec<usize> = Vec::new();
                                            lalias_new.extend(
                                                self.body.local_nodes[llocal_index]
                                                    .alias
                                                    .clone()
                                                    .iter()
                                                    .filter(|&x| *x != llocal_index),
                                            );
                                            for &lalias in lalias_new.iter() {
                                                self.body.local_nodes[lalias].alias =
                                                    lalias_new.clone();
                                            }

                                            // Remove duplicates
                                            let mut caller_alias_new = Vec::<usize>::new();
                                            let mut order_set: FxHashSet<usize> =
                                                FxHashSet::default();
                                            for &idx in caller_alias.iter() {
                                                if order_set.insert(idx) {
                                                    caller_alias_new.push(idx);
                                                }
                                            }
                                            // caller_alias = caller_alias
                                            //     .clone()
                                            //     .into_iter()
                                            //     .collect::<FxHashSet<usize>>()
                                            //     .into_iter()
                                            //     .collect();
                                            for &caller_idx in caller_alias_new.iter() {
                                                self.body.local_nodes[caller_idx].alias =
                                                    caller_alias_new.clone();
                                            }
                                        }
                                    } else {
                                        match self.rcx.translate_body(callee_did).as_ref() {
                                            Err(e) => {
                                                // MIR is not available for def - log it and continue
                                                e.log();
                                            }
                                            Ok(body) => {
                                                let param_env =
                                                    self.rcx.tcx().param_env(callee_did);
                                                let mut body_clone = body.clone();
                                                let body_mut = &mut body_clone;

                                                let scc = body.solve_scc();
                                                let scc_num = scc.group_len();
                                                let local_num = body.local_nodes.len();
                                                let local_extend_num = (local_num as f64
                                                    * EXTEND_COEFFICIENT)
                                                    as usize;

                                                let mut body_analyzer =
                                                    UnsafeDataflowBodyAnalyzer::new(
                                                        self.rcx,
                                                        param_env,
                                                        body_mut,
                                                        scc,
                                                        self.func_summary,
                                                    );
                                                let mut scc_state =
                                                    SccState::new(scc_num, local_extend_num);

                                                let mut current_depth = inter_depth + 1;
                                                let paths = body_analyzer.scc().paths();
                                                let scc_paths =
                                                    body_analyzer.scc().scc_paths().clone();

                                                let mut work_list: VecDeque<usize> =
                                                    VecDeque::new(); // SCC index
                                                for idx in 0..scc_num {
                                                    work_list.push_back(idx);
                                                }

                                                let mut iteration = 0;
                                                while let Some(scc_index) = work_list.pop_front() {
                                                    if iteration > 0 {
                                                        let body_analyzer_scc = body_analyzer.scc();
                                                        let old_state =
                                                            scc_state.scc_alias_by_index(scc_index);
                                                        let predecessors = body_analyzer_scc
                                                            .predecessors_of_scc(scc_index);
                                                        let successors = body_analyzer_scc
                                                            .successors_of_scc(scc_index);

                                                        // Get and merge the predecessors -> Update the predecessors' state
                                                        let mut merge_predecessors: Vec<
                                                            Vec<usize>,
                                                        > = Vec::with_capacity(local_extend_num);
                                                        for (idx, predecessor) in
                                                            predecessors.iter().enumerate()
                                                        {
                                                            if idx == 0 {
                                                                merge_predecessors = scc_state
                                                                    .scc_alias_by_index(
                                                                        *predecessor,
                                                                    )
                                                                    .to_vec();
                                                            } else {
                                                                merge_predecessors =
                                                                    merge_scc_state(
                                                                        local_extend_num,
                                                                        &merge_predecessors,
                                                                        scc_state
                                                                            .scc_alias_by_index(
                                                                                *predecessor,
                                                                            ),
                                                                    );
                                                            }
                                                        }
                                                        if predecessors.len() > 0 {
                                                            body_analyzer.update_localnode_alias(
                                                                &merge_predecessors,
                                                            );
                                                        }
                                                        // KEY: Alias analysis for the current SCC
                                                        body_analyzer.alias_analysis(
                                                            scc_index,
                                                            local_extend_num,
                                                            body_id,
                                                            current_depth,
                                                        );

                                                        // Get alias from local_nodes
                                                        let new_state = body_analyzer
                                                            .get_localnode_alias(local_extend_num);
                                                        // Compare two states -> Update alias into scc_state, push the successors
                                                        if !compare_scc_state(
                                                            local_extend_num,
                                                            old_state,
                                                            &new_state,
                                                        ) {
                                                            scc_state.update_scc_alias_by_index(
                                                                scc_index, &new_state,
                                                            );
                                                            for successor in successors.iter() {
                                                                work_list.push_back(*successor);
                                                            }
                                                        }
                                                    } else {
                                                        let body_analyzer_scc = body_analyzer.scc();
                                                        let old_state =
                                                            scc_state.scc_alias_by_index(scc_index);
                                                        let successors = body_analyzer_scc
                                                            .successors_of_scc(scc_index);

                                                        // Alias analysis for the current SCC
                                                        body_analyzer.alias_analysis(
                                                            scc_index,
                                                            local_extend_num,
                                                            body_id,
                                                            current_depth,
                                                        );

                                                        // Get alias from local_nodes
                                                        let new_state = body_analyzer
                                                            .get_localnode_alias(local_extend_num);
                                                        // Compare two states -> Update alias into scc_state, push the successors
                                                        if !compare_scc_state(
                                                            local_extend_num,
                                                            old_state,
                                                            &new_state,
                                                        ) {
                                                            scc_state.update_scc_alias_by_index(
                                                                scc_index, &new_state,
                                                            );
                                                            for successor in successors.iter() {
                                                                work_list.push_back(*successor);
                                                            }
                                                        }
                                                    }

                                                    iteration += 1;
                                                    if iteration > MAX_ITERATION {
                                                        break;
                                                    }
                                                }

                                                let (func_args_alias, func_return_alias) =
                                                    body_analyzer.function_alias_summary();
                                                body_analyzer.func_summary.update_function_summary(
                                                    body_id,
                                                    // &function_input_state,
                                                    func_args_alias,
                                                    func_return_alias,
                                                );

                                                // Update the function sources
                                                if body_analyzer
                                                    .func_summary
                                                    .is_body_function_sources_analyzed(body_id)
                                                    == false
                                                {
                                                    // let (_, function_sources) = body_analyzer.analyze_path(&body_analyzer.body.clone(), body_id);
                                                    let (_, function_sources, _) = body_analyzer
                                                        .analyze_path(
                                                            body_analyzer.body.clone(),
                                                            body_id,
                                                        );
                                                    body_analyzer
                                                        .func_summary
                                                        .update_function_sources(
                                                            body_id,
                                                            function_sources,
                                                        );
                                                }

                                                /*
                                                for path in scc_paths.iter() {
                                                    for &scc_index in path.iter() {
                                                        body_analyzer
                                                            .alias_analysis(scc_index, local_num);
                                                    }
                                                    // Every path update one function summary
                                                    let (args_alias, return_alias) =
                                                        body_analyzer.function_alias_summary();
                                                    body_analyzer.func_summary =
                                                        FunctionSummary::update(
                                                            args_alias,
                                                            return_alias,
                                                        );
                                                    // FLASH: ToDO:
                                                    //      Fix function summary update -> currently only reserve the final path function summary
                                                    //      !! Add function summary flag to avoid duplicate analysis. We want to directly retrieve the function summary. -> Just for loop once?
                                                    //      Bug checkers for analyze_path -> Which paths?
                                                }
                                                 */

                                                // Merge the alias set of the return locals and args in caller function
                                                // (args_alias, return_alias) = body_analyzer
                                                //     .func_summary
                                                //     .get_function_summary(
                                                //         body_id,
                                                //         &function_input_state,
                                                //     );
                                                (args_alias, return_alias) = body_analyzer
                                                    .func_summary
                                                    .get_function_summary(body_id);

                                                // Merge the args_alias
                                                for alias_set in args_alias.iter() {
                                                    if alias_set.contains(&0) {
                                                        // Skip the return alias
                                                        continue;
                                                    }
                                                    let mut caller_alias = Vec::<usize>::new();
                                                    let mut args_idx = Vec::<usize>::new();

                                                    // handle (_1, _2), (_1, _2.0) case
                                                    for &callee_idx in alias_set.iter() {
                                                        if local_map.contains_key(&callee_idx) {
                                                            let caller_idx =
                                                                local_map.get(&callee_idx).unwrap();
                                                            caller_alias.extend(
                                                                self.body.local_nodes[*caller_idx]
                                                                    .alias
                                                                    .clone(),
                                                            );
                                                            args_idx.push(*caller_idx);
                                                        } else {
                                                            // Field-sensitive analysis.
                                                            let callee_root_idx = body_analyzer
                                                                .body
                                                                .local_nodes[callee_idx]
                                                                .local;
                                                            // Filter const args case
                                                            if local_map
                                                                .contains_key(&callee_root_idx)
                                                            {
                                                                let caller_idx = local_map
                                                                    .get(&callee_root_idx)
                                                                    .unwrap();
                                                                if self.body.local_nodes
                                                                    [*caller_idx]
                                                                    .fields
                                                                    .keys()
                                                                    .eq(body_analyzer
                                                                        .body
                                                                        .local_nodes
                                                                        [callee_root_idx]
                                                                        .fields
                                                                        .keys())
                                                                {
                                                                    let callee_field_idx =
                                                                        body_analyzer
                                                                            .body
                                                                            .local_nodes
                                                                            [callee_root_idx]
                                                                            .fields
                                                                            .iter()
                                                                            .find_map(|(&k, &v)| {
                                                                                if v == callee_idx {
                                                                                    Some(k)
                                                                                } else {
                                                                                    None
                                                                                }
                                                                            });
                                                                    // .unwrap();
                                                                    for (
                                                                        caller_field_idx,
                                                                        caller_field_node,
                                                                    ) in self.body.local_nodes
                                                                        [*caller_idx]
                                                                        .fields
                                                                        .iter()
                                                                    {
                                                                        if *caller_field_idx
                                                                            == callee_field_idx
                                                                                .unwrap()
                                                                        {
                                                                            caller_alias.extend(
                                                                    self.body.local_nodes
                                                                        [*caller_field_node]
                                                                        .alias
                                                                        .clone(),
                                                                        );
                                                                            args_idx.push(
                                                                                *caller_field_node,
                                                                            );
                                                                        }
                                                                    }
                                                                } else {
                                                                    // Create new alias node in caller function
                                                                    let new_ty = body_analyzer
                                                                        .body
                                                                        .local_nodes[callee_idx]
                                                                        .ty;
                                                                    let kind =
                                                                        type_kind_classify(new_ty);
                                                                    let mut local_node =
                                                                        LocalNode::new(
                                                                            self.body
                                                                                .local_nodes
                                                                                .len(),
                                                                            *caller_idx,
                                                                            new_ty,
                                                                            kind,
                                                                        );
                                                                    let field_key = body_analyzer
                                                                        .body
                                                                        .local_nodes
                                                                        [callee_root_idx]
                                                                        .fields
                                                                        .iter()
                                                                        .find_map(|(&k, &v)| {
                                                                            if v == callee_idx {
                                                                                Some(k)
                                                                            } else {
                                                                                None
                                                                            }
                                                                        });
                                                                    // .unwrap();
                                                                    if let None = field_key {
                                                                        continue;
                                                                    }
                                                                    local_node.field_idx =
                                                                        body_analyzer
                                                                            .body
                                                                            .local_nodes
                                                                            [callee_idx]
                                                                            .field_idx
                                                                            .clone();
                                                                    self.body.local_nodes
                                                                        [*caller_idx]
                                                                        .fields
                                                                        .insert(
                                                                            field_key.unwrap(),
                                                                            local_node.index,
                                                                        );
                                                                    args_idx.push(local_node.index);
                                                                    self.body
                                                                        .local_nodes
                                                                        .push(local_node);
                                                                }
                                                            }
                                                        }
                                                    }

                                                    // Remove duplicates
                                                    let mut caller_alias_new = Vec::<usize>::new();
                                                    let mut order_set: FxHashSet<usize> =
                                                        FxHashSet::default();
                                                    for &idx in caller_alias.iter() {
                                                        if order_set.insert(idx) {
                                                            caller_alias_new.push(idx);
                                                        }
                                                    }
                                                    // caller_alias = caller_alias
                                                    //     .clone()
                                                    //     .into_iter()
                                                    //     .collect::<FxHashSet<usize>>()
                                                    //     .into_iter()
                                                    //     .collect();

                                                    // args_idx may be duplicates, but still ok
                                                    for &caller_idx in caller_alias_new.iter() {
                                                        self.body.local_nodes[caller_idx].alias =
                                                            caller_alias_new.clone();
                                                    }
                                                }

                                                // Merge the return_alias
                                                for alias_set in return_alias.iter() {
                                                    let mut caller_alias = Vec::<usize>::new();
                                                    let mut return_idx = Vec::<usize>::new();

                                                    // Update the llocal or as well as its fields?
                                                    let mut llocal_idx_list = Vec::<usize>::new();

                                                    // handle _0.0, _0.0.0 -> (_0.0, _1), (_0.0, _1.0) case
                                                    for &callee_idx in alias_set.iter() {
                                                        if local_map.contains_key(&callee_idx) {
                                                            let caller_idx =
                                                                local_map.get(&callee_idx).unwrap();
                                                            caller_alias.extend(
                                                                self.body.local_nodes[*caller_idx]
                                                                    .alias
                                                                    .clone(),
                                                            );
                                                            return_idx.push(*caller_idx);
                                                        } else {
                                                            // Field-sensitive analysis.
                                                            let callee_root_idx = body_analyzer
                                                                .body
                                                                .local_nodes[callee_idx]
                                                                .local;
                                                            // Filter const args case
                                                            if local_map
                                                                .contains_key(&callee_root_idx)
                                                            {
                                                                let caller_idx = local_map
                                                                    .get(&callee_root_idx)
                                                                    .unwrap();
                                                                if self.body.local_nodes
                                                                    [*caller_idx]
                                                                    .fields
                                                                    .keys()
                                                                    .eq(body_analyzer
                                                                        .body
                                                                        .local_nodes
                                                                        [callee_root_idx]
                                                                        .fields
                                                                        .keys())
                                                                {
                                                                    let callee_field_idx =
                                                                        body_analyzer
                                                                            .body
                                                                            .local_nodes
                                                                            [callee_root_idx]
                                                                            .fields
                                                                            .iter()
                                                                            .find_map(|(&k, &v)| {
                                                                                if v == callee_idx {
                                                                                    Some(k)
                                                                                } else {
                                                                                    None
                                                                                }
                                                                            });
                                                                    // .unwrap();
                                                                    if let None = callee_field_idx {
                                                                        continue;
                                                                    }
                                                                    for (
                                                                        caller_field_idx,
                                                                        caller_field_node,
                                                                    ) in self.body.local_nodes
                                                                        [*caller_idx]
                                                                        .fields
                                                                        .iter()
                                                                    {
                                                                        if *caller_field_idx
                                                                            == callee_field_idx
                                                                                .unwrap()
                                                                        {
                                                                            caller_alias.extend(
                                                                    self.body.local_nodes
                                                                        [*caller_field_node]
                                                                        .alias
                                                                        .clone(),
                                                                        );
                                                                            return_idx.push(
                                                                                *caller_field_node,
                                                                            );
                                                                        }
                                                                    }
                                                                } else {
                                                                    // Create new alias node
                                                                    let new_ty = body_analyzer
                                                                        .body
                                                                        .local_nodes[callee_idx]
                                                                        .ty;
                                                                    let kind =
                                                                        type_kind_classify(new_ty);
                                                                    let mut local_node =
                                                                        LocalNode::new(
                                                                            self.body
                                                                                .local_nodes
                                                                                .len(),
                                                                            *caller_idx,
                                                                            new_ty,
                                                                            kind,
                                                                        );
                                                                    let field_key = body_analyzer
                                                                        .body
                                                                        .local_nodes
                                                                        [callee_root_idx]
                                                                        .fields
                                                                        .iter()
                                                                        .find_map(|(&k, &v)| {
                                                                            if v == callee_idx {
                                                                                Some(k)
                                                                            } else {
                                                                                None
                                                                            }
                                                                        });
                                                                    // .unwrap();
                                                                    if let None = field_key {
                                                                        continue;
                                                                    }
                                                                    local_node.field_idx =
                                                                        body_analyzer
                                                                            .body
                                                                            .local_nodes
                                                                            [callee_idx]
                                                                            .field_idx
                                                                            .clone();
                                                                    self.body.local_nodes
                                                                        [*caller_idx]
                                                                        .fields
                                                                        .insert(
                                                                            field_key.unwrap(),
                                                                            local_node.index,
                                                                        );
                                                                    return_idx
                                                                        .push(local_node.index);
                                                                    self.body
                                                                        .local_nodes
                                                                        .push(local_node);
                                                                }
                                                            }
                                                        }
                                                    }

                                                    let mut lalias_new: Vec<usize> = Vec::new();
                                                    lalias_new.extend(
                                                        self.body.local_nodes[llocal_index]
                                                            .alias
                                                            .clone()
                                                            .iter()
                                                            .filter(|&x| *x != llocal_index),
                                                    );
                                                    for &lalias in lalias_new.iter() {
                                                        self.body.local_nodes[lalias].alias =
                                                            lalias_new.clone();
                                                    }

                                                    // Remove duplicates
                                                    let mut caller_alias_new = Vec::<usize>::new();
                                                    let mut order_set: FxHashSet<usize> =
                                                        FxHashSet::default();
                                                    for &idx in caller_alias.iter() {
                                                        if order_set.insert(idx) {
                                                            caller_alias_new.push(idx);
                                                        }
                                                    }
                                                    // caller_alias = caller_alias
                                                    //     .clone()
                                                    //     .into_iter()
                                                    //     .collect::<FxHashSet<usize>>()
                                                    //     .into_iter()
                                                    //     .collect();
                                                    for &caller_idx in caller_alias_new.iter() {
                                                        self.body.local_nodes[caller_idx].alias =
                                                            caller_alias_new.clone();
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                // else {
                                //     // FLASH: ToDO: other functions
                                // }
                            }
                            None => {
                                return;
                            }
                        }
                    }
                }
                _ => (),
            }
        }

        // Summarize the alias set for args, and return. Field-sensitive
        pub fn function_alias_summary(&mut self) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
            let mut args_alias = Vec::<Vec<usize>>::new(); // alias relationship between args
            let mut return_alias = Vec::<Vec<usize>>::new(); // alias relationship between return and args

            let mut flag: FxHashMap<usize, bool> = FxHashMap::default();

            for local_node in self.body.local_nodes.iter() {
                if local_node.index <= self.body.arg_size {
                    if local_node.index == 0 {
                        // return _0
                        // handle (_0, _1), (_0, _1.0) case
                        let mut alias_set = Vec::<usize>::new();
                        for &alias in local_node.alias.iter() {
                            if self.body.local_nodes[alias].local <= self.body.arg_size {
                                alias_set.push(alias);
                            }
                        }
                        // handle _0.0, _0.0.0 -> (_0.0, _1), (_0.0, _1.0) case
                        if local_node.fields.len() > 0 {
                            let mut field_alias_set = Vec::<usize>::new();
                            for (fieldidx, &nodeidx) in local_node.fields.iter() {
                                if self.body.local_nodes[nodeidx].local == 0
                                    && !alias_set.contains(&self.body.local_nodes[nodeidx].index)
                                {
                                    for &alias in self.body.local_nodes[nodeidx].alias.iter() {
                                        if self.body.local_nodes[alias].local <= self.body.arg_size
                                        {
                                            field_alias_set.push(alias);
                                        }
                                    }
                                }
                            }
                            return_alias.push(field_alias_set);
                        }
                        return_alias.push(alias_set);
                    } else {
                        // not return _0
                        if flag.contains_key(&local_node.index) {
                            continue;
                        } else {
                            // handle (_1, _2), (_1, _2.0) case
                            let mut alias_set = Vec::<usize>::new();
                            for &alias in local_node.alias.iter() {
                                if self.body.local_nodes[alias].local <= self.body.arg_size {
                                    alias_set.push(alias);
                                    flag.insert(alias, true);

                                    // if self.body.local_nodes[alias].fields.len() > 0 {
                                    //     for (fieldidx, nodeidx) in
                                    //         self.body.local_nodes[alias].fields.iter()
                                    //     {
                                    //     }
                                    // }
                                }
                            }
                            args_alias.push(alias_set);
                        }
                    }
                }
            }

            (args_alias, return_alias)
        }

        // pub fn alias_check(&mut self, bb_index: usize, llocalnode_set: &mut FxHashSet<usize>) {}

        // pub fn inter_alias_check(
        //     &mut self,
        //     bb_index: usize,
        //     llocalnode_set: &mut FxHashSet<usize>,
        // ) {
        // }

        // ToDO: 1) How to find the owned object? For different BugCheckers. -> Implement a simple one
        //       2) Fixed-point iteration for bbs in SCC
        // Every LocalNode should be filled with the alias set, and the order is the index inserted order.
        // Alias set construction procedure:
        //              root alias check -> root inter alias check ->
        //              sub alias check -> sub inter alias check
        //              if scc/group or bb_index next = 0 -> merge results for function summary
        //              else bb_index next -> alias_analysis
        //                  Special handle: SwitchInt issues for control flow
        // Update alias set for each LocalNode, also handle with the field information
        pub fn alias_analysis(
            &mut self,
            scc_index: usize,
            local_num: usize,
            body_id: BodyId,
            inter_depth: u32,
        ) {
            let mut work_list: VecDeque<usize> = VecDeque::new(); // bb index
            let scc_list = self.scc().nodes_in_group(scc_index).to_vec();
            let bb_num = scc_list.len();
            work_list.extend(scc_list.iter().copied());

            let mut bb_state = BbState::new(scc_list.clone(), local_num);
            // Update the current SCC state into bb state as the original state
            let original_state = self.get_localnode_alias(local_num);
            for bb_index in scc_list.iter() {
                bb_state.update_bb_alias_by_index(*bb_index, &original_state);
            }

            let mut iteration = 0;
            while let Some(bb_index) = work_list.pop_front() {
                if iteration > 0 {
                    let old_state = bb_state.bb_alias_by_index(bb_index);
                    let predecessors = self.body.predecessors_of_bb_in_scc(bb_index, &scc_list);
                    let successors = self.body.successors_of_bb_in_scc(bb_index, &scc_list);

                    let mut merge_predecessors: Vec<Vec<usize>> = Vec::with_capacity(local_num);
                    for (idx, predecessor) in predecessors.iter().enumerate() {
                        if idx == 0 {
                            merge_predecessors = bb_state.bb_alias_by_index(*predecessor).to_vec();
                        } else {
                            merge_predecessors = merge_scc_state(
                                local_num,
                                &merge_predecessors,
                                bb_state.bb_alias_by_index(*predecessor),
                            );
                        }
                    }
                    if predecessors.len() > 0 {
                        self.update_localnode_alias(&merge_predecessors);
                    }

                    self.alias_check(bb_index);
                    self.inter_alias_check(bb_index, body_id, inter_depth);

                    let new_state = self.get_localnode_alias(local_num);
                    if !compare_scc_state(local_num, old_state, &new_state) {
                        bb_state.update_bb_alias_by_index(bb_index, &new_state);
                        for successor in successors.iter() {
                            work_list.push_back(*successor);
                        }
                    }
                } else {
                    // Pass the case: work_list.len() is 1
                    let old_state = bb_state.bb_alias_by_index(bb_index);
                    let successors = self.body.successors_of_bb_in_scc(bb_index, &scc_list);

                    self.alias_check(bb_index);
                    self.inter_alias_check(bb_index, body_id, inter_depth);

                    let new_state = self.get_localnode_alias(local_num);
                    if !compare_scc_state(local_num, old_state, &new_state) {
                        bb_state.update_bb_alias_by_index(bb_index, &new_state);
                        for successor in successors.iter() {
                            work_list.push_back(*successor);
                        }
                    }
                }

                iteration += 1;
                if iteration > MAX_ITERATION {
                    break;
                }
            }

            /*
            let (root, subs) = {
                let (r, s) = self.scc().nodes_in_scc(scc_index);
                (r.clone(), s.to_vec())
            };

            // For flow-sensitive analysis? Recursion in different SCCs
            self.alias_check(root);
            self.inter_alias_check(root);

            // ToDO: add fixed-point iteration
            if subs.len() > 0 {
                for sub in subs {
                    self.alias_check(sub);
                    self.inter_alias_check(sub);
                }
            }
             */

            // ToDO: Handle SwitchInt case?
        }

        pub fn get_localnode_index(&self, local: Place<'tcx>) -> usize {
            return if self.has_projection_immut(local) {
                self.handle_projection_index_immut(local.local.as_usize(), local)
            } else {
                local.local.as_usize()
            };
        }

        // Analyze all bbs for taint / source-sink analysis, then bug checking
        // Mark sources -> l=r, l as source
        //   -> Sink chain construction -> deref, callee input args, return val, unsafe callee function for lifetime detection, user cases, etc.
        //   -> Propagation based on the alias set with flow-sensitive order
        // Split into different BugCheckers: type and lifetime bug checkers
        fn analyze_path(
            &mut self,
            // bb_path: Vec<usize>,
            body: ir::Body<'tcx>,
            body_id: BodyId,
        ) -> (
            Option<Vec<(usize, Vec<TypeBehaviorFlag>, usize, Vec<TypeBehaviorFlag>)>>,
            FunctionSources,
            FunctionSources,
        ) {
            let mut taint_analyzer_path = TaintAnalyzerPath::new(&body);
            // let mut local_sources = FunctionSources::default();
            let mut function_sources = FunctionSources::default();
            let arg_size = self.body.arg_size;

            // The lifetime of generic_concretized_types should be as long as 'tcx lifetime at the beginning! -> When the 'tcx lifetime is used or declared?
            let mut generic_concretized_types: Vec<TyKind<'tcx>> = Vec::new();
            let mock_int = TyKind::Int(IntTy::I32);
            let mock_uint = TyKind::Uint(UintTy::U32);
            let mock_float = TyKind::Float(FloatTy::F32);
            let mock_bool = TyKind::Bool;
            let mock_char = TyKind::Char;

            let mock_rawptr = TyKind::RawPtr(TypeAndMut {
                ty: self.rcx.tcx().mk_ty_from_kind(mock_int.clone()),
                mutbl: Mutability::Not,
            });
            generic_concretized_types.push(mock_rawptr);

            for (bb_index, basic_block_ref) in body.basic_blocks.iter().enumerate() {
                let basic_block = basic_block_ref.clone();

                // Analyze statements -> Mark sources and sinks
                for (id, statement) in basic_block.statements.iter().enumerate() {
                    match statement.kind {
                        ir::StatementKind::Assign {
                            lplace,
                            rplace,
                            kind,
                            ltype,
                            rtype,
                            castkind,
                        } => {
                            if kind == 1 {
                                continue;
                            } else if kind == 2 || kind == 3 {
                                // Compare types -> Perform castkind filtering
                                //      -> Analyze AddressOf
                                //      -> Analyze CastKind (PtrToPtr, Transmute, **FnPtrToPtr**), Unhandled: IntToInt, FloatToFloat, FloatToInt, IntToFloat, **PointerExposeAddress**, **PointerFromExposedAddress**, PointerCoercion, DynStar,
                                //                 CastKind (Transmute) should not occur between differently-sized types from definition

                                // Compare types:
                                // a) Direct compare
                                // b) Calculate type layout: including size, alignment, and the relative offsets of its fields, as well as representations (using TyKind::Adt. e.g., repr[]).
                                //      -> Primitive type layout -> Sequence type layout (Adt type layout, e.g., enum and its discriminant...) -> Pointers and References layout
                                //      -> Type encoding, generics and trait bounds (including From/Into traits). Features such as Smart pointers, std lib types...
                                //      -> Imitate tcx.types (E.g., tcx.types.u8, etc.)

                                if let Some(mir::CastKind::PointerExposeAddress)
                                | Some(mir::CastKind::PointerFromExposedAddress)
                                | Some(mir::CastKind::PointerCoercion(_))
                                | Some(mir::CastKind::DynStar)
                                | Some(mir::CastKind::IntToInt)
                                | Some(mir::CastKind::FloatToFloat)
                                | Some(mir::CastKind::FloatToInt)
                                | Some(mir::CastKind::IntToFloat) = castkind
                                {
                                    continue;
                                }

                                // a) Direct compare
                                // 1. InferCtxt: infcx.can_eq
                                // let infcx = self.rcx.tcx().infer_ctxt().build();
                                // if !infcx.can_eq(self.param_env, *ltype, *rtype) {
                                // 2. Normalization
                                // tcx.type_of()
                                // 3. ==    ✅

                                // Same TyKind (Primitive: Int, Unit, Float...; Sequence: Adt...; Pointer: RawPtr, Ref...), Compare different size such as u8, u16, u32, u64, etc.
                                // Different TyKind
                                //      -> (Primitive, Sequence, Pointer...), Compare same size or different size
                                //      -> (E.g., RawPtr/Ref with Primitive, RawPtr with Ref, Int/Uint with Bool, etc.)
                                //      -> Corner case: transmute::<bool, u8>(...) is always sound

                                // ToDO: unwrap() for size calculation is wrong!
                                if ltype != rtype {
                                    let ltykind = ltype.kind();
                                    let rtykind = rtype.kind();

                                    // Filter AddressOf that creates correct RawPtr
                                    if castkind == None {
                                        if let TyKind::RawPtr(l_t) = ltykind {
                                            if l_t.ty.kind() == rtykind {
                                                continue;
                                            }
                                        }
                                    }

                                    let (lgeneric, lgeneric_tykind, rgeneric, rgeneric_tykind) =
                                        self.extract_generics(ltykind, rtykind);

                                    match (lgeneric, rgeneric) {
                                        // Same/Different generic names
                                        (Some(lg), Some(rg)) => {
                                            if lg == rg && lgeneric_tykind == rgeneric_tykind {
                                                continue;
                                            } else {
                                                let llocal_index = self.get_localnode_index(lplace);
                                                taint_analyzer_path.mark_source(
                                                    llocal_index,
                                                    TypeBehaviorFlag::GenericToGeneric,
                                                );
                                                self.status
                                                    .source_operations
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::<Span>::new())
                                                    .push(statement.original.source_info.span);
                                                function_sources
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::new())
                                                    .push((
                                                        statement.original.source_info.span,
                                                        TypeBehaviorFlag::GenericToGeneric,
                                                    ));
                                                continue;
                                            }
                                        }
                                        (Some(lg), None) => {
                                            if !self
                                                .rcx
                                                .get_generic_bounds_by_bodyid(body_id)
                                                .is_empty()
                                            {
                                                let bounds =
                                                    self.rcx.get_generic_bounds_by_bodyid(body_id);
                                                let lgeneric_bounds = bounds.get(&lg).unwrap();
                                                // println!("lgeneric_bounds: {:?}", lgeneric_bounds);

                                                // Replace customized trait object
                                                let mut impl_customized_trait: bool = false;
                                                let mut generic_customized_types: Vec<
                                                    TyKind<'tcx>,
                                                > = Vec::new();
                                                for l_bound in lgeneric_bounds.iter() {
                                                    if let rustc_hir::GenericBound::Trait(
                                                        polytrait_ref,
                                                        ..,
                                                    ) = l_bound
                                                    {
                                                        let trait_ref_path =
                                                            polytrait_ref.trait_ref.path;
                                                        if let rustc_hir::def::Res::Def(
                                                            defkind,
                                                            defid,
                                                        ) = trait_ref_path.res
                                                        {
                                                            if let rustc_hir::def::DefKind::Trait =
                                                                defkind
                                                            {
                                                                let mut is_std_trait: bool = false;
                                                                for segment in
                                                                    trait_ref_path.segments
                                                                {
                                                                    if TRAIT_LIST.contains(
                                                                        segment.ident.as_str(),
                                                                    ) {
                                                                        is_std_trait = true;
                                                                        break;
                                                                    }
                                                                }
                                                                if !is_std_trait {
                                                                    impl_customized_trait = true;
                                                                    // Initiate and insert customized trait
                                                                    // generic_customized_types.push();
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                // Replace primitive, sequence, pointer, etc. TyKinds
                                                if !impl_customized_trait {
                                                } else {
                                                }
                                                let llocal_index = self.get_localnode_index(lplace);
                                                taint_analyzer_path.mark_source(
                                                    llocal_index,
                                                    TypeBehaviorFlag::ConcretizedToGeneric,
                                                );
                                                self.status
                                                    .source_operations
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::<Span>::new())
                                                    .push(statement.original.source_info.span);
                                                function_sources
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::new())
                                                    .push((
                                                        statement.original.source_info.span,
                                                        TypeBehaviorFlag::ConcretizedToGeneric,
                                                    ));
                                                continue;
                                            } else {
                                            }
                                        }
                                        (None, Some(rg)) => {
                                            if !self
                                                .rcx
                                                .get_generic_bounds_by_bodyid(body_id)
                                                .is_empty()
                                            {
                                                let bounds =
                                                    self.rcx.get_generic_bounds_by_bodyid(body_id);
                                                let rgeneric_bounds = bounds.get(&rg).unwrap();
                                                // println!("rgeneric_bounds: {:?}", rgeneric_bounds);

                                                // Replace customized trait object
                                                let mut impl_customized_trait: bool = false;
                                                let mut generic_customized_types: Vec<
                                                    TyKind<'tcx>,
                                                > = Vec::new();
                                                for r_bound in rgeneric_bounds.iter() {
                                                    if let rustc_hir::GenericBound::Trait(
                                                        polytrait_ref,
                                                        ..,
                                                    ) = r_bound
                                                    {
                                                        let trait_ref_path =
                                                            polytrait_ref.trait_ref.path;
                                                        if let rustc_hir::def::Res::Def(
                                                            defkind,
                                                            defid,
                                                        ) = trait_ref_path.res
                                                        {
                                                            if let rustc_hir::def::DefKind::Trait =
                                                                defkind
                                                            {
                                                                let mut is_std_trait: bool = false;
                                                                for segment in
                                                                    trait_ref_path.segments
                                                                {
                                                                    if TRAIT_LIST.contains(
                                                                        segment.ident.as_str(),
                                                                    ) {
                                                                        is_std_trait = true;
                                                                        break;
                                                                    }
                                                                }
                                                                if !is_std_trait {
                                                                    impl_customized_trait = true;
                                                                    // Initiate and insert customized trait
                                                                    // generic_customized_types.push();
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                // Replace primitive, sequence, pointer, etc. TyKinds
                                                if !impl_customized_trait {
                                                    // for generic_type in &generic_concretized_types {
                                                    //     self.analyze_concretized_type_conversion(
                                                    //         lplace.clone(),
                                                    //         rplace.clone(),
                                                    //         ltykind,
                                                    //         generic_type,
                                                    //         statement,
                                                    //         &mut taint_analyzer_path,
                                                    //     );
                                                    // }
                                                } else {
                                                }
                                                let llocal_index = self.get_localnode_index(lplace);
                                                taint_analyzer_path.mark_source(
                                                    llocal_index,
                                                    TypeBehaviorFlag::GenericToConcretized,
                                                );
                                                self.status
                                                    .source_operations
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::<Span>::new())
                                                    .push(statement.original.source_info.span);
                                                function_sources
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::new())
                                                    .push((
                                                        statement.original.source_info.span,
                                                        TypeBehaviorFlag::GenericToConcretized,
                                                    ));
                                                continue;
                                            } else {
                                            }
                                        }
                                        _ => {
                                            for (source_key, source_values) in self
                                                .analyze_concretized_type_conversion(
                                                    lplace.clone(),
                                                    rplace.clone(),
                                                    ltykind,
                                                    rtykind,
                                                    statement,
                                                    &mut taint_analyzer_path,
                                                )
                                            {
                                                function_sources
                                                    .entry(source_key)
                                                    .or_insert(Vec::new())
                                                    .extend(source_values);
                                            }
                                        }
                                    }
                                }

                                /*
                                // b) Compare size and alignment, respectively. Same size and alignment could still be the same type.
                                // 1. rustc_middle::ty::layout::SizeSkeleton -> check layout size
                                let lsizeskeleton =
                                    SizeSkeleton::compute(*ltype, self.rcx.tcx(), self.param_env)
                                        .unwrap();
                                let rsizeskeleton =
                                    SizeSkeleton::compute(*rtype, self.rcx.tcx(), self.param_env)
                                        .unwrap();
                                let compare = lsizeskeleton.same_size(rsizeskeleton);
                                println!("@@@@@@===FLASH: 2 SizeSkeleton Compare===@@@@@@");
                                println!("lsizeskeleton: {:?}", lsizeskeleton);
                                println!("rsizeskeleton: {:?}", rsizeskeleton);
                                println!("compare: {:?}", compare);

                                // 2. std::mem::size_of::<T>() -> size    ❌
                                // 3. std::mem::size_of_val<T>(val: &T) -> size
                                // 4. std::mem::align_of::<T>() -> alignment    ❌
                                // 5. std::mem::align_of_val<T>(val: &T) -> alignment
                                // 6. std::mem::offset_of::<T, U>(field: U) -> offset
                                let lsize = std::mem::size_of_val(ltype);
                                let lalign = std::mem::align_of_val(ltype);
                                let rsize = std::mem::size_of_val(rtype);
                                let ralign = std::mem::align_of_val(rtype);
                                let size_compare = lsize == rsize;
                                let align_compare = lalign == ralign;
                                println!("@@@@@@===FLASH: 3 std::mem Compare===@@@@@@");
                                println!("lsize: {:?}, lalign: {:?}", lsize, lalign);
                                println!("rsize: {:?}, ralign: {:?}", rsize, ralign);
                                println!(
                                    "size_compare: {:?}, align_compare: {:?}",
                                    size_compare, align_compare
                                );
                                */
                            } else if kind == 0 {
                                self.analyze_deref_sink_operations(
                                    lplace.clone(),
                                    rplace.clone(),
                                    statement,
                                    &mut taint_analyzer_path,
                                );
                            }
                        }
                        _ => (),
                    }
                }

                // Analyze terminator
                let terminator = basic_block.terminator.clone();
                // Mark sinks and sources
                match terminator.kind {
                    // ir::TerminatorKind::Return => {
                    //     taint_analyzer_path.mark_sink(0, TypeBehaviorFlag::FunctionReturnValue);
                    //     self.status
                    //         .sink_operations
                    //         .entry(0)
                    //         .or_insert(Vec::<Span>::new())
                    //         .push(terminator.original.source_info.span);
                    // }
                    ir::TerminatorKind::StaticCall {
                        callee_did,
                        callee_substs,
                        func,
                        args,
                        destination,
                    } => {
                        // General sink construction: Map the local indexes of the args between caller and callee
                        for arg in args.clone() {
                            match arg {
                                Operand::Copy(rplace) => {
                                    let rlocal_index = self.get_localnode_index(rplace);
                                    taint_analyzer_path.mark_sink(
                                        rlocal_index,
                                        TypeBehaviorFlag::FunctionInputArgs,
                                    );
                                    self.status
                                        .sink_operations
                                        .entry(rlocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(terminator.original.source_info.span);
                                }
                                Operand::Move(rplace) => {
                                    let rlocal_index = self.get_localnode_index(rplace);
                                    taint_analyzer_path.mark_sink(
                                        rlocal_index,
                                        TypeBehaviorFlag::FunctionInputArgs,
                                    );
                                    self.status
                                        .sink_operations
                                        .entry(rlocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(terminator.original.source_info.span);
                                }
                                Operand::Constant(_) => {}
                            }
                        }
                        // String-based source and sink construction
                        let (lplace, _) = destination.unwrap();
                        let llocal_index = self.get_localnode_index(lplace);
                        let tcx = self.rcx.tcx();
                        let ext = tcx.ext();
                        // 1) Check for lifetime expand, type conversion bypass
                        let symbol_vec = ext.get_def_path(callee_did);
                        if paths::LIFETIME_EXPAND_LIST.contains(&symbol_vec) {
                            let lifetime_behavior = LIFETIME_EXPAND_MAP.get(&symbol_vec).unwrap();
                            taint_analyzer_path.mark_source(llocal_index, *lifetime_behavior);
                            self.status
                                .source_operations
                                .entry(llocal_index)
                                .or_insert(Vec::<Span>::new())
                                .push(terminator.original.source_info.span);
                            function_sources
                                .entry(llocal_index)
                                .or_insert(Vec::new())
                                .push((terminator.original.source_info.span, *lifetime_behavior));
                        } else if paths::SINK_FN_LIST.contains(&symbol_vec) {
                            let sink_behavior = SINK_FN_MAP.get(&symbol_vec).unwrap();
                            for arg in args.clone() {
                                match arg {
                                    Operand::Copy(rplace) => {
                                        let rlocal_index = self.get_localnode_index(rplace);
                                        taint_analyzer_path.mark_sink(rlocal_index, *sink_behavior);
                                        self.status
                                            .sink_operations
                                            .entry(rlocal_index)
                                            .or_insert(Vec::<Span>::new())
                                            .push(terminator.original.source_info.span);
                                    }
                                    Operand::Move(rplace) => {
                                        let rlocal_index = self.get_localnode_index(rplace);
                                        taint_analyzer_path.mark_sink(rlocal_index, *sink_behavior);
                                        self.status
                                            .sink_operations
                                            .entry(rlocal_index)
                                            .or_insert(Vec::<Span>::new())
                                            .push(terminator.original.source_info.span);
                                    }
                                    Operand::Constant(_) => {}
                                }
                            }
                        } else {
                            // Check for unresolvable generic function calls
                            match Instance::resolve(
                                self.rcx.tcx(),
                                self.param_env,
                                callee_did,
                                callee_substs,
                            ) {
                                Err(_e) => log_err!(ResolveError),
                                Ok(Some(_)) => {
                                    // Calls were successfully resolved
                                }
                                Ok(None) => {
                                    // Call contains unresolvable generic parts
                                    // Here, we are making a two step approximation:
                                    // 1. Unresolvable generic code is potentially user-provided
                                    // 2. User-provided code potentially panics
                                    for arg in args.clone() {
                                        match arg {
                                            Operand::Copy(rplace) => {
                                                let rlocal_index = self.get_localnode_index(rplace);
                                                taint_analyzer_path.mark_sink(
                                                    rlocal_index,
                                                    TypeBehaviorFlag::UnresolvableGenericFunction,
                                                );
                                                self.status
                                                    .sink_operations
                                                    .entry(rlocal_index)
                                                    .or_insert(Vec::<Span>::new())
                                                    .push(terminator.original.source_info.span);
                                            }
                                            Operand::Move(rplace) => {
                                                let rlocal_index = self.get_localnode_index(rplace);
                                                taint_analyzer_path.mark_sink(
                                                    rlocal_index,
                                                    TypeBehaviorFlag::UnresolvableGenericFunction,
                                                );
                                                self.status
                                                    .sink_operations
                                                    .entry(rlocal_index)
                                                    .or_insert(Vec::<Span>::new())
                                                    .push(terminator.original.source_info.span);
                                            }
                                            Operand::Constant(_) => {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ir::TerminatorKind::SwitchInt { discr, targets } => match discr {
                        Operand::Copy(rplace) => {
                            let rlocal_index = self.get_localnode_index(rplace);
                            taint_analyzer_path
                                .mark_sink(rlocal_index, TypeBehaviorFlag::ControlFlowDiverge);
                            self.status
                                .sink_operations
                                .entry(rlocal_index)
                                .or_insert(Vec::<Span>::new())
                                .push(terminator.original.source_info.span);
                        }
                        Operand::Move(rplace) => {
                            let rlocal_index = self.get_localnode_index(rplace);
                            taint_analyzer_path
                                .mark_sink(rlocal_index, TypeBehaviorFlag::ControlFlowDiverge);
                            self.status
                                .sink_operations
                                .entry(rlocal_index)
                                .or_insert(Vec::<Span>::new())
                                .push(terminator.original.source_info.span);
                        }
                        Operand::Constant(_) => {}
                    },
                    _ => {}
                }
            }

            // Path traversal without fixed-point iterations
            // for &bb_index in bb_path.iter() {
            //     let basic_block = body.basic_blocks[bb_index].clone();
            // }

            // Update inter-procedural functions taint sources
            let callee_ids = self.func_summary.get_callee_functions(body_id);
            for callee_id in callee_ids {
                let callee_function_sources = self.func_summary.get_function_sources(callee_id);
                let callee_idxs = callee_function_sources
                    .keys()
                    .cloned()
                    .collect::<Vec<usize>>();
                for callee_idx in callee_idxs {
                    let function_sources_values = callee_function_sources.get(&callee_idx).unwrap();
                    let caller_idxs = self
                        .func_summary
                        .get_inter_functions_local_map_caller_idxs(body_id, callee_id, callee_idx);

                    for caller_idx in caller_idxs {
                        for (source_span, source_behavior) in function_sources_values.iter() {
                            taint_analyzer_path.mark_source(caller_idx, *source_behavior);
                            self.status
                                .source_operations
                                .entry(caller_idx)
                                .or_insert(Vec::<Span>::new())
                                .push(*source_span); // Inter- span
                            function_sources
                                .entry(caller_idx)
                                .or_insert(Vec::new())
                                .push((*source_span, *source_behavior));
                        }
                    }
                }
            }

            // Propagation
            let (result, function_sources_propagated, function_sources_taint) = taint_analyzer_path
                .propagate(
                    self.body.local_nodes.clone(),
                    &function_sources,
                    // &self.status.source_operations,
                    arg_size,
                );
            if result.is_empty() {
                (None, function_sources_propagated, function_sources_taint)
            } else {
                (
                    Some(result),
                    function_sources_propagated,
                    function_sources_taint,
                )
            }
        }

        /*
        // Analyze function body
        fn analyze(&mut self) -> UnsafeDataflowStatus {
            let mut taint_analyzer = TaintAnalyzer::new(self.body);

            // Inter-procedural taint-analysis
            for (id, terminator) in self.body.terminators().enumerate() {
                match terminator.kind {
                    ir::TerminatorKind::StaticCall {
                        callee_did,
                        callee_substs,
                        ref args,
                        ..
                    } => {
                        let tcx = self.rcx.tcx();
                        let ext = tcx.ext();
                        // Check for lifetime bypass
                        let symbol_vec = ext.get_def_path(callee_did);
                        if paths::STRONG_LIFETIME_BYPASS_LIST.contains(&symbol_vec) {
                            if self.fn_called_on_copy(
                                (callee_did, args),
                                &[&PTR_READ[..], &PTR_DIRECT_READ[..]],
                            ) {
                                // read on Copy types is not a lifetime bypass.
                                continue;
                            }

                            if ext.match_def_path(callee_did, &VEC_SET_LEN)
                                && vec_set_len_to_0(self.rcx, callee_did, args)
                            {
                                // Leaking data is safe (`vec.set_len(0);`)
                                continue;
                            }

                            taint_analyzer
                                .mark_source(id, STRONG_BYPASS_MAP.get(&symbol_vec).unwrap());
                            self.status
                                .strong_bypasses
                                .push(terminator.original.source_info.span);
                        } else if paths::WEAK_LIFETIME_BYPASS_LIST.contains(&symbol_vec) {
                            if self.fn_called_on_copy(
                                (callee_did, args),
                                &[&PTR_WRITE[..], &PTR_DIRECT_WRITE[..]],
                            ) {
                                // writing Copy types is not a lifetime bypass.
                                continue;
                            }

                            taint_analyzer
                                .mark_source(id, WEAK_BYPASS_MAP.get(&symbol_vec).unwrap());
                            self.status
                                .weak_bypasses
                                .push(terminator.original.source_info.span);
                        } else if paths::GENERIC_FN_LIST.contains(&symbol_vec) {
                            taint_analyzer.mark_sink(id);
                            self.status
                                .unresolvable_generic_functions
                                .push(terminator.original.source_info.span);
                        } else {
                            // Check for unresolvable generic function calls
                            match Instance::resolve(
                                self.rcx.tcx(),
                                self.param_env,
                                callee_did,
                                callee_substs,
                            ) {
                                Err(_e) => log_err!(ResolveError),
                                Ok(Some(_)) => {
                                    // Calls were successfully resolved
                                }
                                Ok(None) => {
                                    // Call contains unresolvable generic parts
                                    // Here, we are making a two step approximation:
                                    // 1. Unresolvable generic code is potentially user-provided
                                    // 2. User-provided code potentially panics
                                    taint_analyzer.mark_sink(id);
                                    self.status
                                        .unresolvable_generic_functions
                                        .push(terminator.original.source_info.span);
                                }
                            }
                        }
                    }
                    _ => (),
                }
            }

            self.status.behavior_flag = taint_analyzer.propagate();
            self.status.clone()
        }
         */

        fn fn_called_on_copy(
            &self,
            (callee_did, callee_args): (DefId, &Vec<Operand<'tcx>>),
            paths: &[&[&str]],
        ) -> bool {
            let tcx = self.rcx.tcx();
            let ext = tcx.ext();
            for path in paths.iter() {
                if ext.match_def_path(callee_did, path) {
                    for arg in callee_args.iter() {
                        if_chain! {
                            if let Operand::Move(place) = arg;
                            let place_ty = place.ty(self.body.get_all_local_decls(tcx).unwrap(), tcx);
                            if let TyKind::RawPtr(ty_and_mut) = place_ty.ty.kind();
                            let pointed_ty = ty_and_mut.ty;
                            if pointed_ty.is_copy_modulo_regions(tcx, self.param_env);
                            then {
                                return true;
                            }
                        }
                        // No need to inspect beyond first arg of the
                        // target bypass functions.
                        break;
                    }
                }
            }
            false
        }

        // TypeBehaviorFlag bug oracles decoupled functions
        pub fn analyze_concretized_type_conversion<'b, G: Graph>(
            &mut self,
            lplace: Place<'tcx>,
            rplace: Place<'tcx>,
            ltykind: &'tcx TyKind<'tcx>,
            rtykind: &'tcx TyKind<'tcx>,
            statement: &ir::Statement<'tcx>,
            taint_analyzer_path: &mut TaintAnalyzerPath<'b, G, TypeBehaviorFlag>,
            // mut taint_analyzer_path: TaintAnalyzerPath<'b, G, TypeBehaviorFlag>,
        ) -> FunctionSources {
            let mut function_sources = FunctionSources::default(); // Actually only one func_source operation
            let arg_size = self.body.arg_size;

            match (ltykind, rtykind) {
                // Same TyKind. CastKind: PtrToPtr, FnPtrToPtr.
                (TyKind::RawPtr(l_t), TyKind::RawPtr(r_t)) => {
                    // Case 1: Immut -> Mut
                    if l_t.mutbl == mir::Mutability::Mut && r_t.mutbl == mir::Mutability::Not {
                        let llocal_index = self.get_localnode_index(lplace);
                        taint_analyzer_path
                            .mark_source(llocal_index, TypeBehaviorFlag::ImmutPtrToMutPtr);
                        self.status
                            .source_operations
                            .entry(llocal_index)
                            .or_insert(Vec::<Span>::new())
                            .push(statement.original.source_info.span);
                        function_sources
                            .entry(llocal_index)
                            .or_insert(Vec::new())
                            .push((
                                statement.original.source_info.span,
                                TypeBehaviorFlag::ImmutPtrToMutPtr,
                            ));
                        // continue;
                        return function_sources;
                    }
                    if l_t.ty == r_t.ty {
                        // continue;
                        return function_sources;
                    }
                    let l_t_ty = l_t.ty;
                    let r_t_ty = r_t.ty;
                    let l_tkind = l_t.ty.kind();
                    let r_tkind = r_t.ty.kind();
                    // Case 2: Primitive type: size comparison
                    if l_t_ty.is_primitive() && r_t_ty.is_primitive() {
                        match (l_tkind, r_tkind) {
                            // Detailed handle for Int, Uint, Float, Bool, Char
                            (TyKind::Uint(l_u), TyKind::Uint(r_u)) => {
                                if l_u == r_u {
                                    // continue;
                                    return function_sources;
                                } else if l_u > r_u {
                                    // u8 -> u32
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::SmallToBigUintToUint,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::SmallToBigUintToUint,
                                        ));
                                    // continue;
                                    return function_sources;
                                } else if l_u < r_u {
                                    // u32 -> u8, u32 exceed the range of u8
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::BigToSmallUintToUint,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::BigToSmallUintToUint,
                                        ));
                                    // continue;
                                    return function_sources;
                                }
                            }
                            (TyKind::Int(l_i), TyKind::Int(r_i)) => {
                                if l_i == r_i {
                                    // continue;
                                    return function_sources;
                                } else if l_i > r_i {
                                    // i8 -> i32
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::SmallToBigIntToInt,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::SmallToBigIntToInt,
                                        ));
                                    // continue;
                                    return function_sources;
                                } else if l_i < r_i {
                                    // i32 -> i8, i32 exceed the range of i8
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::BigToSmallIntToInt,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::BigToSmallIntToInt,
                                        ));
                                    // continue;
                                    return function_sources;
                                }
                            }
                            (TyKind::Uint(l_u), TyKind::Int(r_i)) => {
                                if l_u.to_signed() == *r_i {
                                    // continue;
                                    return function_sources;
                                } else if l_u.to_signed() > *r_i {
                                    // i8 -> u32, i8 negative
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::SmallToBigIntToUint,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::SmallToBigIntToUint,
                                        ));
                                    // continue;
                                    return function_sources;
                                } else if l_u.to_signed() < *r_i {
                                    // i32 -> u8, i32 exceed or negative
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::BigToSmallIntToUint,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::BigToSmallIntToUint,
                                        ));
                                    // continue;
                                    return function_sources;
                                }
                            }
                            (TyKind::Int(l_i), TyKind::Uint(r_u)) => {
                                if l_i.to_unsigned() == *r_u {
                                    // continue;
                                    return function_sources;
                                } else if l_i.to_unsigned() > *r_u {
                                    // u8 -> i32
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::SmallToBigUintToInt,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::SmallToBigUintToInt,
                                        ));
                                    // continue;
                                    return function_sources;
                                } else if l_i.to_unsigned() < *r_u {
                                    // u32 -> i8, u32 exceed
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::BigToSmallUintToInt,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::BigToSmallUintToInt,
                                        ));
                                    // continue;
                                    return function_sources;
                                }
                            }
                            _ => {
                                let l_t_size = l_t_ty.primitive_size(self.rcx.tcx()).bytes_usize();
                                let r_t_size = r_t_ty.primitive_size(self.rcx.tcx()).bytes_usize();
                                if l_t_size == r_t_size {
                                    if (!l_t_ty.is_floating_point() && !r_t_ty.is_floating_point())
                                        || (!l_t_ty.is_bool() && !r_t_ty.is_bool())
                                        || (!l_t_ty.is_char() && !r_t_ty.is_char())
                                    {
                                        let llocal_index = self.get_localnode_index(lplace);
                                        taint_analyzer_path.mark_source(
                                            llocal_index,
                                            TypeBehaviorFlag::DifferentPrimitiveType,
                                        );
                                        self.status
                                            .source_operations
                                            .entry(llocal_index)
                                            .or_insert(Vec::<Span>::new())
                                            .push(statement.original.source_info.span);
                                        function_sources
                                            .entry(llocal_index)
                                            .or_insert(Vec::new())
                                            .push((
                                                statement.original.source_info.span,
                                                TypeBehaviorFlag::DifferentPrimitiveType,
                                            ));
                                        // continue;
                                        return function_sources;
                                    }
                                } else if l_t_size > r_t_size {
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::SmallToBigSizePrimitive,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::SmallToBigSizePrimitive,
                                        ));
                                    // continue;
                                    return function_sources;
                                } else if l_t_size < r_t_size {
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::BigToSmallSizePrimitive,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::BigToSmallSizePrimitive,
                                        ));
                                    // continue;
                                    return function_sources;
                                }
                            }
                        }
                    }
                    // Case 3: Sequence type: seq -> seq. No size restriction.
                    if is_sequence(&l_t_ty) && is_sequence(&r_t_ty) {
                        let l_t_size =
                            SizeSkeleton::compute(l_t_ty, self.rcx.tcx(), self.param_env).unwrap();
                        let r_t_size =
                            SizeSkeleton::compute(r_t_ty, self.rcx.tcx(), self.param_env).unwrap();
                        let compare = l_t_size.same_size(r_t_size);
                        match (l_t_size, r_t_size) {
                            (SizeSkeleton::Known(l_size), SizeSkeleton::Known(r_size)) => {
                                let l_size_usize = l_size.bytes_usize();
                                let r_size_usize = r_size.bytes_usize();
                                if l_size_usize == r_size_usize {
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::DifferentSequenceType,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::DifferentSequenceType,
                                        ));
                                    // continue;
                                    return function_sources;
                                } else if l_size_usize > r_size_usize {
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::SmallToBigSizeSequence,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::SmallToBigSizeSequence,
                                        ));
                                    // continue;
                                    return function_sources;
                                } else if l_size_usize < r_size_usize {
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::BigToSmallSizeSequence,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::BigToSmallSizeSequence,
                                        ));
                                    // continue;
                                    return function_sources;
                                }
                            }
                            _ => {
                                let llocal_index = self.get_localnode_index(lplace);
                                taint_analyzer_path.mark_source(
                                    llocal_index,
                                    TypeBehaviorFlag::DifferentSequenceType,
                                );
                                self.status
                                    .source_operations
                                    .entry(llocal_index)
                                    .or_insert(Vec::<Span>::new())
                                    .push(statement.original.source_info.span);
                                function_sources
                                    .entry(llocal_index)
                                    .or_insert(Vec::new())
                                    .push((
                                        statement.original.source_info.span,
                                        TypeBehaviorFlag::DifferentSequenceType,
                                    ));
                                // continue;
                                return function_sources;
                            }
                        }
                    }
                    // Case 4: Other different TyKind. seq -> prim, prim -> seq, ref, str, etc.
                    // Can add special case report
                    match (l_tkind, r_tkind) {
                        _ => {
                            let l_size_skeleton =
                                SizeSkeleton::compute(l_t_ty, self.rcx.tcx(), self.param_env);
                            let r_size_skeleton =
                                SizeSkeleton::compute(r_t_ty, self.rcx.tcx(), self.param_env);
                            match (l_size_skeleton, r_size_skeleton) {
                                (Ok(l_t_size), Ok(r_t_size)) => {
                                    let compare = l_t_size.same_size(r_t_size);
                                    match (l_t_size, r_t_size) {
                                        (
                                            SizeSkeleton::Known(l_size),
                                            SizeSkeleton::Known(r_size),
                                        ) => {
                                            let l_size_usize = l_size.bytes_usize();
                                            let r_size_usize = r_size.bytes_usize();
                                            if l_size_usize == r_size_usize {
                                                let llocal_index = self.get_localnode_index(lplace);
                                                taint_analyzer_path.mark_source(
                                                    llocal_index,
                                                    TypeBehaviorFlag::DifferentRawPtrType,
                                                );
                                                self.status
                                                    .source_operations
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::<Span>::new())
                                                    .push(statement.original.source_info.span);
                                                function_sources
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::new())
                                                    .push((
                                                        statement.original.source_info.span,
                                                        TypeBehaviorFlag::DifferentRawPtrType,
                                                    ));
                                                // continue;
                                                return function_sources;
                                            } else if l_size_usize > r_size_usize {
                                                let llocal_index = self.get_localnode_index(lplace);
                                                taint_analyzer_path.mark_source(
                                                    llocal_index,
                                                    TypeBehaviorFlag::SmallToBigSizeRawPtr,
                                                );
                                                self.status
                                                    .source_operations
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::<Span>::new())
                                                    .push(statement.original.source_info.span);
                                                function_sources
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::new())
                                                    .push((
                                                        statement.original.source_info.span,
                                                        TypeBehaviorFlag::SmallToBigSizeRawPtr,
                                                    ));
                                                // continue;
                                                return function_sources;
                                            } else if l_size_usize < r_size_usize {
                                                let llocal_index = self.get_localnode_index(lplace);
                                                taint_analyzer_path.mark_source(
                                                    llocal_index,
                                                    TypeBehaviorFlag::BigToSmallSizeRawPtr,
                                                );
                                                self.status
                                                    .source_operations
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::<Span>::new())
                                                    .push(statement.original.source_info.span);
                                                function_sources
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::new())
                                                    .push((
                                                        statement.original.source_info.span,
                                                        TypeBehaviorFlag::BigToSmallSizeRawPtr,
                                                    ));
                                                // continue;
                                                return function_sources;
                                            } else {
                                                return function_sources;
                                            }
                                        }
                                        _ => {
                                            let llocal_index = self.get_localnode_index(lplace);
                                            taint_analyzer_path.mark_source(
                                                llocal_index,
                                                TypeBehaviorFlag::DifferentRawPtrType,
                                            );
                                            self.status
                                                .source_operations
                                                .entry(llocal_index)
                                                .or_insert(Vec::<Span>::new())
                                                .push(statement.original.source_info.span);
                                            function_sources
                                                .entry(llocal_index)
                                                .or_insert(Vec::new())
                                                .push((
                                                    statement.original.source_info.span,
                                                    TypeBehaviorFlag::DifferentRawPtrType,
                                                ));
                                            // continue;
                                            return function_sources;
                                        }
                                    }
                                }
                                _ => {
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::DifferentRawPtrType,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::DifferentRawPtrType,
                                        ));
                                    // continue;
                                    return function_sources;
                                }
                            }
                        }
                    }
                }
                // Same TyKind. CastKind: Transmute. Not frequent than RawPtr
                (TyKind::Ref(l_re, l_ty, l_mut), TyKind::Ref(r_re, r_ty, r_mut)) => {
                    // Case 1: Immut -> Mut
                    if *l_mut == mir::Mutability::Mut && *r_mut == mir::Mutability::Not {
                        let llocal_index = self.get_localnode_index(lplace);
                        taint_analyzer_path
                            .mark_source(llocal_index, TypeBehaviorFlag::ImmutRefToMutRef);
                        self.status
                            .source_operations
                            .entry(llocal_index)
                            .or_insert(Vec::<Span>::new())
                            .push(statement.original.source_info.span);
                        function_sources
                            .entry(llocal_index)
                            .or_insert(Vec::new())
                            .push((
                                statement.original.source_info.span,
                                TypeBehaviorFlag::ImmutRefToMutRef,
                            ));
                        // continue;
                        return function_sources;
                    }
                    if *l_ty == *r_ty {
                        // continue;
                        return function_sources;
                    }
                    let l_t_ty = *l_ty;
                    let r_t_ty = *r_ty;
                    let l_tkind = l_t_ty.kind();
                    let r_tkind = r_t_ty.kind();
                    // Case 2: Primitive type: size comparison
                    if l_t_ty.is_primitive() && r_t_ty.is_primitive() {
                        match (l_tkind, r_tkind) {
                            // Detailed handle for Int, Uint, Float, Bool, Char
                            (TyKind::Uint(l_u), TyKind::Uint(r_u)) => {
                                if l_u == r_u {
                                    // continue;
                                    return function_sources;
                                } else if l_u > r_u {
                                    // u8 -> u32
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::SmallToBigUintToUint,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::SmallToBigUintToUint,
                                        ));
                                    // continue;
                                    return function_sources;
                                } else if l_u < r_u {
                                    // u32 -> u8, u32 exceed the range of u8
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::BigToSmallUintToUint,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::BigToSmallUintToUint,
                                        ));
                                    // continue;
                                    return function_sources;
                                }
                            }
                            (TyKind::Int(l_i), TyKind::Int(r_i)) => {
                                if l_i == r_i {
                                    // continue;
                                    return function_sources;
                                } else if l_i > r_i {
                                    // i8 -> i32
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::SmallToBigIntToInt,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::SmallToBigIntToInt,
                                        ));
                                    // continue;
                                    return function_sources;
                                } else if l_i < r_i {
                                    // i32 -> i8, i32 exceed the range of i8
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::BigToSmallIntToInt,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::BigToSmallIntToInt,
                                        ));
                                    // continue;
                                    return function_sources;
                                }
                            }
                            (TyKind::Uint(l_u), TyKind::Int(r_i)) => {
                                if l_u.to_signed() == *r_i {
                                    // continue;
                                    return function_sources;
                                } else if l_u.to_signed() > *r_i {
                                    // i8 -> u32, i8 negative
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::SmallToBigIntToUint,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::SmallToBigIntToUint,
                                        ));
                                    // continue;
                                    return function_sources;
                                } else if l_u.to_signed() < *r_i {
                                    // i32 -> u8, i32 exceed or negative
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::BigToSmallIntToUint,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::BigToSmallIntToUint,
                                        ));
                                    // continue;
                                    return function_sources;
                                }
                            }
                            (TyKind::Int(l_i), TyKind::Uint(r_u)) => {
                                if l_i.to_unsigned() == *r_u {
                                    // continue;
                                    return function_sources;
                                } else if l_i.to_unsigned() > *r_u {
                                    // u8 -> i32
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::SmallToBigUintToInt,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::SmallToBigUintToInt,
                                        ));
                                    // continue;
                                    return function_sources;
                                } else if l_i.to_unsigned() < *r_u {
                                    // u32 -> i8, u32 exceed
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::BigToSmallUintToInt,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::BigToSmallUintToInt,
                                        ));
                                    // continue;
                                    return function_sources;
                                }
                            }
                            _ => {
                                let l_t_size = l_t_ty.primitive_size(self.rcx.tcx()).bytes_usize();
                                let r_t_size = r_t_ty.primitive_size(self.rcx.tcx()).bytes_usize();
                                if l_t_size == r_t_size {
                                    if (!l_t_ty.is_floating_point() && !r_t_ty.is_floating_point())
                                        || (!l_t_ty.is_bool() && !r_t_ty.is_bool())
                                        || (!l_t_ty.is_char() && !r_t_ty.is_char())
                                    {
                                        let llocal_index = self.get_localnode_index(lplace);
                                        taint_analyzer_path.mark_source(
                                            llocal_index,
                                            TypeBehaviorFlag::DifferentPrimitiveType,
                                        );
                                        self.status
                                            .source_operations
                                            .entry(llocal_index)
                                            .or_insert(Vec::<Span>::new())
                                            .push(statement.original.source_info.span);
                                        function_sources
                                            .entry(llocal_index)
                                            .or_insert(Vec::new())
                                            .push((
                                                statement.original.source_info.span,
                                                TypeBehaviorFlag::DifferentPrimitiveType,
                                            ));
                                        // continue;
                                        return function_sources;
                                    }
                                } else if l_t_size > r_t_size {
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::SmallToBigSizePrimitive,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::SmallToBigSizePrimitive,
                                        ));
                                    // continue;
                                    return function_sources;
                                } else if l_t_size < r_t_size {
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::BigToSmallSizePrimitive,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::BigToSmallSizePrimitive,
                                        ));
                                    // continue;
                                    return function_sources;
                                }
                            }
                        }
                    }
                    // Case 3: Sequence type: seq -> seq. No size restriction.
                    if is_sequence(&l_t_ty) && is_sequence(&r_t_ty) {
                        let l_t_size =
                            SizeSkeleton::compute(l_t_ty, self.rcx.tcx(), self.param_env).unwrap();
                        let r_t_size =
                            SizeSkeleton::compute(r_t_ty, self.rcx.tcx(), self.param_env).unwrap();
                        let compare = l_t_size.same_size(r_t_size);
                        match (l_t_size, r_t_size) {
                            (SizeSkeleton::Known(l_size), SizeSkeleton::Known(r_size)) => {
                                let l_size_usize = l_size.bytes_usize();
                                let r_size_usize = r_size.bytes_usize();
                                if l_size_usize == r_size_usize {
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::DifferentSequenceType,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::DifferentSequenceType,
                                        ));
                                    // continue;
                                    return function_sources;
                                } else if l_size_usize > r_size_usize {
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::SmallToBigSizeSequence,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::SmallToBigSizeSequence,
                                        ));
                                    // continue;
                                    return function_sources;
                                } else if l_size_usize < r_size_usize {
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::BigToSmallSizeSequence,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::BigToSmallSizeSequence,
                                        ));
                                    // continue;
                                    return function_sources;
                                }
                            }
                            _ => {
                                let llocal_index = self.get_localnode_index(lplace);
                                taint_analyzer_path.mark_source(
                                    llocal_index,
                                    TypeBehaviorFlag::DifferentSequenceType,
                                );
                                self.status
                                    .source_operations
                                    .entry(llocal_index)
                                    .or_insert(Vec::<Span>::new())
                                    .push(statement.original.source_info.span);
                                function_sources
                                    .entry(llocal_index)
                                    .or_insert(Vec::new())
                                    .push((
                                        statement.original.source_info.span,
                                        TypeBehaviorFlag::DifferentSequenceType,
                                    ));
                                // continue;
                                return function_sources;
                            }
                        }
                    }
                    // Case 4: Other different TyKind. seq -> prim, prim -> seq, ref, etc.
                    match (l_tkind, r_tkind) {
                        // Can add special case report
                        _ => {
                            let l_size_skeleton =
                                SizeSkeleton::compute(l_t_ty, self.rcx.tcx(), self.param_env);
                            let r_size_skeleton =
                                SizeSkeleton::compute(r_t_ty, self.rcx.tcx(), self.param_env);
                            match (l_size_skeleton, r_size_skeleton) {
                                (Ok(l_t_size), Ok(r_t_size)) => {
                                    let compare = l_t_size.same_size(r_t_size);
                                    match (l_t_size, r_t_size) {
                                        (
                                            SizeSkeleton::Known(l_size),
                                            SizeSkeleton::Known(r_size),
                                        ) => {
                                            let l_size_usize = l_size.bytes_usize();
                                            let r_size_usize = r_size.bytes_usize();
                                            if l_size_usize == r_size_usize {
                                                let llocal_index = self.get_localnode_index(lplace);
                                                taint_analyzer_path.mark_source(
                                                    llocal_index,
                                                    TypeBehaviorFlag::DifferentRefType,
                                                );
                                                self.status
                                                    .source_operations
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::<Span>::new())
                                                    .push(statement.original.source_info.span);
                                                function_sources
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::new())
                                                    .push((
                                                        statement.original.source_info.span,
                                                        TypeBehaviorFlag::DifferentRefType,
                                                    ));
                                                // continue;
                                                return function_sources;
                                            } else if l_size_usize > r_size_usize {
                                                let llocal_index = self.get_localnode_index(lplace);
                                                taint_analyzer_path.mark_source(
                                                    llocal_index,
                                                    TypeBehaviorFlag::SmallToBigSizeRef,
                                                );
                                                self.status
                                                    .source_operations
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::<Span>::new())
                                                    .push(statement.original.source_info.span);
                                                function_sources
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::new())
                                                    .push((
                                                        statement.original.source_info.span,
                                                        TypeBehaviorFlag::SmallToBigSizeRef,
                                                    ));
                                                // continue;
                                                return function_sources;
                                            } else if l_size_usize < r_size_usize {
                                                let llocal_index = self.get_localnode_index(lplace);
                                                taint_analyzer_path.mark_source(
                                                    llocal_index,
                                                    TypeBehaviorFlag::BigToSmallSizeRef,
                                                );
                                                self.status
                                                    .source_operations
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::<Span>::new())
                                                    .push(statement.original.source_info.span);
                                                function_sources
                                                    .entry(llocal_index)
                                                    .or_insert(Vec::new())
                                                    .push((
                                                        statement.original.source_info.span,
                                                        TypeBehaviorFlag::BigToSmallSizeRef,
                                                    ));
                                                // continue;
                                                return function_sources;
                                            } else {
                                                return function_sources;
                                            }
                                        }
                                        _ => {
                                            let llocal_index = self.get_localnode_index(lplace);
                                            taint_analyzer_path.mark_source(
                                                llocal_index,
                                                TypeBehaviorFlag::DifferentRefType,
                                            );
                                            self.status
                                                .source_operations
                                                .entry(llocal_index)
                                                .or_insert(Vec::<Span>::new())
                                                .push(statement.original.source_info.span);
                                            function_sources
                                                .entry(llocal_index)
                                                .or_insert(Vec::new())
                                                .push((
                                                    statement.original.source_info.span,
                                                    TypeBehaviorFlag::DifferentRefType,
                                                ));
                                            // continue;
                                            return function_sources;
                                        }
                                    }
                                }
                                _ => {
                                    let llocal_index = self.get_localnode_index(lplace);
                                    taint_analyzer_path.mark_source(
                                        llocal_index,
                                        TypeBehaviorFlag::DifferentRefType,
                                    );
                                    self.status
                                        .source_operations
                                        .entry(llocal_index)
                                        .or_insert(Vec::<Span>::new())
                                        .push(statement.original.source_info.span);
                                    function_sources
                                        .entry(llocal_index)
                                        .or_insert(Vec::new())
                                        .push((
                                            statement.original.source_info.span,
                                            TypeBehaviorFlag::DifferentRefType,
                                        ));
                                    // continue;
                                    return function_sources;
                                }
                            }
                        }
                    }
                }
                // (TyKind::RawPtr(l_t), TyKind::RawPtr(r_t)) => {  // Another written way for debug
                //     if let Some(values) =
                //         self.status.source_operations.get_mut(&llocal_index)
                //     {
                //         values.push(statement.original.source_info.span);
                //     } else {
                //         let new_values =
                //             vec![statement.original.source_info.span];
                //         self.status
                //             .source_operations
                //             .insert(llocal_index, new_values);
                //     }
                // }
                // Same TyKind. CastKind: IntToInt, FloatToFloat, IntToFloat, FloatToInt. Int, Uint, Float. Safe and expected behaviors in rustc compiler -> Should be deprecated!
                (TyKind::Uint(l_u), TyKind::Uint(r_u)) => {
                    if l_u == r_u || l_u > r_u {
                        // continue;
                        return function_sources;
                    } else if l_u < r_u {
                        // u32 -> u8, u32 exceed the range of u8
                        let llocal_index = self.get_localnode_index(lplace);
                        taint_analyzer_path
                            .mark_source(llocal_index, TypeBehaviorFlag::BigToSmallUintToUint);
                        self.status
                            .source_operations
                            .entry(llocal_index)
                            .or_insert(Vec::<Span>::new())
                            .push(statement.original.source_info.span);
                        function_sources
                            .entry(llocal_index)
                            .or_insert(Vec::new())
                            .push((
                                statement.original.source_info.span,
                                TypeBehaviorFlag::BigToSmallUintToUint,
                            ));
                        return function_sources;
                    } else {
                        return function_sources;
                    }
                }
                (TyKind::Int(l_i), TyKind::Int(r_i)) => {
                    if l_i == r_i || l_i > r_i {
                        // continue;
                        return function_sources;
                    } else if l_i < r_i {
                        // i32 -> i8, i32 exceed the range of i8
                        let llocal_index = self.get_localnode_index(lplace);
                        taint_analyzer_path
                            .mark_source(llocal_index, TypeBehaviorFlag::BigToSmallIntToInt);
                        self.status
                            .source_operations
                            .entry(llocal_index)
                            .or_insert(Vec::<Span>::new())
                            .push(statement.original.source_info.span);
                        function_sources
                            .entry(llocal_index)
                            .or_insert(Vec::new())
                            .push((
                                statement.original.source_info.span,
                                TypeBehaviorFlag::BigToSmallIntToInt,
                            ));
                        return function_sources;
                    } else {
                        return function_sources;
                    }
                }
                (TyKind::Uint(l_u), TyKind::Int(r_i)) => {
                    if l_u.to_signed() == *r_i {
                        // continue;
                        return function_sources;
                    } else if l_u.to_signed() < *r_i {
                        // i32 -> u8, i32 exceed or negative
                        let llocal_index = self.get_localnode_index(lplace);
                        taint_analyzer_path
                            .mark_source(llocal_index, TypeBehaviorFlag::BigToSmallIntToUint);
                        self.status
                            .source_operations
                            .entry(llocal_index)
                            .or_insert(Vec::<Span>::new())
                            .push(statement.original.source_info.span);
                        function_sources
                            .entry(llocal_index)
                            .or_insert(Vec::new())
                            .push((
                                statement.original.source_info.span,
                                TypeBehaviorFlag::BigToSmallIntToUint,
                            ));
                        return function_sources;
                    } else if l_u.to_signed() > *r_i {
                        // i8 -> u32, i8 negative
                        let llocal_index = self.get_localnode_index(lplace);
                        taint_analyzer_path
                            .mark_source(llocal_index, TypeBehaviorFlag::SmallToBigIntToUint);
                        self.status
                            .source_operations
                            .entry(llocal_index)
                            .or_insert(Vec::<Span>::new())
                            .push(statement.original.source_info.span);
                        function_sources
                            .entry(llocal_index)
                            .or_insert(Vec::new())
                            .push((
                                statement.original.source_info.span,
                                TypeBehaviorFlag::SmallToBigIntToUint,
                            ));
                        return function_sources;
                    } else {
                        return function_sources;
                    }
                }
                (TyKind::Int(l_i), TyKind::Uint(r_u)) => {
                    if l_i.to_unsigned() == *r_u || l_i.to_unsigned() > *r_u {
                        // continue;
                        return function_sources;
                    } else if l_i.to_unsigned() < *r_u {
                        // u32 -> i8, u32 exceed
                        let llocal_index = self.get_localnode_index(lplace);
                        taint_analyzer_path
                            .mark_source(llocal_index, TypeBehaviorFlag::BigToSmallUintToInt);
                        self.status
                            .source_operations
                            .entry(llocal_index)
                            .or_insert(Vec::<Span>::new())
                            .push(statement.original.source_info.span);
                        function_sources
                            .entry(llocal_index)
                            .or_insert(Vec::new())
                            .push((
                                statement.original.source_info.span,
                                TypeBehaviorFlag::BigToSmallUintToInt,
                            ));
                        return function_sources;
                    } else {
                        return function_sources;
                    }
                }
                // Same TyKind. CastKind: Transmute. Whole Adts, Array, Tuple. Rare cases in MIR! Normally is the same Adt TyKind assignment.
                (TyKind::Adt(l_adtdef, l_generic), TyKind::Adt(r_adtdef, r_generic)) => {
                    if (l_adtdef == r_adtdef) && (l_generic == r_generic) {
                        // continue;
                        return function_sources;
                    }
                    let llocal_index = self.get_localnode_index(lplace);
                    taint_analyzer_path
                        .mark_source(llocal_index, TypeBehaviorFlag::DifferentAdtType);
                    self.status
                        .source_operations
                        .entry(llocal_index)
                        .or_insert(Vec::<Span>::new())
                        .push(statement.original.source_info.span);
                    function_sources
                        .entry(llocal_index)
                        .or_insert(Vec::new())
                        .push((
                            statement.original.source_info.span,
                            TypeBehaviorFlag::DifferentAdtType,
                        ));
                    return function_sources;
                }
                (TyKind::Array(l_ty, l_const), TyKind::Array(r_ty, r_const)) => {
                    if l_ty == r_ty && l_const == r_const {
                        // continue;
                        return function_sources;
                    }
                    let llocal_index = self.get_localnode_index(lplace);
                    taint_analyzer_path
                        .mark_source(llocal_index, TypeBehaviorFlag::DifferentArrayType);
                    self.status
                        .source_operations
                        .entry(llocal_index)
                        .or_insert(Vec::<Span>::new())
                        .push(statement.original.source_info.span);
                    function_sources
                        .entry(llocal_index)
                        .or_insert(Vec::new())
                        .push((
                            statement.original.source_info.span,
                            TypeBehaviorFlag::DifferentArrayType,
                        ));
                    return function_sources;
                }
                (TyKind::Tuple(l_tys), TyKind::Tuple(r_tys)) => {
                    if l_tys == r_tys {
                        // continue;
                        return function_sources;
                    }
                    let llocal_index = self.get_localnode_index(lplace);
                    taint_analyzer_path
                        .mark_source(llocal_index, TypeBehaviorFlag::DifferentTupleType);
                    self.status
                        .source_operations
                        .entry(llocal_index)
                        .or_insert(Vec::<Span>::new())
                        .push(statement.original.source_info.span);
                    function_sources
                        .entry(llocal_index)
                        .or_insert(Vec::new())
                        .push((
                            statement.original.source_info.span,
                            TypeBehaviorFlag::DifferentTupleType,
                        ));
                    return function_sources;
                }
                (TyKind::Slice(l_ty), TyKind::Slice(r_ty)) => {
                    // Incorporate with Ref. Mut or Immut
                    return function_sources;
                }
                // Different TyKind. CastKind: Transmute. Same size, transmute between different TyKind, such as: seq -> seq, seq -> prim, prim -> seq, other TyKind s.
                // Different TyKind. CastKind: AddressOf. Normally is correct. Corner cases: as_mut_ptr, as_ptr conversion!!
                (TyKind::RawPtr(l_t), _) => {
                    if l_t.ty.kind() == rtykind {
                        // continue;
                        return function_sources;
                    }
                    let llocal_index = self.get_localnode_index(lplace);
                    taint_analyzer_path.mark_source(llocal_index, TypeBehaviorFlag::AddressOf);
                    self.status
                        .source_operations
                        .entry(llocal_index)
                        .or_insert(Vec::<Span>::new())
                        .push(statement.original.source_info.span);
                    function_sources
                        .entry(llocal_index)
                        .or_insert(Vec::new())
                        .push((
                            statement.original.source_info.span,
                            TypeBehaviorFlag::AddressOf,
                        ));
                    return function_sources;
                }
                _ => {
                    let llocal_index = self.get_localnode_index(lplace);
                    taint_analyzer_path.mark_source(llocal_index, TypeBehaviorFlag::Transmute);
                    self.status
                        .source_operations
                        .entry(llocal_index)
                        .or_insert(Vec::<Span>::new())
                        .push(statement.original.source_info.span);
                    function_sources
                        .entry(llocal_index)
                        .or_insert(Vec::new())
                        .push((
                            statement.original.source_info.span,
                            TypeBehaviorFlag::Transmute,
                        ));
                    return function_sources;
                }
            }
        }

        pub fn analyze_deref_sink_operations<'b, G: Graph>(
            &mut self,
            lplace: Place<'tcx>,
            rplace: Place<'tcx>,
            statement: &ir::Statement<'tcx>,
            taint_analyzer_path: &mut TaintAnalyzerPath<'b, G, TypeBehaviorFlag>,
        ) {
            if self.has_deref_projection(lplace) {
                let llocal_index = self.get_localnode_index(lplace);
                taint_analyzer_path.mark_sink(llocal_index, TypeBehaviorFlag::Dereference);
                self.status
                    .sink_operations
                    .entry(llocal_index)
                    .or_insert(Vec::<Span>::new())
                    .push(statement.original.source_info.span);
            }
            if self.has_deref_projection(rplace) {
                let rlocal_index = self.get_localnode_index(rplace);
                taint_analyzer_path.mark_sink(rlocal_index, TypeBehaviorFlag::Dereference);
                self.status
                    .sink_operations
                    .entry(rlocal_index)
                    .or_insert(Vec::<Span>::new())
                    .push(statement.original.source_info.span);
            }
        }

        pub fn analyze_terminator_sink_operations<'b, G: Graph>(
            &mut self,
            lplace: Place<'tcx>,
            rplace: Place<'tcx>,
            terminator: &ir::Terminator<'tcx>,
            taint_analyzer_path: &mut TaintAnalyzerPath<'b, G, TypeBehaviorFlag>,
        ) {
        }

        // Extract generic name from more generic cases -> Determine generic bug detection capabilities
        //      -> Others: such as &[T].as_mut_ptr(), Vec<T>.as_mut_ptr()...
        pub fn extract_generics(
            &mut self,
            ltykind: &'tcx TyKind<'tcx>,
            rtykind: &'tcx TyKind<'tcx>,
        ) -> (Option<String>, String, Option<String>, String) {
            let mut lgeneric: Option<String> = None;
            let mut rgeneric: Option<String> = None;
            let mut l_ty_string: String = String::new();
            let mut r_ty_string: String = String::new();

            match ltykind {
                TyKind::Param(paramty) => {
                    lgeneric = Some(paramty.to_string());
                    l_ty_string = "TyKind::Param".to_string();
                }
                TyKind::RawPtr(l_t) => {
                    if let TyKind::Param(paramty) = l_t.ty.kind() {
                        lgeneric = Some(paramty.to_string());
                        l_ty_string = "TyKind::PawPtr,Param".to_string();
                    } else if let TyKind::Adt(_, l_generic_args) = l_t.ty.kind() {
                        for l_generic_arg in l_generic_args.iter() {
                            if let Some(l_ty) = l_generic_arg.as_type() {
                                if let TyKind::Param(paramty) = l_ty.kind() {
                                    lgeneric = Some(paramty.to_string());
                                    l_ty_string = "TyKind::RawPtr,Adt".to_string();
                                    break;
                                }
                            }
                        }
                    } else if let TyKind::Array(l_ty, _) = l_t.ty.kind() {
                        if let TyKind::Param(paramty) = l_ty.kind() {
                            lgeneric = Some(paramty.to_string());
                            l_ty_string = "TyKind::PawPtr,Array,Param".to_string();
                        }
                    } else if let TyKind::Tuple(l_tys) = l_t.ty.kind() {
                        for l_ty in l_tys.iter() {
                            if let TyKind::Param(paramty) = l_ty.kind() {
                                lgeneric = Some(paramty.to_string());
                                l_ty_string = "TyKind::PawPtr,Tuple,Param".to_string();
                                break;
                            }
                        }
                    }
                }
                TyKind::Ref(_, l_ty, _) => {
                    if let TyKind::Param(paramty) = l_ty.kind() {
                        lgeneric = Some(paramty.to_string());
                        l_ty_string = "TyKind::Ref,Param".to_string();
                    } else if let TyKind::Slice(l_l_ty) = l_ty.kind() {
                        if let TyKind::Param(paramty) = l_l_ty.kind() {
                            lgeneric = Some(paramty.to_string());
                            l_ty_string = "TyKind::Ref,Slice,Param".to_string();
                        }
                    } else if let TyKind::Adt(_, l_generic_args) = l_ty.kind() {
                        for l_generic_arg in l_generic_args.iter() {
                            if let Some(l_l_ty) = l_generic_arg.as_type() {
                                if let TyKind::Param(paramty) = l_l_ty.kind() {
                                    lgeneric = Some(paramty.to_string());
                                    l_ty_string = "TyKind::Ref,Adt".to_string();
                                    break;
                                }
                            }
                        }
                    } else if let TyKind::Array(l_l_ty, _) = l_ty.kind() {
                        if let TyKind::Param(paramty) = l_l_ty.kind() {
                            lgeneric = Some(paramty.to_string());
                            l_ty_string = "TyKind::Ref,Array,Param".to_string();
                        }
                    } else if let TyKind::Tuple(l_tys) = l_ty.kind() {
                        for l_l_ty in l_tys.iter() {
                            if let TyKind::Param(paramty) = l_l_ty.kind() {
                                lgeneric = Some(paramty.to_string());
                                l_ty_string = "TyKind::Ref,Tuple,Param".to_string();
                                break;
                            }
                        }
                    }
                }
                // ToDO: Only extract one generic name
                TyKind::Adt(_, l_generic_args) => {
                    for l_generic_arg in l_generic_args.iter() {
                        if let Some(l_ty) = l_generic_arg.as_type() {
                            if let TyKind::Param(paramty) = l_ty.kind() {
                                lgeneric = Some(paramty.to_string());
                                l_ty_string = "TyKind::Adt".to_string();
                                break;
                            }
                        }
                    }
                }
                TyKind::Array(l_ty, _) => {
                    if let TyKind::Param(paramty) = l_ty.kind() {
                        lgeneric = Some(paramty.to_string());
                        l_ty_string = "TyKind::Array,Param".to_string();
                    }
                }
                TyKind::Tuple(l_tys) => {
                    for l_ty in l_tys.iter() {
                        if let TyKind::Param(paramty) = l_ty.kind() {
                            lgeneric = Some(paramty.to_string());
                            l_ty_string = "TyKind::Tuple,Param".to_string();
                            break;
                        }
                    }
                }
                _ => {}
            }

            match rtykind {
                TyKind::Param(paramty) => {
                    rgeneric = Some(paramty.to_string());
                    r_ty_string = "TyKind::Param".to_string();
                }
                TyKind::RawPtr(r_t) => {
                    if let TyKind::Param(paramty) = r_t.ty.kind() {
                        rgeneric = Some(paramty.to_string());
                        r_ty_string = "TyKind::PawPtr,Param".to_string();
                    } else if let TyKind::Adt(_, r_generic_args) = r_t.ty.kind() {
                        for r_generic_arg in r_generic_args.iter() {
                            if let Some(r_ty) = r_generic_arg.as_type() {
                                if let TyKind::Param(paramty) = r_ty.kind() {
                                    rgeneric = Some(paramty.to_string());
                                    r_ty_string = "TyKind::RawPtr,Adt".to_string();
                                    break;
                                }
                            }
                        }
                    } else if let TyKind::Array(r_ty, _) = r_t.ty.kind() {
                        if let TyKind::Param(paramty) = r_ty.kind() {
                            rgeneric = Some(paramty.to_string());
                            r_ty_string = "TyKind::PawPtr,Array,Param".to_string();
                        }
                    } else if let TyKind::Tuple(r_tys) = r_t.ty.kind() {
                        for r_ty in r_tys.iter() {
                            if let TyKind::Param(paramty) = r_ty.kind() {
                                rgeneric = Some(paramty.to_string());
                                r_ty_string = "TyKind::PawPtr,Tuple,Param".to_string();
                                break;
                            }
                        }
                    }
                }
                TyKind::Ref(_, r_ty, _) => {
                    if let TyKind::Param(paramty) = r_ty.kind() {
                        rgeneric = Some(paramty.to_string());
                        r_ty_string = "TyKind::Ref,Param".to_string();
                    } else if let TyKind::Slice(r_r_ty) = r_ty.kind() {
                        if let TyKind::Param(paramty) = r_r_ty.kind() {
                            rgeneric = Some(paramty.to_string());
                            r_ty_string = "TyKind::Ref,Slice,Param".to_string();
                        }
                    } else if let TyKind::Adt(_, r_generic_args) = r_ty.kind() {
                        for r_generic_arg in r_generic_args.iter() {
                            if let Some(r_r_ty) = r_generic_arg.as_type() {
                                if let TyKind::Param(paramty) = r_r_ty.kind() {
                                    rgeneric = Some(paramty.to_string());
                                    r_ty_string = "TyKind::Ref,Adt".to_string();
                                    break;
                                }
                            }
                        }
                    } else if let TyKind::Array(r_r_ty, _) = r_ty.kind() {
                        if let TyKind::Param(paramty) = r_r_ty.kind() {
                            rgeneric = Some(paramty.to_string());
                            r_ty_string = "TyKind::Ref,Array,Param".to_string();
                        }
                    } else if let TyKind::Tuple(r_tys) = r_ty.kind() {
                        for r_r_ty in r_tys.iter() {
                            if let TyKind::Param(paramty) = r_r_ty.kind() {
                                rgeneric = Some(paramty.to_string());
                                r_ty_string = "TyKind::Ref,Tuple,Param".to_string();
                                break;
                            }
                        }
                    }
                }
                TyKind::Adt(_, r_generic_args) => {
                    for r_generic_arg in r_generic_args.iter() {
                        if let Some(r_ty) = r_generic_arg.as_type() {
                            if let TyKind::Param(paramty) = r_ty.kind() {
                                rgeneric = Some(paramty.to_string());
                                r_ty_string = "TyKind::Adt".to_string();
                                break;
                            }
                        }
                    }
                }
                TyKind::Array(r_ty, _) => {
                    if let TyKind::Param(paramty) = r_ty.kind() {
                        lgeneric = Some(paramty.to_string());
                        r_ty_string = "TyKind::Array,Param".to_string();
                    }
                }
                TyKind::Tuple(r_tys) => {
                    for r_ty in r_tys.iter() {
                        if let TyKind::Param(paramty) = r_ty.kind() {
                            lgeneric = Some(paramty.to_string());
                            r_ty_string = "TyKind::Tuple,Param".to_string();
                            break;
                        }
                    }
                }
                _ => {}
            }

            (lgeneric, l_ty_string, rgeneric, r_ty_string)
        }
    }

    fn trace_calls_in_body<'tcx>(rcx: RudraCtxt<'tcx>, body_def_id: DefId) {
        warn!("Paths discovery function has been detected");
        if let Ok(body) = rcx.translate_body(body_def_id).as_ref() {
            for terminator in body.terminators() {
                match terminator.kind {
                    ir::TerminatorKind::StaticCall { callee_did, .. } => {
                        let ext = rcx.tcx().ext();
                    }
                    _ => (),
                }
            }
        }
    }

    // Check if the argument of `Vec::set_len()` is 0_usize.
    fn vec_set_len_to_0<'tcx>(
        rcx: RudraCtxt<'tcx>,
        callee_did: DefId,
        args: &Vec<Operand<'tcx>>,
    ) -> bool {
        let tcx = rcx.tcx();
        for arg in args.iter() {
            if_chain! {
                if let Operand::Constant(c) = arg;
                if let Some(c_val) = c.const_.try_eval_target_usize(
                    tcx,
                    tcx.param_env(callee_did),
                );
                if c_val == 0;
                then {
                    // Leaking(`vec.set_len(0);`) is safe.
                    return true;
                }
            }
        }
        false
    }

    // ToDO: Ty/TyKind classification
    fn is_sequence<'tcx>(ty: &Ty<'tcx>) -> bool {
        match ty.kind() {
            TyKind::Adt(_, _) | TyKind::Array(_, _) | TyKind::Tuple(_) | TyKind::Slice(_) => true,
            _ => false,
        }
    }

    pub fn merge_scc_state(
        local_num: usize,
        alias1: &Vec<Vec<usize>>,
        alias2: &Vec<Vec<usize>>,
    ) -> Vec<Vec<usize>> {
        let mut merge_alias: Vec<Vec<usize>> = Vec::new();
        // let alias1 = self.scc_alias_by_index(idx1);
        // let alias2 = self.scc_alias_by_index(idx2);
        for idx in 0..local_num {
            let mut alias = Vec::new();
            alias.extend(alias1[idx].clone());
            alias.extend(alias2[idx].clone());
            let mut new_alias = Vec::new();
            let mut order_set: FxHashSet<usize> = FxHashSet::default();
            for &a in alias.iter() {
                if order_set.insert(a) {
                    new_alias.push(a);
                }
            }
            // Dedup and FxHashSet is incorrect! -> Keep here for reminder
            // alias = alias
            //     .clone()
            //     .into_iter()
            //     .collect::<FxHashSet<usize>>()
            //     .into_iter()
            //     .collect();
            // alias.dedup();
            merge_alias.push(new_alias);
        }
        merge_alias
    }

    pub fn compare_scc_state(
        local_num: usize,
        state1: &Vec<Vec<usize>>,
        state2: &Vec<Vec<usize>>,
    ) -> bool {
        for idx in 0..local_num {
            if state1[idx] != state2[idx] {
                return false;
            }
        }
        true
    }
}

// Unsafe Dataflow BypassKind.
// Used to associate each Unsafe-Dataflow bug report with its cause.
bitflags! {
    #[derive(Default)]
    pub struct BehaviorFlag: u16 {
        const READ_FLOW = 0b00000001;
        const COPY_FLOW = 0b00000010;
        const VEC_FROM_RAW = 0b00000100;
        const TRANSMUTE = 0b00001000;
        const WRITE_FLOW = 0b00010000;
        const PTR_AS_REF = 0b00100000;
        const SLICE_UNCHECKED = 0b01000000;
        const SLICE_FROM_RAW = 0b10000000;
        const VEC_SET_LEN = 0b100000000;
        const TYPE_CONVERSION = 0b1000000000;
    }
}

impl IntoReportLevel for BehaviorFlag {
    fn report_level(&self) -> ReportLevel {
        use BehaviorFlag as Flag;

        let high = Flag::VEC_FROM_RAW | Flag::VEC_SET_LEN;
        let med = Flag::READ_FLOW | Flag::COPY_FLOW | Flag::WRITE_FLOW;

        // Bitwise and operation, *self equal to high or med
        if !(*self & high).is_empty() {
            ReportLevel::Error
        } else if !(*self & med).is_empty() {
            ReportLevel::Warning
        } else {
            ReportLevel::Info
        }
    }
}

impl GraphTaint for BehaviorFlag {
    fn is_empty(&self) -> bool {
        self.is_all()
    }

    fn contains(&self, taint: &Self) -> bool {
        self.contains(*taint)
    }

    // Bitwise or operation, u8: 0b0011 represents both 0b0010 and 0b0001 cases
    fn join(&mut self, taint: &Self) {
        *self |= *taint;
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TypeBehaviorFlag {
    // Type conversion source operations
    ImmutPtrToMutPtr,
    ImmutRefToMutRef,
    AddressOf,
    Transmute,
    ConcretizedToGeneric,
    GenericToConcretized,
    GenericToGeneric,
    Lifetime,
    // - RawPtr/Ref Int and Uint
    BigToSmallIntToInt,
    BigToSmallUintToUint,
    BigToSmallIntToUint,
    BigToSmallUintToInt,
    SmallToBigIntToInt,
    SmallToBigUintToInt,
    SmallToBigUintToUint,
    SmallToBigIntToUint,
    // - RawPtr/Ref Primitive
    SmallToBigSizePrimitive,
    BigToSmallSizePrimitive,
    DifferentPrimitiveType,
    // - RawPtr/Ref Sequence
    SmallToBigSizeSequence,
    BigToSmallSizeSequence,
    DifferentSequenceType,
    // - RawPtr
    SmallToBigSizeRawPtr,
    BigToSmallSizeRawPtr,
    DifferentRawPtrType,
    // - Ref
    SmallToBigSizeRef,
    BigToSmallSizeRef,
    DifferentRefType,
    // - Sequence
    DifferentAdtType,
    DifferentArrayType,
    DifferentTupleType,
    // Sink operations
    Dereference,
    FunctionReturnValue,
    FunctionInputArgs,
    ArrayIndexOutOfBound,
    ArrayCapacityOverflow,
    ControlFlowDiverge,
    PtrDropInPlace,
    PtrDirectDropInPlace,
    IntrinsicsDropInPlace,
    PtrRead,
    PtrDirectRead,
    IntrinsicsCopy,
    IntrinsicsCopyNonoverlapping,
    PtrWrite,
    PtrDirectWrite,
    SliceGetUnchecked,
    SliceGetUncheckedMut,
    VecFromElem,
    VecIndex,
    StrGetUnchecked,
    StrGetUncheckedMut,
    UnresolvableGenericFunction,
    // LifetimeExpand
    VecFromRawParts,
    PtrAsRef,
    PtrAsMut,
    NonNullAsRef,
    NonNullAsMut,
    PtrSliceFromRawParts,
    PtrSliceFromRawPartsMut,
    SliceFromRawParts,
    SliceFromRawPartsMut,
    StringFromRawParts,
    BoxFromRaw,
}
