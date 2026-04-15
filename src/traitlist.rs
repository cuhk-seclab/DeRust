use std::collections::{HashMap, HashSet};

use rustc_span::Symbol;

use maplit::hashmap;
use once_cell::sync::Lazy;

// Rust standard trait sets
pub static TRAIT_LIST: Lazy<HashSet<&str>> = Lazy::new(|| {
    [
        "Sized",
        "Unsize",
        "StructuralPeq",
        "StructuralTeq",
        "Copy",
        "Clone",
        "Sync",
        "DiscriminantKind",
        "PointeeTrait",
        "Freeze",
        "FnPtrTrait",
        "Drop",
        "Destruct",
        "CoerceUnsized",
        "DispatchFromDyn",
        "TransmuteTrait",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Rem",
        "Neg",
        "Not",
        "BitXor",
        "BitAnd",
        "BitOr",
        "Shl",
        "Shr",
        "AddAssign",
        "SubAssign",
        "MulAssign",
        "DivAssign",
        "RemAssign",
        "BitXorAssign",
        "BitAndAssign",
        "BitOrAssign",
        "ShlAssign",
        "ShrAssign",
        "Index",
        "IndexMut",
        "Deref",
        "DerefMut",
        "Receiver",
        "Fn",
        "FnMut",
        "FnOnce",
        "Iterator",
        "Future",
        "Coroutine",
        "Unpin",
        "PartialEq",
        "PartialOrd",
        "Termination",
        "Try",
        "Tuple",
        "PointerLike",
        "ConstParamTy",
        // Customized added
        "fmt",
        "Display",
    ]
    .iter()
    .cloned()
    .collect()
});

// paths
pub const FMT_DISPLAY: [&str; 3] = ["std", "fmt", "Display"];
// More ...

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

pub static TRAIT_PATH_LIST: Lazy<PathSet> = Lazy::new(move || PathSet::new(&[&FMT_DISPLAY]));
