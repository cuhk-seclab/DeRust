//! Reduced MIR intended to cover many common use cases while keeping the analysis pipeline manageable.
//! Note that this is a translation of non-monomorphized, generic MIR.
use rustc_data_structures::fx::FxHashMap;
use rustc_middle::mir::HasLocalDecls;

use std::borrow::Cow;

use rustc_hir::ConstContext;
use rustc_hir::{def_id::DefId, BodyId};
use rustc_index::{IndexSlice, IndexVec};
use rustc_middle::ty;
use rustc_middle::ty::TyCtxt;
use rustc_middle::{
    mir,
    ty::{GenericArgsRef, Ty},
};
use rustc_span::Span;

use crate::analysis::{FunctionInputState, FunctionSources, TypeBehaviorFlag};

#[derive(Debug, Clone)]
pub struct Statement<'tcx> {
    pub kind: StatementKind<'tcx>,
    pub original: mir::Statement<'tcx>,
}

// Unhandled statements: FakeRead(Disallowed), Deinit, Retag(Special), PlaceMention, AscribeUserType(Disallowed), Coverage, Intrinsic, ConstEvalCounter, Nop
#[derive(Debug, Clone)]
pub enum StatementKind<'tcx> {
    // Evaluate the LHS to a place and the RHS to a value, and then store the value to the place.
    // 1. Type requirement. Assignments in which the types of the place and rvalue differ are not well-formed.
    // 2. Place: local: Place { local, projection: [] }; non-locals: ParentPlace: **Downcast**, **OpaqueCast**, Subtype are related to type conversion
    //    Place: *ptr on a dangling or unaligned pointer is never UB -> Later doing a load/store on that place or turning it into a reference can be UB though!
    //    Two place computation causing UB: Deref projection, projections that perform pointer arithmetic, the offset must in-bounds of an allocation (i.e., the preconditions of `ptr::offset` must be met).
    // 3. Rvalue: Unhandled: **Repeat** ([x; 32]), ThreadLocalRef, Len, BinaryOp, CheckedBinaryOp, NullaryOp, UnaryOp, **Discriminant**, **Aggregate**, **ShallowInitBox**,
    // 4. Operand: Copy, Move, Constant
    // Statement assign kind: general==1; sources (type inconsistency): AddressOf==2, Cast==3, etc.; sinks: Deref==0, sink chain (ToDO), etc.
    Assign {
        lplace: mir::Place<'tcx>,
        rplace: mir::Place<'tcx>,
        // rvalue: mir::Rvalue<'tcx>,
        kind: usize, // statement assign kind
        ltype: Ty<'tcx>,
        rtype: Ty<'tcx>,
        castkind: Option<mir::CastKind>,
    },
    SetDiscriminant {
        place: mir::Place<'tcx>,
        variant_index: usize,
    },
    StorageLive(mir::Local),
    StorageDead(mir::Local),
    Unrelated(Cow<'static, str>),
    Unimplemented(Cow<'static, str>),
}

#[derive(Debug, Clone)]
pub struct Terminator<'tcx> {
    pub kind: TerminatorKind<'tcx>,
    pub original: mir::Terminator<'tcx>,
}

// Unhandled terminators: SwitchInt, UnwindResume, UnwindTerminate, Unreachable, **Drop**, Assert, Yield, CoroutineDrop, FalseEdge, FalseUnwind, InlineAsm
#[derive(Debug, Clone)]
pub enum TerminatorKind<'tcx> {
    // Goto(usize), // Goto(BasicBlock.index)
    Return,
    SwitchInt {
        discr: mir::Operand<'tcx>,
        targets: mir::SwitchTargets,
    },
    Drop,
    // ToDO: Interprocedural-analysis
    StaticCall {
        callee_did: DefId,
        callee_substs: GenericArgsRef<'tcx>, // types, lifetimes, and const parameters
        func: mir::Operand<'tcx>,
        args: Vec<mir::Operand<'tcx>>,
        destination: Option<(mir::Place<'tcx>, usize)>, // usize: BasicBlock.index
                                                        // cleanup: Option<usize>,
    },
    FnPtr {
        value: mir::Const<'tcx>,
    },
    Unimplemented(Cow<'static, str>),
}

#[derive(Debug, Clone)]
pub struct BasicBlock<'tcx> {
    pub index: usize,
    pub statements: Vec<Statement<'tcx>>,
    pub terminator: Terminator<'tcx>, // Terminator kinds: ToBBs, Switch_Stmts, FnCalls, Drop, etc.
    pub is_cleanup: bool,
}

// impl<'tcx> BasicBlock<'tcx> {
//     pub fn new(index: usize, is_cleanup: bool) -> Self {
//         BasicBlock {
//             index: index,
//             statements: Vec::<Statement<'tcx>>::new(),
//             terminator: Terminator,
//             is_cleanup: is_cleanup,
//         }
//     }
// }

#[derive(Debug, Clone)]
pub struct LocalDecl<'tcx> {
    pub ty: Ty<'tcx>,
}

/// Types for locals
pub type LocalDecls<'tcx> = IndexSlice<mir::Local, mir::LocalDecl<'tcx>>;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum LocalNodeState {
    Init,
    TypeConverted,
    Used,
}

impl Default for LocalNodeState {
    fn default() -> Self {
        LocalNodeState::Init
    }
}

// Representent LocalDecls as well as ADT
#[derive(Debug, Clone)]
pub struct LocalNode<'tcx> {
    pub index: usize, // node index in tree
    pub local: usize, // local/root index
    pub ty: Ty<'tcx>,
    pub kind: usize,                     // categories of LocalNode, stack, heap, etc.
    pub alias: Vec<usize>,               // need sort, store the index
    pub fields: FxHashMap<usize, usize>, // mapping between node and its fields using index
    pub field_idx: Vec<usize>,           // field index, handle _1.f, _1.1.f, etc.
    pub state: LocalNodeState,           // state for source-sink analysis
                                         // pub tainted: bool,
}

impl<'tcx> LocalNode<'tcx> {
    pub fn new(index: usize, local: usize, ty: Ty<'tcx>, kind: usize) -> Self {
        let mut root = Vec::new();
        root.push(index);
        LocalNode {
            index: index,
            local: local,
            ty: ty,
            kind: kind,
            alias: root,
            fields: FxHashMap::default(),
            field_idx: Vec::<usize>::new(),
            state: LocalNodeState::default(),
        }
    }

    pub fn kind(&self) -> usize {
        return self.kind;
    }
}

pub fn type_kind_classify<'tcx>(local_ty: Ty<'tcx>) -> usize {
    match local_ty.kind() {
        ty::Bool | ty::Char | ty::Int(_) | ty::Uint(_) | ty::Float(_) => 0,
        ty::Array(ref tys, _) => type_kind_classify(*tys),
        ty::Adt(_, ref substs) => {
            for tys in substs.types() {
                if type_kind_classify(tys) != 0 {
                    return 1;
                }
            }
            1
        }
        ty::Tuple(ref substs) => {
            for tys in substs.iter() {
                if type_kind_classify(tys) != 0 {
                    return 1;
                }
            }
            1
        }
        _ => 1,
    }
}

// Special handling for Pointer, Reference, ADT, etc.
// compiler/rustc_middle/src/ty/sty.rs
//          Ty<'tcx> utilities: is_unit, is_primitive, is_adt, is_ref, is_phantom_data, is_bool, is_str, is_slice, is_array_slice, is_array,
//              sequence_element_type, is_mutable_ptr, is_unsafe_ptr, is_any_ptr, is_box, boxed_ty, is_scalar, ...
// compiler/rustc_type_ir/src/ty_kind.rs
//          TyKind<'tcx> utilities: is_primitive, PartialEq::eq, DebugWithInfcx::fmt, ...
pub fn type_to_string<'tcx>(local_ty: Ty<'tcx>) -> String {
    match local_ty.kind() {
        // Primitive types
        ty::Bool | ty::Char | ty::Int(_) | ty::Uint(_) | ty::Float(_) | ty::Str => {
            format!("Primitive types: {:?}", local_ty.kind())
        }
        // Sequence types
        // Adt: structures (e.g., List), enumerations and unions
        ty::Adt(_adtdef, _genericargs) => {
            format!("Adt type: {:?}", local_ty.kind())
        }
        ty::Array(_ty, _const) => {
            format!("Array type: {:?}", local_ty.kind())
        }
        ty::Tuple(_tys) => {
            format!("Tuple type: {:?}", local_ty.kind())
        }
        // Pointer types: Reference and Pointer
        ty::RawPtr(_typeandmut) => {
            format!("RawPtr type: {:?}", local_ty.kind())
        }
        ty::Ref(_region, _ty, _mut) => {
            format!("Ref type: {:?}", local_ty.kind())
        }
        ty::Slice(_ty) => {
            format!("Slice type: {:?}", local_ty.kind()) // &[T], &mut [T]
        }
        // Unhandled and ignored
        ty::Foreign(_) => {
            format!("Unhandled Foreign type: {:?}", local_ty.kind())
        }
        ty::FnDef(_, _) => {
            format!("Unhandled FnDef type: {:?}", local_ty.kind())
        }
        // Anonymous function types
        ty::FnPtr(_) => {
            format!("Unhandled FnPtr type: {:?}", local_ty.kind())
        }
        ty::Dynamic(_, _, _) => {
            format!("Unhandled Dynamic type: {:?}", local_ty.kind())
        }
        ty::Closure(_, _) => {
            format!("Unhandled Closure type: {:?}", local_ty.kind())
        }
        ty::Coroutine(_, _, _) => {
            format!("Unhandled Coroutine type: {:?}", local_ty.kind())
        }
        ty::CoroutineWitness(_, _) => {
            format!("Unhandled CoroutineWitness type: {:?}", local_ty.kind())
        }
        ty::Never => {
            format!("Unhandled Never type: {:?}", local_ty.kind())
        }
        // Generic types
        ty::Alias(_, _) => {
            // ToDO
            format!("Unhandled Alias type: {:?}", local_ty.kind())
        }
        ty::Param(_) => {
            // Generic: `T` in `fn f<T>(x: T) {}`
            format!("Param type: {:?}", local_ty.kind())
        }
        ty::Bound(_, _) => {
            // Could be lifetime annotation or Generic param: `'a` in `for<'a> fn(&'a ())`
            format!("Bound type: {:?}", local_ty.kind())
        }
        ty::Placeholder(_) => {
            format!("Unhandled Placeholder type: {:?}", local_ty.kind())
        }
        ty::Infer(_) => {
            format!("Unhandled Infer type: {:?}", local_ty.kind())
        }
        // | ty::Error(_)
        _ => "Unknown".to_string(),
    }
}

// For context-sensitive inter-procedural analysis
// FunctionInputState is the caller to callee (BodyId) state
#[derive(Debug, Clone)]
pub struct FunctionSummary {
    pub func_summary: FxHashMap<BodyId, FunctionAlias>,
    // pub func_summary: FxHashMap<(BodyId, FunctionInputState), FunctionAlias>, // Context-sensitive. ToDO: With the call depth
    pub func_sources: FxHashMap<BodyId, FunctionSources>,
    pub inter_functions: FxHashMap<BodyId, FxHashMap<BodyId, Vec<FxHashMap<usize, usize>>>>, // local_map
}

impl Default for FunctionSummary {
    fn default() -> Self {
        FunctionSummary {
            func_summary: FxHashMap::default(),
            func_sources: FxHashMap::default(),
            inter_functions: FxHashMap::default(),
        }
    }
}

impl FunctionSummary {
    pub fn new() -> Self {
        FunctionSummary {
            func_summary: FxHashMap::default(),
            func_sources: FxHashMap::default(),
            inter_functions: FxHashMap::default(),
        }
    }

    // Create BodyId key
    pub fn is_body_analyzed(&mut self, body_id: BodyId) -> bool {
        if !self.func_summary.contains_key(&body_id) {
            let func_alias = FunctionAlias::default();
            self.func_summary.insert(body_id, func_alias);
            return false;
        } else {
            if self.func_summary.get(&body_id).unwrap().is_empty() {
                return false;
            } else {
                return true;
            }
        }
    }

    pub fn get_function_summary(&self, body_id: BodyId) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
        let func_alias = self.func_summary.get(&body_id).unwrap();
        return (
            func_alias.args_alias.clone(),
            func_alias.return_alias.clone(),
        );
    }

    pub fn update_function_summary(
        &mut self,
        body_id: BodyId,
        args_alias: Vec<Vec<usize>>,
        return_alias: Vec<Vec<usize>>,
    ) {
        let func_alias = FunctionAlias::new(args_alias, return_alias);
        self.func_summary.insert(body_id, func_alias);
    }

    /*
    // Create BodyId key
    pub fn is_body_function_summary_analyzed(
        &mut self,
        body_id: BodyId,
        input_state: &FunctionInputState,
    ) -> bool {
        if !self
            .func_summary
            .contains_key(&(body_id, input_state.clone()))
        {
            let func_alias = FunctionAlias::default();
            self.func_summary
                .insert((body_id, input_state.clone()), func_alias);
            return false;
        } else {
            if self
                .func_summary
                .get(&(body_id, input_state.clone()))
                .unwrap()
                .is_empty()
            {
                return false;
            } else {
                return true;
            }
        }
    }

    pub fn get_function_summary(
        &self,
        body_id: BodyId,
        input_state: &FunctionInputState,
    ) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
        let func_alias = self
            .func_summary
            .get(&(body_id, input_state.clone()))
            .unwrap();
        return (
            func_alias.args_alias.clone(),
            func_alias.return_alias.clone(),
        );
    }

    pub fn update_function_summary(
        &mut self,
        body_id: BodyId,
        input_state: &FunctionInputState,
        args_alias: Vec<Vec<usize>>,
        return_alias: Vec<Vec<usize>>,
    ) {
        let func_alias = FunctionAlias::new(args_alias, return_alias);
        self.func_summary
            .insert((body_id, input_state.clone()), func_alias);
    }
     */

    pub fn is_body_function_sources_analyzed(&mut self, body_id: BodyId) -> bool {
        if !self.func_sources.contains_key(&body_id) {
            let func_sources = FunctionSources::default();
            self.func_sources.insert(body_id, func_sources);
            return false;
        } else {
            if self.func_sources.get(&body_id).unwrap().is_empty() {
                return false;
            } else {
                return true;
            }
        }
    }

    pub fn get_function_sources(&mut self, body_id: BodyId) -> FunctionSources {
        // Add an error handling bound
        if self.is_body_function_sources_analyzed(body_id) {
            self.func_sources.get(&body_id).unwrap().clone()
        } else {
            FunctionSources::default()
        }
    }

    pub fn update_function_sources(&mut self, body_id: BodyId, sources: FunctionSources) {
        self.func_sources.insert(body_id, sources);
    }

    pub fn get_callee_functions(&self, caller_id: BodyId) -> Vec<BodyId> {
        if self.inter_functions.contains_key(&caller_id) {
            let inter_map = self.inter_functions.get(&caller_id).unwrap();
            return inter_map.keys().cloned().collect();
        }
        Vec::new()
    }

    // Still can be empty Vec!
    pub fn get_inter_functions_local_map(
        &self,
        caller_id: BodyId,
        callee_id: BodyId,
    ) -> Vec<FxHashMap<usize, usize>> {
        if self.inter_functions.contains_key(&caller_id) {
            let inter_map = self.inter_functions.get(&caller_id).unwrap();
            if inter_map.contains_key(&callee_id) {
                return (inter_map.get(&callee_id).unwrap().clone());
            }
        }
        Vec::new()
    }

    pub fn get_inter_functions_local_map_caller_idxs(
        &self,
        caller_id: BodyId,
        callee_id: BodyId,
        callee_idx: usize,
    ) -> Vec<usize> {
        let local_map = self.get_inter_functions_local_map(caller_id, callee_id);
        let mut caller_idxs = Vec::new();
        for map in local_map {
            if map.contains_key(&callee_idx) {
                caller_idxs.push(*map.get(&callee_idx).unwrap());
            }
        }
        caller_idxs
    }

    pub fn update_inter_functions(
        &mut self,
        caller_id: BodyId,
        callee_id: BodyId,
        local_map: FxHashMap<usize, usize>, // FxHashMap<callee, caller>, reversed
    ) {
        if !self.inter_functions.contains_key(&caller_id) {
            let mut inter_map = FxHashMap::default();
            inter_map.insert(callee_id, vec![local_map]);
            self.inter_functions.insert(caller_id, inter_map);
        } else {
            let mut inter_map = self.inter_functions.get_mut(&caller_id).unwrap();
            if !inter_map.contains_key(&callee_id) {
                inter_map.insert(callee_id, vec![local_map]);
            } else {
                let mut local_maps = inter_map.get_mut(&callee_id).unwrap();
                local_maps.push(local_map);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunctionAlias {
    pub args_alias: Vec<Vec<usize>>,
    pub return_alias: Vec<Vec<usize>>,
}

impl Default for FunctionAlias {
    fn default() -> Self {
        FunctionAlias {
            args_alias: Vec::default(),
            return_alias: Vec::default(),
        }
    }
}

impl FunctionAlias {
    pub fn new(args_alias: Vec<Vec<usize>>, return_alias: Vec<Vec<usize>>) -> Self {
        FunctionAlias {
            args_alias: args_alias,
            return_alias: return_alias,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.args_alias.is_empty() && self.return_alias.is_empty()
    }
}

// Put information related to function MIR into ir::Body, others related to SCC/spanning tree into graph::Graph
#[derive(Debug, Clone)]
pub struct Body<'tcx> {
    pub def_id: DefId,
    pub local_nodes: Vec<LocalNode<'tcx>>, // contains all local varibles (including fields) as nodes.
    pub basic_blocks: Vec<BasicBlock<'tcx>>, // contains all blocks in the CFG
    pub arg_size: usize,                   // argument size
                                           // pub local_decls: Vec<LocalDecl<'tcx>>,
                                           // pub original_decls: IndexVec<mir::Local, mir::LocalDecl<'tcx>>,
                                           // pub original: mir::Body<'tcx>,
}

// impl<'tcx> mir::HasLocalDecls<'tcx> for Body<'tcx> {
//     fn local_decls(&self) -> &LocalDecls<'tcx> {
//         &self.original_decls
//     }
// }

impl<'tcx> Body<'tcx> {
    // Scc/bb State utilities
    pub fn predecessors_of_bb_in_scc(&self, index: usize, scc_list: &[usize]) -> Vec<usize> {
        let mut result: Vec<usize> = Vec::new();
        for (group_idx, group) in self.map_successors_of_bb_in_scc(scc_list).iter() {
            if group.contains(&index) {
                result.push(*group_idx);
            }
        }
        result
    }

    pub fn successors_of_bb_in_scc(&self, index: usize, scc_list: &[usize]) -> Vec<usize> {
        self.basic_blocks[index]
            .terminator
            .original
            .successors()
            .map(|block| block.index())
            .filter(|&succ_index| scc_list.contains(&succ_index))
            .collect()
    }

    pub fn map_successors_of_bb_in_scc(&self, scc_list: &[usize]) -> FxHashMap<usize, Vec<usize>> {
        let mut map = FxHashMap::default();
        for scc_index in scc_list {
            let succs: Vec<usize> = self.basic_blocks[*scc_index]
                .terminator
                .original
                .successors()
                .map(|block| block.index())
                .filter(|&succ_index| scc_list.contains(&succ_index))
                .collect();
            map.insert(*scc_index, succs);
        }
        map
    }

    pub fn statements(&self) -> impl Iterator<Item = &Statement<'tcx>> {
        self.basic_blocks
            .iter()
            .flat_map(|block| block.statements.iter())
    }

    pub fn terminators(&self) -> impl Iterator<Item = &Terminator<'tcx>> {
        self.basic_blocks.iter().map(|block| &block.terminator)
    }

    // Retrieve the original information in mir::Body, retrieved by bb index
    pub fn get_mir_all_basicblock(&self, tcx: TyCtxt<'tcx>) -> Option<mir::BasicBlocks<'tcx>> {
        if tcx.is_mir_available(self.def_id)
            && matches!(
                tcx.hir().body_const_context(self.def_id.expect_local()),
                None | Some(ConstContext::ConstFn)
            )
        {
            Some(tcx.optimized_mir(self.def_id).basic_blocks.clone())
        } else {
            debug!(
                "Skipping an item {:?}, no MIR available for this item",
                self.def_id
            );
            None
        }
    }

    pub fn get_mir_basicblock_by_index(
        &self,
        tcx: TyCtxt<'tcx>,
        index: usize,
    ) -> Option<mir::BasicBlockData<'tcx>> {
        if tcx.is_mir_available(self.def_id)
            && matches!(
                tcx.hir().body_const_context(self.def_id.expect_local()),
                None | Some(ConstContext::ConstFn)
            )
        {
            let iter: mir::BasicBlock = mir::BasicBlock::from(index);
            Some(tcx.optimized_mir(self.def_id).basic_blocks[iter].clone())
        } else {
            debug!(
                "Skipping an item {:?}, no MIR available for this item",
                self.def_id
            );
            None
        }
    }

    pub fn get_mir_basicblock_data_by_index(
        &self,
        index: usize,
        tcx: TyCtxt<'tcx>,
    ) -> Option<mir::BasicBlockData<'tcx>> {
        if tcx.is_mir_available(self.def_id)
            && matches!(
                tcx.hir().body_const_context(self.def_id.expect_local()),
                None | Some(ConstContext::ConstFn)
            )
        {
            let iter: mir::BasicBlock = mir::BasicBlock::from(index);
            Some(tcx.optimized_mir(self.def_id).basic_blocks[iter].clone())
        } else {
            debug!(
                "Skipping an item {:?}, no MIR available for this item",
                self.def_id
            );
            None
        }
    }

    pub fn get_all_local_decls(&self, tcx: TyCtxt<'tcx>) -> Option<&'tcx LocalDecls<'tcx>> {
        if tcx.is_mir_available(self.def_id)
            && matches!(
                tcx.hir().body_const_context(self.def_id.expect_local()),
                None | Some(ConstContext::ConstFn)
            )
        {
            Some(tcx.optimized_mir(self.def_id).local_decls())
        } else {
            debug!(
                "Skipping an item {:?}, no MIR available for this item",
                self.def_id
            );
            // NotAvailable { def_id }.fail()
            None
        }
    }

    pub fn get_local_decls_by_index(
        &self,
        tcx: TyCtxt<'tcx>,
        index: usize,
    ) -> Option<&'tcx mir::LocalDecl<'tcx>> {
        if tcx.is_mir_available(self.def_id)
            && matches!(
                tcx.hir().body_const_context(self.def_id.expect_local()),
                None | Some(ConstContext::ConstFn)
            )
        {
            Some(&tcx.optimized_mir(self.def_id).local_decls()[mir::Local::from(index)])
        } else {
            debug!(
                "Skipping an item {:?}, no MIR available for this item",
                self.def_id
            );
            // NotAvailable { def_id }.fail()
            None
        }
    }
}
