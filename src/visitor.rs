use rustc_data_structures::fx::FxHashMap;
use rustc_hir::{
    def_id::{DefId, LocalDefId},
    intravisit, Block, BodyId, HirId, Impl, ItemKind,
};
use rustc_middle::hir::nested_filter::OnlyBodies;
use rustc_middle::ty::{Ty, TyCtxt, TyKind};
use rustc_span::Span;

// HIR visitor and collector

/// Maps `HirId` of a type to `BodyId` of related impls. One HirId with multiple BodyIds
/// Free-standing (top level) functions and default trait impls have `None` as a key.
pub type RelatedItemMap = FxHashMap<Option<HirId>, Vec<(BodyId, Span)>>;

// pub type GenericBoundsMap<'tcx> = FxHashMap<BodyId, FxHashMap<DefId, rustc_hir::GenericBounds<'tcx>>>;
pub type GenericBoundsMap<'tcx> =
    FxHashMap<BodyId, FxHashMap<String, rustc_hir::GenericBounds<'tcx>>>;

/// Creates `AdtItemMap` with the given HIR map.
/// You might want to use `RudraCtxt`'s `related_item_cache` field instead of
/// directly using this collector.
pub struct RelatedFnCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    hash_map: RelatedItemMap,
    generic_bounds: GenericBoundsMap<'tcx>,
}

impl<'tcx> RelatedFnCollector<'tcx> {
    pub fn collect(tcx: TyCtxt<'tcx>) -> (RelatedItemMap, GenericBoundsMap<'tcx>) {
        let mut collector = RelatedFnCollector {
            tcx,
            hash_map: RelatedItemMap::default(),
            generic_bounds: FxHashMap::default(),
        };

        // tcx.hir().krate().visit_all_item_likes(&mut collector);
        tcx.hir().visit_all_item_likes_in_crate(&mut collector); // item -> trait_item -> impl_item -> foreign_item. Not nested items

        (collector.hash_map, collector.generic_bounds)
    }
}

impl<'tcx> intravisit::Visitor<'tcx> for RelatedFnCollector<'tcx> {
    type NestedFilter = OnlyBodies;

    fn nested_visit_map(&mut self) -> Self::Map {
        self.tcx.hir()
    }

    fn visit_item(&mut self, item: &'tcx rustc_hir::Item<'tcx>) {
        let hir_map = self.tcx.hir();

        match &item.kind {
            // Utilize:
            //          Unsafety, self_ty, items
            ItemKind::Impl(Impl {
                unsafety: _unsafety,
                generics: _generics, // ToDO: utilize generics functions: is_impl_trait...
                self_ty,
                items: impl_items,
                ..
            }) => {
                let key = Some(self_ty.hir_id);
                let entry = self.hash_map.entry(key).or_insert(Vec::new());
                entry.extend(impl_items.iter().filter_map(|impl_item_ref| {
                    // let hir_id = impl_item_ref.id.hir_id();
                    let local_def_id = impl_item_ref.id.owner_id.def_id;
                    hir_map
                        .maybe_body_owned_by(local_def_id)
                        .map(|body_id| (body_id, impl_item_ref.span))
                }));

                // ToDO: whether the generics is enough?
                for item_item_ref in impl_items.iter() {
                    if let Some(rustc_hir::Node::ImplItem(rustc_hir::ImplItem {
                        kind: rustc_hir::ImplItemKind::Fn(fn_sig, body_id),
                        generics,
                        ..
                    })) = hir_map.find(item_item_ref.id.hir_id())
                    {
                        let generic_entry = self
                            .generic_bounds
                            .entry(*body_id)
                            .or_insert(FxHashMap::default());
                        generic_entry.extend(get_bounds_from_generics(generics));
                    }
                }
            }
            // Free-standing (top level) functions and default trait impls have `None` as a key.
            // Utilize: Entire ItemKind::Trait object is already a trait bound
            //          Unsafety
            //          Generics, GenericBounds -> They're related to trait object. Not usual
            //          [TraitItemRef] -> Find trait's functions
            ItemKind::Trait(_is_auto, _unsafety, _generics, _generic_bounds, trait_items) => {
                let key = None;
                let entry = self.hash_map.entry(key).or_insert(Vec::new());
                entry.extend(trait_items.iter().filter_map(|trait_item_ref| {
                    // let hir_id = trait_item_ref.id.hir_id();
                    let local_def_id = trait_item_ref.id.owner_id.def_id;
                    hir_map
                        .maybe_body_owned_by(local_def_id)
                        .map(|body_id| (body_id, trait_item_ref.span))
                }));

                // ToDO: could extend
            }
            // Utilize: find functions' generic with its trait bounds
            //          FnSig
            //              -> FnHeader -> unsafety, abi;
            //              -> FnDecl -> inputs, output -> Ty; implicit_self (&self, &mut self, self, mut self)
            //          Generics
            //              ->
            ItemKind::Fn(_fn_sig, generics, body_id) => {
                let key = None;
                let entry = self.hash_map.entry(key).or_insert(Vec::new());
                entry.push((*body_id, item.span));

                let generic_entry = self
                    .generic_bounds
                    .entry(*body_id)
                    .or_insert(FxHashMap::default());
                generic_entry.extend(get_bounds_from_generics(generics));
            }
            _ => (),
        }
    }

    fn visit_trait_item(&mut self, _trait_item: &'tcx rustc_hir::TraitItem<'tcx>) {
        // We don't process items inside trait blocks
    }

    fn visit_impl_item(&mut self, _impl_item: &'tcx rustc_hir::ImplItem<'tcx>) {
        // We don't process items inside impl blocks
    }

    fn visit_foreign_item(&mut self, _foreign_item: &'tcx rustc_hir::ForeignItem<'tcx>) {
        // We don't process foreign items
    }
}

// Only analyze GenericParamKind Type
pub fn get_bounds_from_generics<'a, 'tcx>(
    generics: &'a rustc_hir::Generics<'tcx>,
    // ) -> FxHashMap<DefId, rustc_hir::GenericBounds<'tcx>> {
) -> FxHashMap<String, rustc_hir::GenericBounds<'tcx>> {
    let mut bound_map: FxHashMap<DefId, rustc_hir::GenericBounds<'tcx>> = FxHashMap::default();
    let mut name_bound_map: FxHashMap<String, rustc_hir::GenericBounds<'tcx>> =
        FxHashMap::default();

    for predicate in generics.predicates {
        if let rustc_hir::WherePredicate::BoundPredicate(rustc_hir::WhereBoundPredicate {
            bounded_ty,
            bounds,
            ..
        }) = predicate
        {
            if let (Some(def_id), _) = get_defid_args_from_kind(&bounded_ty.kind) {
                bound_map.insert(def_id, bounds);
            }
        }
    }

    // Recheck
    for param in generics.params {
        if let rustc_hir::GenericParamKind::Type { .. } = param.kind {
            if !bound_map.contains_key(&param.def_id.to_def_id()) {
                bound_map.insert(param.def_id.to_def_id(), &[]);
            }
        }
    }

    // Convert def_id to name
    for param in generics.params {
        if let rustc_hir::GenericParamKind::Type { .. } = param.kind {
            if bound_map.contains_key(&param.def_id.to_def_id()) {
                let bounds = bound_map.get(&param.def_id.to_def_id()).unwrap();
                if let rustc_hir::ParamName::Plain(ident) = param.name {
                    name_bound_map.insert(ident.name.to_string(), *bounds);
                }
            }
        }
    }
    // bound_map
    name_bound_map
}

pub fn get_defid_args_from_kind<'a, 'tcx>(
    kind: &'a rustc_hir::TyKind<'tcx>,
) -> (Option<DefId>, Vec<&'a rustc_hir::GenericArg<'tcx>>) {
    let mut ret_def_id: Option<DefId> = None;
    let mut ret_args: Vec<&rustc_hir::GenericArg> = Vec::new();

    if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(
        _,
        rustc_hir::Path { res, segments, .. },
    )) = kind
    {
        match res {
            rustc_hir::def::Res::Def(_, def_id) => {
                ret_def_id = Some(*def_id);
            }
            rustc_hir::def::Res::SelfTyAlias {
                alias_to: def_id, ..
            } => {
                Some(*def_id);
            }
            _ => (),
        }

        if let Some(rustc_hir::PathSegment {
            args: Some(rustc_hir::GenericArgs { args, .. }),
            ..
        }) = segments.last()
        {
            for arg in &(**args) {
                ret_args.push(arg);
            }
        }
    }

    (ret_def_id, ret_args)
}

pub struct ContainsUnsafe<'tcx> {
    tcx: TyCtxt<'tcx>,
    contains_unsafe: bool,
}

impl<'tcx> ContainsUnsafe<'tcx> {
    /// Given a `BodyId`, returns if the corresponding body contains unsafe code in it.
    /// Note that it only checks the function body, so this function will return false for
    /// body ids of functions that are defined as unsafe.
    pub fn contains_unsafe(tcx: TyCtxt<'tcx>, body_id: BodyId) -> bool {
        use intravisit::Visitor;

        let mut visitor = ContainsUnsafe {
            tcx,
            contains_unsafe: false,
        };

        let body = visitor.tcx.hir().body(body_id);
        visitor.visit_body(body);

        visitor.contains_unsafe
    }
}

impl<'tcx> intravisit::Visitor<'tcx> for ContainsUnsafe<'tcx> {
    type NestedFilter = OnlyBodies;

    fn nested_visit_map(&mut self) -> Self::Map {
        self.tcx.hir()
    }

    fn visit_block(&mut self, block: &'tcx Block<'tcx>) {
        use rustc_hir::BlockCheckMode;
        if let BlockCheckMode::UnsafeBlock(_unsafe_source) = block.rules {
            self.contains_unsafe = true;
        }
        intravisit::walk_block(self, block);
    }
}

pub struct ContainsUnsafeFnHeader<'tcx> {
    tcx: TyCtxt<'tcx>,
    body_id: BodyId,
    contains_unsafe_fnheader: bool,
}

impl<'tcx> ContainsUnsafeFnHeader<'tcx> {
    /// Given a `BodyId`, returns if the corresponding body contains unsafe function header.
    pub fn contains_unsafe_fnheader(tcx: TyCtxt<'tcx>, body_id: BodyId) -> bool {
        use intravisit::Visitor;

        let mut visitor = ContainsUnsafeFnHeader {
            tcx,
            body_id,
            contains_unsafe_fnheader: false,
        };

        visitor.contains_unsafe_fnheader
    }
}

impl<'tcx> intravisit::Visitor<'tcx> for ContainsUnsafeFnHeader<'tcx> {
    type NestedFilter = OnlyBodies;

    fn nested_visit_map(&mut self) -> Self::Map {
        self.tcx.hir()
    }

    fn visit_fn(
        &mut self,
        fn_kind: rustc_hir::intravisit::FnKind<'tcx>,
        fd: &'tcx rustc_hir::FnDecl<'tcx>,
        body_id: BodyId,
        _: Span,
        id: LocalDefId,
    ) {
        if body_id == self.body_id {
            match fn_kind {
                rustc_hir::intravisit::FnKind::ItemFn(_, _, fn_header) => {
                    if fn_header.unsafety == rustc_hir::Unsafety::Unsafe {
                        self.contains_unsafe_fnheader = true;
                    }
                }
                rustc_hir::intravisit::FnKind::Method(_, fn_sig) => {
                    if fn_sig.header.unsafety == rustc_hir::Unsafety::Unsafe {
                        self.contains_unsafe_fnheader = true;
                    }
                }
                _ => (),
            }
        }
        intravisit::walk_fn(self, fn_kind, fd, body_id, id)
    }
}

/// (`DefId` of ADT) => Vec<(HirId of relevant impl block, impl_self_ty)>
/// We use this map to quickly access associated impl blocks per ADT.
/// `impl_self_ty` in the return value may differ from `tcx.type_of(ADT.DefID)`,
/// as different instantiations of the same ADT are distinct `Ty`s.
/// (e.g. Foo<i32, i64>, Foo<String, i32>)
pub type AdtImplMap<'tcx> = FxHashMap<DefId, Vec<(LocalDefId, Ty<'tcx>)>>;

/// Create & initialize `AdtImplMap`.
/// `AdtImplMap` is initialized before analysis of each crate,
/// avoiding quadratic complexity of scanning all impl blocks for each ADT.
pub fn create_adt_impl_map<'tcx>(tcx: TyCtxt<'tcx>) -> AdtImplMap<'tcx> {
    let mut map = FxHashMap::default();

    for id in tcx.hir_crate_items(()).items() {
        let item = tcx.hir().item(id);
        if let ItemKind::Impl(Impl { self_ty, .. }) = item.kind {
            // `Self` type of the given impl block.
            let impl_self_ty = tcx.type_of(self_ty.hir_id.owner).skip_binder();

            if let TyKind::Adt(impl_self_adt_def, _impl_substs) = impl_self_ty.kind() {
                // We use `AdtDef.did` as key for `AdtImplMap`.
                // For any crazy instantiation of the same generic ADT (Foo<i32>, Foo<String>, etc..),
                // `AdtDef.did` refers to the original ADT definition.
                // Thus it can be used to map & collect impls for all instantitations of the same ADT.

                map.entry(impl_self_adt_def.did())
                    .or_insert_with(|| Vec::new())
                    .push((item.owner_id.def_id, impl_self_ty));
            }
        }
    }

    map
}
