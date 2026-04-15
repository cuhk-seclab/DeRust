use std::rc::Rc;

use rustc_data_structures::fx::FxHashMap;
use rustc_hir::{
    def_id::{DefId, LocalDefId},
    BodyId, ConstContext, HirId,
};
use rustc_middle::mir::{
    self, HasLocalDecls, Local, Operand, Place, Rvalue, StatementKind, TerminatorKind,
};
use rustc_middle::ty::{Ty, TyCtxt, TyKind};
use rustc_span::Span;

use dashmap::DashMap;
use snafu::Snafu;
use std::collections::HashMap;

// use crate::graph::{self, Graph, Scc};
use crate::graph::*;
use crate::ir::*;
use crate::ir::{self, BasicBlock, Body, LocalNode};
use crate::prelude::*;
use crate::report::ReportLevel;
use crate::visitor::{
    create_adt_impl_map, AdtImplMap, GenericBoundsMap, RelatedFnCollector, RelatedItemMap,
};

use crate::analysis::type_analysis::AdtOwner;
use crate::{Elapsed, RudraConfig};

#[derive(Debug, Snafu, Clone)]
pub enum MirInstantiationError {
    NotAvailable { def_id: DefId },
}

impl AnalysisError for MirInstantiationError {
    fn kind(&self) -> AnalysisErrorKind {
        use MirInstantiationError::*;
        match self {
            NotAvailable { .. } => AnalysisErrorKind::OutOfScope,
        }
    }
}

pub type RudraCtxt<'tcx> = &'tcx RudraCtxtOwner<'tcx>;
pub type TranslationResult<'tcx, T> = Result<T, MirInstantiationError>;

/// HIR visitor -> Maps Instance to MIR and cache the result.
#[derive(Clone)]
pub struct RudraCtxtOwner<'tcx> {
    tcx: TyCtxt<'tcx>,
    translation_cache: DashMap<DefId, Rc<TranslationResult<'tcx, ir::Body<'tcx>>>>,
    related_item_cache: RelatedItemMap,
    pub generic_bounds_cache: GenericBoundsMap<'tcx>,
    adt_impl_cache: AdtImplMap<'tcx>,
    report_level: ReportLevel,
    adt_owner: AdtOwner,
}

/// Visit MIR body and returns a Rudra IR function
/// Check rustc::mir::visit::Visitor for possible visit targets
/// https://doc.rust-lang.org/nightly/nightly-rustc/rustc/mir/visit/trait.Visitor.html
impl<'tcx> RudraCtxtOwner<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, report_level: ReportLevel) -> Self {
        let (hash_map, generic_bounds) = RelatedFnCollector::collect(tcx);

        RudraCtxtOwner {
            tcx,
            translation_cache: DashMap::new(),
            related_item_cache: hash_map,
            generic_bounds_cache: generic_bounds,
            adt_impl_cache: create_adt_impl_map(tcx),
            report_level,
            adt_owner: HashMap::default(),
        }
    }

    // Get, Set functions
    pub fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    pub fn adt_owner(&self) -> &AdtOwner {
        &self.adt_owner
    }

    pub fn adt_owner_mut(&mut self) -> &mut AdtOwner {
        &mut self.adt_owner
    }

    pub fn types_with_related_items(
        &self,
    ) -> impl Iterator<Item = (Option<HirId>, (BodyId, Span))> + '_ {
        (&self.related_item_cache)
            .into_iter()
            .flat_map(|(&k, v)| v.iter().map(move |&body_id| (k, body_id)))
    }

    pub fn get_generic_bounds_by_bodyid(
        &self,
        body_id: BodyId,
    ) -> FxHashMap<String, rustc_hir::GenericBounds<'tcx>> {
        self.generic_bounds_cache
            .get(&body_id)
            .cloned()
            .unwrap_or_else(FxHashMap::default)
    }

    pub fn translate_body(&self, def_id: DefId) -> Rc<TranslationResult<'tcx, ir::Body<'tcx>>> {
        let tcx = self.tcx();
        let result = self.translation_cache.entry(def_id).or_insert_with(|| {
            Rc::new(
                try {
                    let mir_body = Self::find_fn(tcx, def_id)?;
                    self.translate_body_impl(mir_body, tcx, def_id)?
                },
            )
        });

        result.clone()
    }

    fn translate_body_impl(
        &self,
        body: &mir::Body<'tcx>,
        tcx: TyCtxt<'tcx>,
        def_id: DefId,
    ) -> TranslationResult<'tcx, ir::Body<'tcx>> {
        // let local_decls = body
        //     .local_decls
        //     .iter()
        //     .map(|local_decl| self.translate_local_decl(local_decl))
        //     .collect::<Vec<_>>();

        // let basic_blocks: Vec<_> = body
        //     .basic_blocks
        //     .iter()
        //     .map(|basic_block| self.translate_basic_block(basic_block, body))
        //     .collect::<Result<Vec<_>, _>>()?;

        // Handle local_nodes
        let arg_size = body.arg_count;
        let locals_decls = &body.local_decls;
        let mut local_nodes = Vec::<LocalNode<'tcx>>::new();
        let param_env = tcx.param_env(def_id);
        for ld in 0..locals_decls.len() {
            let index = Local::from(ld);
            let local_ty = locals_decls[index].ty;
            let kind = type_kind_classify(local_ty);
            let mut local_node = LocalNode::new(ld, ld, local_ty, kind);
            local_nodes.push(local_node);
        }

        // Handle basic_blocks
        let basicblocks = &body.basic_blocks;
        let mut basic_blocks = Vec::<BasicBlock<'tcx>>::new();
        for i in 0..basicblocks.len() {
            basic_blocks.push(self.translate_basic_block(
                i,
                basicblocks,
                &mut local_nodes,
                body,
            )?);
        }

        Ok(ir::Body {
            def_id: def_id,
            local_nodes: local_nodes,
            basic_blocks: basic_blocks,
            arg_size: arg_size,
        })
    }

    fn translate_basic_block(
        &self,
        index: usize,
        basic_blocks: &mir::BasicBlocks<'tcx>,
        local_nodes: &mut Vec<LocalNode<'tcx>>,
        body: &mir::Body<'tcx>,
    ) -> TranslationResult<'tcx, ir::BasicBlock<'tcx>> {
        // let statements = basic_block
        //     .statements
        //     .iter()
        //     .map(|statement| statement.clone())
        //     .collect::<Vec<_>>();

        // let statements = basic_block
        //     .statements
        //     .iter()
        //     .map(|statement| {
        //         self.translate_conversion_statement(statement, body)
        //             .unwrap()
        //     })
        //     .collect::<Vec<_>>();

        // let terminator = self.translate_terminator(
        //     basic_block
        //         .terminator
        //         .as_ref()
        //         .expect("Terminator should not be empty at this point"),
        // )?;

        let iter: mir::BasicBlock = mir::BasicBlock::from(index);
        let basic_block = &basic_blocks[iter];
        let is_cleanup = basic_block.is_cleanup;

        // Handle statements
        let mut statements = Vec::<ir::Statement<'tcx>>::new();
        for statement in &basic_block.statements {
            statements.push(self.translate_statement(statement, local_nodes, body)?)
        }

        // Handle terminator
        let mir_terminator = &basic_block.terminator.clone().unwrap();
        let ir_terminator = self.translate_terminator(mir_terminator)?;

        Ok(ir::BasicBlock {
            index: index,
            statements: statements,
            terminator: ir_terminator,
            is_cleanup: is_cleanup,
        })
    }

    fn translate_statement(
        &self,
        statement: &mir::Statement<'tcx>,
        local_nodes: &mut Vec<LocalNode<'tcx>>,
        body: &mir::Body<'tcx>,
    ) -> TranslationResult<'tcx, ir::Statement<'tcx>> {
        Ok(ir::Statement {
            kind: match &statement.kind {
                StatementKind::Assign(box (lplace, rvalue)) => {
                    let lvalue_has_projection = self.has_projection(lplace);
                    match rvalue {
                        // Rvalue unhandled: Repeat, ThreadLocalRef, Len, BinaryOp, CheckedBinaryOp, NullaryOp, UnaryOp, Aggregate, ShallowInitBox
                        Rvalue::Use(operand) => match operand {
                            Operand::Copy(rplace) => {
                                let mut stmt_kind: usize = 1;
                                let rvalue_has_projection = self.has_projection(rplace);
                                let llocal_index = lplace.local.as_usize();
                                let llocal_ty = if lvalue_has_projection {
                                    if self.has_deref_projection(lplace) {
                                        stmt_kind = 0;
                                    }
                                    lplace.ty(body.local_decls(), self.tcx()).ty
                                } else {
                                    local_nodes[llocal_index].ty
                                };
                                let rlocal_index = rplace.local.as_usize();
                                let rlocal_ty = if rvalue_has_projection {
                                    if self.has_deref_projection(rplace) {
                                        stmt_kind = 0;
                                    }
                                    rplace.ty(body.local_decls(), self.tcx()).ty
                                } else {
                                    local_nodes[rlocal_index].ty
                                };
                                ir::StatementKind::Assign {
                                    lplace: lplace.clone(),
                                    rplace: rplace.clone(),
                                    kind: stmt_kind,
                                    ltype: llocal_ty,
                                    rtype: rlocal_ty,
                                    castkind: None,
                                }
                            }
                            Operand::Move(rplace) => {
                                let mut stmt_kind: usize = 1;
                                let rvalue_has_projection = self.has_projection(rplace);
                                let llocal_index = lplace.local.as_usize();
                                let llocal_ty = if lvalue_has_projection {
                                    if self.has_deref_projection(lplace) {
                                        stmt_kind = 0;
                                    }
                                    lplace.ty(body.local_decls(), self.tcx()).ty
                                } else {
                                    local_nodes[llocal_index].ty
                                };
                                let rlocal_index = rplace.local.as_usize();
                                let rlocal_ty = if rvalue_has_projection {
                                    if self.has_deref_projection(rplace) {
                                        stmt_kind = 0;
                                    }
                                    rplace.ty(body.local_decls(), self.tcx()).ty
                                } else {
                                    local_nodes[rlocal_index].ty
                                };
                                ir::StatementKind::Assign {
                                    lplace: lplace.clone(),
                                    rplace: rplace.clone(),
                                    kind: stmt_kind,
                                    ltype: llocal_ty,
                                    rtype: rlocal_ty,
                                    castkind: None,
                                }
                            }
                            _ => ir::StatementKind::Unimplemented(
                                format!("Unimplemented constant statement: {:?}", statement.kind)
                                    .into(),
                            ),
                        },
                        Rvalue::Ref(_region, _borrowkind, rplace) => {
                            // Ignore &(*_x) deref case; Deref values should be passed to funcs...
                            let rvalue_has_projection = self.has_projection(rplace);
                            let llocal_index = lplace.local.as_usize();
                            let llocal_ty = if lvalue_has_projection {
                                lplace.ty(body.local_decls(), self.tcx()).ty
                            } else {
                                local_nodes[llocal_index].ty
                            };
                            let rlocal_index = rplace.local.as_usize();
                            let rlocal_ty = if rvalue_has_projection {
                                rplace.ty(body.local_decls(), self.tcx()).ty
                            } else {
                                local_nodes[rlocal_index].ty
                            };
                            ir::StatementKind::Assign {
                                lplace: lplace.clone(),
                                rplace: rplace.clone(),
                                kind: 1,
                                ltype: llocal_ty,
                                rtype: rlocal_ty,
                                castkind: None,
                            }
                        }
                        Rvalue::AddressOf(_mutability, rplace) => {
                            // Ignore &raw const (*_x), etc. deref case
                            let rvalue_has_projection = self.has_projection(rplace);
                            let llocal_index = lplace.local.as_usize();
                            let llocal_ty = if lvalue_has_projection {
                                lplace.ty(body.local_decls(), self.tcx()).ty
                            } else {
                                local_nodes[llocal_index].ty
                            };
                            let rlocal_index = rplace.local.as_usize();
                            let rlocal_ty = if rvalue_has_projection {
                                rplace.ty(body.local_decls(), self.tcx()).ty
                            } else {
                                local_nodes[rlocal_index].ty
                            };
                            ir::StatementKind::Assign {
                                lplace: lplace.clone(),
                                rplace: rplace.clone(),
                                kind: 2,
                                ltype: llocal_ty,
                                rtype: rlocal_ty,
                                castkind: None,
                            }
                        }
                        Rvalue::Cast(castkind, operand, dst_ty) => match operand {
                            Operand::Copy(rplace) => {
                                let rvalue_has_projection = self.has_projection(rplace);
                                let llocal_index = lplace.local.as_usize();
                                let llocal_ty = if lvalue_has_projection {
                                    lplace.ty(body.local_decls(), self.tcx()).ty
                                } else {
                                    local_nodes[llocal_index].ty
                                };
                                let rlocal_index = rplace.local.as_usize();
                                let rlocal_ty = if rvalue_has_projection {
                                    rplace.ty(body.local_decls(), self.tcx()).ty
                                } else {
                                    local_nodes[rlocal_index].ty
                                };
                                ir::StatementKind::Assign {
                                    lplace: lplace.clone(),
                                    rplace: rplace.clone(),
                                    kind: 3,
                                    ltype: llocal_ty,
                                    rtype: rlocal_ty,
                                    castkind: Some(castkind.clone()),
                                }
                            }
                            Operand::Move(rplace) => {
                                let rvalue_has_projection = self.has_projection(rplace);
                                let llocal_index = lplace.local.as_usize();
                                let llocal_ty = if lvalue_has_projection {
                                    lplace.ty(body.local_decls(), self.tcx()).ty
                                } else {
                                    local_nodes[llocal_index].ty
                                };
                                let rlocal_index = rplace.local.as_usize();
                                let rlocal_ty = if rvalue_has_projection {
                                    rplace.ty(body.local_decls(), self.tcx()).ty
                                } else {
                                    local_nodes[rlocal_index].ty
                                };
                                ir::StatementKind::Assign {
                                    lplace: lplace.clone(),
                                    rplace: rplace.clone(),
                                    kind: 3,
                                    ltype: llocal_ty,
                                    rtype: rlocal_ty,
                                    castkind: Some(castkind.clone()),
                                }
                            }
                            _ => ir::StatementKind::Unimplemented(
                                format!("Unimplemented constant statement: {:?}", statement.kind)
                                    .into(),
                            ),
                        },
                        // Rvalue::Aggregate(_aggregatekind, ref operands) => {},   // To handle for loop in Aggregate
                        // Handle Discriminant assignment
                        Rvalue::Discriminant(ref rplace) => {
                            let mut stmt_kind: usize = 1;
                            let rvalue_has_projection = self.has_projection(rplace);
                            let llocal_index = lplace.local.as_usize();
                            let llocal_ty = if lvalue_has_projection {
                                if self.has_deref_projection(lplace) {
                                    stmt_kind = 0;
                                }
                                lplace.ty(body.local_decls(), self.tcx()).ty
                            } else {
                                local_nodes[llocal_index].ty
                            };
                            let rlocal_index = rplace.local.as_usize();
                            let rlocal_ty = if rvalue_has_projection {
                                if self.has_deref_projection(rplace) {
                                    stmt_kind = 0;
                                }
                                rplace.ty(body.local_decls(), self.tcx()).ty
                            } else {
                                local_nodes[rlocal_index].ty
                            };
                            ir::StatementKind::Assign {
                                lplace: lplace.clone(),
                                rplace: rplace.clone(),
                                kind: stmt_kind,
                                ltype: llocal_ty,
                                rtype: rlocal_ty,
                                castkind: None,
                            }
                        }
                        Rvalue::CopyForDeref(ref rplace) => {
                            let mut stmt_kind: usize = 1;
                            let rvalue_has_projection = self.has_projection(rplace);
                            let llocal_index = lplace.local.as_usize();
                            let llocal_ty = if lvalue_has_projection {
                                if self.has_deref_projection(lplace) {
                                    stmt_kind = 0;
                                }
                                lplace.ty(body.local_decls(), self.tcx()).ty
                            } else {
                                local_nodes[llocal_index].ty
                            };
                            let rlocal_index = rplace.local.as_usize();
                            let rlocal_ty = if rvalue_has_projection {
                                if self.has_deref_projection(rplace) {
                                    stmt_kind = 0;
                                }
                                rplace.ty(body.local_decls(), self.tcx()).ty
                            } else {
                                local_nodes[rlocal_index].ty
                            };
                            ir::StatementKind::Assign {
                                lplace: lplace.clone(),
                                rplace: rplace.clone(),
                                kind: stmt_kind,
                                ltype: llocal_ty,
                                rtype: rlocal_ty,
                                castkind: None,
                            }
                        }
                        _ => ir::StatementKind::Unimplemented(
                            format!("Unimplemented statement: {:?}", statement.kind).into(),
                        ),
                    }
                }
                StatementKind::SetDiscriminant {
                    place,
                    variant_index,
                } => ir::StatementKind::SetDiscriminant {
                    place: *place.clone(),
                    variant_index: variant_index.index(),
                },
                StatementKind::StorageLive(local) => ir::StatementKind::StorageLive(*local),
                StatementKind::StorageDead(local) => ir::StatementKind::StorageDead(*local),
                _ => ir::StatementKind::Unimplemented(
                    format!("Unimplemented statement: {:?}", statement.kind).into(),
                ),
            },
            original: statement.clone(),
        })
    }

    fn translate_terminator(
        &self,
        terminator: &mir::Terminator<'tcx>,
    ) -> TranslationResult<'tcx, ir::Terminator<'tcx>> {
        Ok(ir::Terminator {
            kind: match &terminator.kind {
                // TerminatorKind::Goto { target } => ir::TerminatorKind::Goto(target.index()),
                TerminatorKind::Return => ir::TerminatorKind::Return,
                TerminatorKind::SwitchInt { discr, targets } => ir::TerminatorKind::SwitchInt {
                    discr: discr.clone(),
                    targets: targets.clone(),
                },
                TerminatorKind::Drop { .. } => ir::TerminatorKind::Drop,
                TerminatorKind::Call {
                    func: func_operand,
                    args,
                    destination,
                    target,
                    unwind: _,
                    call_source: _,
                    fn_span: _,
                } => {
                    // let cleanup = cleanup.clone().map(|block| block.index());
                    // ToDO: target should be discarded
                    let destination = if let Some(target_bb) = target {
                        Some((destination.clone(), target_bb.index()))
                    } else {
                        Some((destination.clone(), 0usize))
                    };

                    if let mir::Operand::Constant(box constoperand) = func_operand {
                        let func_ty = constoperand.ty();
                        match func_ty.kind() {
                            TyKind::FnDef(def_id, callee_substs) => {
                                ir::TerminatorKind::StaticCall {
                                    callee_did: def_id.clone(),
                                    callee_substs,
                                    func: func_operand.clone(),
                                    args: args.clone(),
                                    destination,
                                    // cleanup,
                                }
                            }
                            TyKind::FnPtr(_) => ir::TerminatorKind::FnPtr {
                                value: constoperand.const_.clone(),
                            },
                            _ => panic!("invalid callee of type {:?}", func_ty),
                        }
                    } else {
                        ir::TerminatorKind::Unimplemented("Non-constant function call".into())
                    }
                }
                _ => ir::TerminatorKind::Unimplemented(
                    format!("Unimplemented unknown terminator: {:?}", terminator).into(),
                ),
            },
            original: terminator.clone(),
        })
    }

    /*
    // FLASH: Other statements such as dereference, etc.
    fn translate_conversion_statement(
        &self,
        statement: &mir::Statement<'tcx>,
        body: &mir::Body<'tcx>,
    ) -> TranslationResult<'tcx, ir::Statement<'tcx>> {
        Ok(ir::Statement {
            kind: match &statement.kind {
                StatementKind::Assign(box (lplace, rvalue)) => {
                    let lvalue_has_projection = self.has_projection(lplace);
                    match rvalue {
                        Rvalue::Use(operand) => match operand {
                            Operand::Copy(rplace) => {
                                let rvalue_has_projection = self.has_projection(rplace);
                                match (lvalue_has_projection, rvalue_has_projection) {
                                    (true, true)
                                    | (true, false)
                                    | (false, true)
                                    | (false, false) => ir::StatementKind::Unrelated(
                                        format!("Unrelated type statement: {:?}", statement.kind)
                                            .into(),
                                    ),
                                }
                            }
                            Operand::Move(rplace) => {
                                let rvalue_has_projection = self.has_projection(rplace);
                                match (lvalue_has_projection, rvalue_has_projection) {
                                    (true, true)
                                    | (true, false)
                                    | (false, true)
                                    | (false, false) => ir::StatementKind::Unrelated(
                                        format!("Unrelated type statement: {:?}", statement.kind)
                                            .into(),
                                    ),
                                }
                            }
                            _ => ir::StatementKind::Unimplemented(
                                format!("Unimplemented statement: {:?}", statement.kind).into(),
                            ),
                        },
                        Rvalue::Ref(_region, _borrowkind, rplace) => {
                            let rvalue_has_projection = self.has_projection(rplace);
                            match (lvalue_has_projection, rvalue_has_projection) {
                                (true, true) | (true, false) | (false, true) | (false, false) => {
                                    ir::StatementKind::Unrelated(
                                        format!("Unrelated type statement: {:?}", statement.kind)
                                            .into(),
                                    )
                                }
                            }
                        }
                        Rvalue::AddressOf(_mutability, rplace) => {
                            let rvalue_has_projection = self.has_projection(rplace);
                            match (lvalue_has_projection, rvalue_has_projection) {
                                // FLASH: ToDO
                                (true, true) | (true, false) | (false, true) => {
                                    // let lproj_ty = lplace.ty(body.local_decls(), self.tcx()).ty;
                                    // let rproj_ty = rplace.ty(body.local_decls(), self.tcx()).ty;
                                    ir::StatementKind::Unimplemented(
                                        format!(
                                            "Unimplemented todo statement: {:?}",
                                            statement.kind
                                        )
                                        .into(),
                                    )
                                }
                                (false, false) => {
                                    let llocal = lplace.local;
                                    let llocal_decl = body.local_decls[llocal].clone();
                                    let llocal_ty = llocal_decl.ty;
                                    let rlocal = rplace.local;
                                    let rlocal_decl = body.local_decls[rlocal].clone();
                                    let rlocal_ty = rlocal_decl.ty;

                                    // let llocal_ty = lplace.ty(body.local_decls(), self.tcx()).ty;
                                    // let rlocal_ty = rplace.ty(body.local_decls(), self.tcx()).ty;
                                    // let llocal_index = llocal.as_usize();
                                    // let rlocal_index = rlocal.as_usize();
                                    // println!("@@@@@===FLASH===@@@@@");
                                    // println!("llocal_ty: {:?}", llocal_ty);
                                    // println!("rlocal_ty: {:?}", rlocal_ty);
                                    ir::StatementKind::Assign {
                                        lplace: lplace.clone(),
                                        rvalue: rvalue.clone(),
                                        ltype: llocal_ty,
                                        rtype: rlocal_ty,
                                        castkind: None,
                                    }
                                }
                            }
                        }
                        Rvalue::Cast(castkind, operand, dst_ty) => match operand {
                            Operand::Copy(rplace) => {
                                let rvalue_has_projection = self.has_projection(rplace);
                                match (lvalue_has_projection, rvalue_has_projection) {
                                    // FLASH: ToDO
                                    (true, true) | (true, false) | (false, true) => {
                                        ir::StatementKind::Unimplemented(
                                            format!(
                                                "Unimplemented todo statement: {:?}",
                                                statement.kind
                                            )
                                            .into(),
                                        )
                                    }
                                    (false, false) => {
                                        let llocal = lplace.local;
                                        let llocal_decl = body.local_decls[llocal].clone();
                                        let llocal_ty = llocal_decl.ty;
                                        let rlocal = rplace.local;
                                        let rlocal_decl = body.local_decls[rlocal].clone();
                                        let rlocal_ty = rlocal_decl.ty;
                                        // println!("@@@@@===FLASH===@@@@@");
                                        // println!("llocal_ty: {:?}", llocal_ty);
                                        // println!("dst_ty: {:?}", dst_ty);
                                        // println!("rlocal_ty: {:?}", rlocal_ty);
                                        // println!("castkind: {:?}", castkind);
                                        ir::StatementKind::Assign {
                                            lplace: lplace.clone(),
                                            rvalue: rvalue.clone(),
                                            ltype: llocal_ty,
                                            rtype: rlocal_ty,
                                            castkind: Some(castkind.clone()),
                                        }
                                    }
                                }
                            }
                            Operand::Move(rplace) => {
                                let rvalue_has_projection = self.has_projection(rplace);
                                match (lvalue_has_projection, rvalue_has_projection) {
                                    // FLASH: ToDO
                                    (true, true) | (true, false) | (false, true) => {
                                        ir::StatementKind::Unimplemented(
                                            format!(
                                                "Unimplemented todo statement: {:?}",
                                                statement.kind
                                            )
                                            .into(),
                                        )
                                    }
                                    (false, false) => {
                                        let llocal = lplace.local;
                                        let llocal_decl = body.local_decls[llocal].clone();
                                        let llocal_ty = llocal_decl.ty;
                                        let rlocal = rplace.local;
                                        let rlocal_decl = body.local_decls[rlocal].clone();
                                        let rlocal_ty = rlocal_decl.ty;
                                        // println!("@@@@@===FLASH===@@@@@");
                                        // println!("llocal_ty: {:?}", llocal_ty);
                                        // println!("dst_ty: {:?}", dst_ty);
                                        // println!("rlocal_ty: {:?}", rlocal_ty);
                                        // println!("castkind: {:?}", castkind);
                                        ir::StatementKind::Assign {
                                            lplace: lplace.clone(),
                                            rvalue: rvalue.clone(),
                                            ltype: llocal_ty,
                                            rtype: rlocal_ty,
                                            castkind: Some(castkind.clone()),
                                        }
                                    }
                                }
                            }
                            _ => ir::StatementKind::Unimplemented(
                                format!("Unimplemented statement: {:?}", statement.kind).into(),
                            ),
                        },
                        _ => ir::StatementKind::Unimplemented(
                            format!("Unimplemented statement: {:?}", statement.kind).into(),
                        ),
                    }
                }
                StatementKind::SetDiscriminant {
                    place,
                    variant_index,
                } => ir::StatementKind::SetDiscriminant {
                    place: *place.clone(),
                    variant_index: variant_index.index(),
                },
                StatementKind::StorageLive(local) => ir::StatementKind::StorageLive(*local),
                StatementKind::StorageDead(local) => ir::StatementKind::StorageDead(*local),
                _ => ir::StatementKind::Unimplemented(
                    format!("Unimplemented statement: {:?}", statement.kind).into(),
                ),
            },
            original: statement.clone(),
        })
    }
     */

    /*
    fn translate_terminator(
        &self,
        terminator: &mir::Terminator<'tcx>,
    ) -> TranslationResult<'tcx, ir::Terminator<'tcx>> {
        Ok(ir::Terminator {
            kind: match &terminator.kind {
                TerminatorKind::Goto { target } => ir::TerminatorKind::Goto(target.index()),
                TerminatorKind::Return => ir::TerminatorKind::Return,
                TerminatorKind::Call {
                    func: func_operand,
                    args,
                    destination,
                    target,
                    ..
                } => {
                    // let cleanup = cleanup.clone().map(|block| block.index());
                    let destination = Some((destination.clone(), target.unwrap().index()));

                    // FLASH： ToDO. Constant or Copy/Move
                    if let mir::Operand::Constant(box func) = func_operand {
                        let func_ty = func.ty();
                        match func_ty.kind() {
                            TyKind::FnDef(def_id, callee_substs) => {
                                ir::TerminatorKind::StaticCall {
                                    callee_did: def_id.clone(),
                                    callee_substs,
                                    args: args.clone(),
                                    // cleanup,
                                    destination,
                                }
                            }
                            TyKind::FnPtr(_) => ir::TerminatorKind::FnPtr {
                                value: func.const_.clone(),
                            },
                            _ => panic!("invalid callee of type {:?}", func_ty),
                        }
                    } else {
                        ir::TerminatorKind::Unimplemented("non-constant function call".into())
                    }
                }
                TerminatorKind::Drop { .. } => {
                    // FLASH: ToDO: implement Drop and DropAndReplace terminators
                    ir::TerminatorKind::Unimplemented(
                        format!("ToDO terminator: {:?}", terminator).into(),
                    )
                }
                _ => ir::TerminatorKind::Unimplemented(
                    format!("Unknown terminator: {:?}", terminator).into(),
                ),
            },
            original: terminator.clone(),
        })
    }
     */

    fn translate_local_decl(&self, local_decl: &mir::LocalDecl<'tcx>) -> ir::LocalDecl<'tcx> {
        ir::LocalDecl { ty: local_decl.ty }
    }

    /// Try to find MIR function body with def_id.
    fn find_fn(
        tcx: TyCtxt<'tcx>,
        def_id: DefId,
    ) -> Result<&'tcx mir::Body<'tcx>, MirInstantiationError> {
        if tcx.is_mir_available(def_id)
            && matches!(
                tcx.hir().body_const_context(def_id.expect_local()),
                None | Some(ConstContext::ConstFn)
            )
        {
            Ok(tcx.optimized_mir(def_id))
        } else {
            debug!(
                "Skipping an item {:?}, no MIR available for this item",
                def_id
            );
            NotAvailable { def_id }.fail()
        }
    }

    pub fn index_adt_cache(&self, adt_did: &DefId) -> Option<&Vec<(LocalDefId, Ty<'tcx>)>> {
        self.adt_impl_cache.get(adt_did)
    }

    pub fn report_level(&self) -> ReportLevel {
        self.report_level
    }

    fn has_projection(&self, place: &Place) -> bool {
        return if place.projection.len() > 0 {
            true
        } else {
            false
        };
    }

    pub fn has_deref_projection(&self, place: &Place) -> bool {
        place
            .projection
            .iter()
            .any(|elem| matches!(elem, mir::ProjectionElem::Deref))
    }

}

pub trait Rcx<'tcx, 'o, 'a> {
    fn rcx(&'o self) -> &'o RudraCtxtOwner<'tcx>;

    fn tcx(&'o self) -> TyCtxt<'tcx>;
}

pub trait RcxMut<'tcx, 'o, 'a> {
    fn rcx(&'o self) -> &'o RudraCtxtOwner<'tcx>;

    fn rcx_mut(&'o mut self) -> &'o mut RudraCtxtOwner<'tcx>;

    fn tcx(&'o self) -> TyCtxt<'tcx>;
}
