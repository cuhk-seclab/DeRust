use std::collections::{HashMap, HashSet};

use rustc_span::Symbol;

use maplit::hashmap;
use once_cell::sync::Lazy;

use crate::analysis::{TypeBehaviorFlag, UnsafeDataflowBehaviorFlag};

/*
How to find a path for unknown item:
1. Modify tests/utility/rurda_paths_discovery.rs
2. cargo run --bin rudra -- --crate-type lib tests/utility/rudra_paths_discovery.rs

For temporary debugging, you can also change this line in `prelude.rs`
`let names = self.get_def_path(def_id);`
to
`let names = dbg!(self.get_def_path(def_id));`
*/

// Strong bypasses
pub const PTR_READ: [&str; 3] = ["core", "ptr", "read"];
pub const PTR_DIRECT_READ: [&str; 5] = ["core", "ptr", "const_ptr", "<impl *const T>", "read"];

pub const INTRINSICS_COPY: [&str; 3] = ["core", "intrinsics", "copy"];
pub const INTRINSICS_COPY_NONOVERLAPPING: [&str; 3] = ["core", "intrinsics", "copy_nonoverlapping"];

pub const VEC_SET_LEN: [&str; 4] = ["alloc", "vec", "Vec", "set_len"];
pub const VEC_FROM_RAW_PARTS: [&str; 4] = ["alloc", "vec", "Vec", "from_raw_parts"];

// Weak bypasses
pub const TRANSMUTE: [&str; 4] = ["core", "intrinsics", "", "transmute"];

pub const PTR_WRITE: [&str; 3] = ["core", "ptr", "write"];
pub const PTR_DIRECT_WRITE: [&str; 5] = ["core", "ptr", "mut_ptr", "<impl *mut T>", "write"];

pub const PTR_AS_REF: [&str; 5] = ["core", "ptr", "const_ptr", "<impl *const T>", "as_ref"];
pub const PTR_AS_MUT: [&str; 5] = ["core", "ptr", "mut_ptr", "<impl *mut T>", "as_mut"];
pub const NON_NULL_AS_REF: [&str; 5] = ["core", "ptr", "non_nul", "NonNull", "as_ref"];
pub const NON_NULL_AS_MUT: [&str; 5] = ["core", "ptr", "non_nul", "NonNull", "as_mut"];

pub const SLICE_GET_UNCHECKED: [&str; 4] = ["core", "slice", "<impl [T]>", "get_unchecked"];
pub const SLICE_GET_UNCHECKED_MUT: [&str; 4] = ["core", "slice", "<impl [T]>", "get_unchecked_mut"];

pub const PTR_SLICE_FROM_RAW_PARTS: [&str; 3] = ["core", "ptr", "slice_from_raw_parts"];
pub const PTR_SLICE_FROM_RAW_PARTS_MUT: [&str; 3] = ["core", "ptr", "slice_from_raw_parts_mut"];
pub const SLICE_FROM_RAW_PARTS: [&str; 4] = ["core", "slice", "raw", "from_raw_parts"];
pub const SLICE_FROM_RAW_PARTS_MUT: [&str; 4] = ["core", "slice", "raw", "from_raw_parts_mut"];

// Generic function call
pub const INTRINSICS_DROP_IN_PLACE: [&str; 3] = ["core", "intrinsics", "drop_in_place"];
pub const PTR_DROP_IN_PLACE: [&str; 3] = ["core", "ptr", "drop_in_place"];
pub const PTR_DIRECT_DROP_IN_PLACE: [&str; 5] =
    ["core", "ptr", "mut_ptr", "<impl *mut T>", "drop_in_place"];

// More sources
pub const STRING_FROM_RAW_PARTS: [&str; 4] = ["alloc", "string", "String", "from_raw_parts"];
pub const BOX_FROM_RAW: [&str; 4] = ["alloc", "boxed", "Box", "from_raw"];

// More sinks
pub const VEC_FROM_ELEM: [&str; 3] = ["alloc", "vec", "from_elem"];
pub const VEC_INDEX: [&str; 5] = ["core", "ops", "index", "Index", "index"];
pub const STR_GET_UNCHECKED: [&str; 4] = ["core", "str", "<impl str>", "get_unchecked"];
pub const STR_GET_UNCHECKED_MUT: [&str; 4] = ["core", "str", "<impl str>", "get_unchecked_mut"];

pub struct PathSet {
    set: HashSet<Vec<Symbol>>,
}

impl PathSet {
    pub fn new(path_arr: &[&[&str]]) -> Self {
        let mut set = HashSet::new();
        for path in path_arr {
            let path_vec = path.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>();
            set.insert(path_vec);
        }

        PathSet { set }
    }

    pub fn contains(&self, target: &Vec<Symbol>) -> bool {
        self.set.contains(target)
    }
}

/// Special path used only for path discovery
pub static SPECIAL_PATH_DISCOVERY: Lazy<PathSet> =
    Lazy::new(move || PathSet::new(&[&["rudra_paths_discovery", "PathsDiscovery", "discover"]]));

pub static STRONG_LIFETIME_BYPASS_LIST: Lazy<PathSet> = Lazy::new(move || {
    PathSet::new(&[
        &PTR_READ,
        &PTR_DIRECT_READ,
        //
        &INTRINSICS_COPY,
        &INTRINSICS_COPY_NONOVERLAPPING,
        //
        &VEC_SET_LEN,
        &VEC_FROM_RAW_PARTS,
    ])
});

pub static WEAK_LIFETIME_BYPASS_LIST: Lazy<PathSet> = Lazy::new(move || {
    PathSet::new(&[
        &TRANSMUTE,
        //
        &PTR_WRITE,
        &PTR_DIRECT_WRITE,
        //
        &PTR_AS_REF,
        &PTR_AS_MUT,
        &NON_NULL_AS_REF,
        &NON_NULL_AS_MUT,
        //
        &SLICE_GET_UNCHECKED,
        &SLICE_GET_UNCHECKED_MUT,
        //
        &PTR_SLICE_FROM_RAW_PARTS,
        &PTR_SLICE_FROM_RAW_PARTS_MUT,
        &SLICE_FROM_RAW_PARTS,
        &SLICE_FROM_RAW_PARTS_MUT,
    ])
});

pub static GENERIC_FN_LIST: Lazy<PathSet> = Lazy::new(move || {
    PathSet::new(&[
        &PTR_DROP_IN_PLACE,
        &PTR_DIRECT_DROP_IN_PLACE,
        &INTRINSICS_DROP_IN_PLACE,
    ])
});

// Caused by type conversion
pub static LIFETIME_EXPAND_LIST: Lazy<PathSet> = Lazy::new(move || {
    PathSet::new(&[
        &VEC_FROM_RAW_PARTS,
        &PTR_AS_REF,
        &PTR_AS_MUT,
        &NON_NULL_AS_REF,
        &NON_NULL_AS_MUT,
        &PTR_SLICE_FROM_RAW_PARTS,
        &PTR_SLICE_FROM_RAW_PARTS_MUT,
        &SLICE_FROM_RAW_PARTS,
        &SLICE_FROM_RAW_PARTS_MUT,
        &STRING_FROM_RAW_PARTS,
        &BOX_FROM_RAW,
    ])
});

pub static SINK_FN_LIST: Lazy<PathSet> = Lazy::new(move || {
    PathSet::new(&[
        &PTR_DROP_IN_PLACE,
        &PTR_DIRECT_DROP_IN_PLACE,
        &INTRINSICS_DROP_IN_PLACE,
        &PTR_READ,
        &PTR_DIRECT_READ,
        &INTRINSICS_COPY,
        &INTRINSICS_COPY_NONOVERLAPPING,
        &PTR_WRITE,
        &PTR_DIRECT_WRITE,
        &SLICE_GET_UNCHECKED,
        &SLICE_GET_UNCHECKED_MUT,
        &VEC_FROM_ELEM,
        &VEC_INDEX,
        &STR_GET_UNCHECKED,
        &STR_GET_UNCHECKED_MUT,
    ])
});

type TypePathMap = HashMap<Vec<Symbol>, TypeBehaviorFlag>;

// Convert paths into TypeBehaviorFlag
pub static LIFETIME_EXPAND_MAP: Lazy<TypePathMap> = Lazy::new(move || {
    hashmap! {
        VEC_FROM_RAW_PARTS.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::VecFromRawParts,
        PTR_AS_REF.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::PtrAsRef,
        PTR_AS_MUT.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::PtrAsMut,
        NON_NULL_AS_REF.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::NonNullAsRef,
        NON_NULL_AS_MUT.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::NonNullAsMut,
        PTR_SLICE_FROM_RAW_PARTS.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::PtrSliceFromRawParts,
        PTR_SLICE_FROM_RAW_PARTS_MUT.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::PtrSliceFromRawPartsMut,
        SLICE_FROM_RAW_PARTS.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::SliceFromRawParts,
        SLICE_FROM_RAW_PARTS_MUT.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::SliceFromRawPartsMut,
        STRING_FROM_RAW_PARTS.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::StringFromRawParts,
        BOX_FROM_RAW.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::BoxFromRaw,
    }
});

pub static GENERIC_FN_MAP: Lazy<TypePathMap> = Lazy::new(move || {
    hashmap! {
        PTR_DROP_IN_PLACE.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::PtrDropInPlace,
        PTR_DIRECT_DROP_IN_PLACE.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::PtrDirectDropInPlace,
        INTRINSICS_DROP_IN_PLACE.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::IntrinsicsDropInPlace,
    }
});

pub static SINK_FN_MAP: Lazy<TypePathMap> = Lazy::new(move || {
    hashmap! {
        PTR_DROP_IN_PLACE.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::PtrDropInPlace,
        PTR_DIRECT_DROP_IN_PLACE.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::PtrDirectDropInPlace,
        INTRINSICS_DROP_IN_PLACE.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::IntrinsicsDropInPlace,
        PTR_READ.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::PtrRead,
        PTR_DIRECT_READ.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::PtrDirectRead,
        INTRINSICS_COPY.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::IntrinsicsCopy,
        INTRINSICS_COPY_NONOVERLAPPING.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::IntrinsicsCopyNonoverlapping,
        PTR_WRITE.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::PtrWrite,
        PTR_DIRECT_WRITE.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::PtrDirectWrite,
        SLICE_GET_UNCHECKED.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::SliceGetUnchecked,
        SLICE_GET_UNCHECKED_MUT.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::SliceGetUncheckedMut,
        VEC_FROM_ELEM.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::VecFromElem,
        VEC_INDEX.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::VecIndex,
        STR_GET_UNCHECKED.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::StrGetUnchecked,
        STR_GET_UNCHECKED_MUT.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => TypeBehaviorFlag::StrGetUncheckedMut,
    }
});

type PathMap = HashMap<Vec<Symbol>, UnsafeDataflowBehaviorFlag>;

pub static STRONG_BYPASS_MAP: Lazy<PathMap> = Lazy::new(move || {
    use UnsafeDataflowBehaviorFlag as BehaviorFlag;

    hashmap! {
        PTR_READ.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::READ_FLOW,
        PTR_DIRECT_READ.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::READ_FLOW,
        //
        INTRINSICS_COPY.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::COPY_FLOW,
        INTRINSICS_COPY_NONOVERLAPPING.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::COPY_FLOW,
        //
        VEC_SET_LEN.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::VEC_SET_LEN,
        //
        VEC_FROM_RAW_PARTS.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::VEC_FROM_RAW,
    }
});

pub static WEAK_BYPASS_MAP: Lazy<PathMap> = Lazy::new(move || {
    use UnsafeDataflowBehaviorFlag as BehaviorFlag;

    hashmap! {
        TRANSMUTE.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::TRANSMUTE,
        //
        PTR_WRITE.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::WRITE_FLOW,
        PTR_DIRECT_WRITE.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::WRITE_FLOW,
        //
        PTR_AS_REF.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::PTR_AS_REF,
        PTR_AS_MUT.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::PTR_AS_REF,
        NON_NULL_AS_REF.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::PTR_AS_REF,
        NON_NULL_AS_MUT.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::PTR_AS_REF,
        //
        SLICE_GET_UNCHECKED.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::SLICE_UNCHECKED,
        SLICE_GET_UNCHECKED_MUT.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::SLICE_UNCHECKED,
        //
        PTR_SLICE_FROM_RAW_PARTS.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::SLICE_FROM_RAW,
        PTR_SLICE_FROM_RAW_PARTS_MUT.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::SLICE_FROM_RAW,
        SLICE_FROM_RAW_PARTS.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::SLICE_FROM_RAW,
        SLICE_FROM_RAW_PARTS_MUT.iter().map(|p| Symbol::intern(p)).collect::<Vec<_>>() => BehaviorFlag::SLICE_FROM_RAW,
    }
});
