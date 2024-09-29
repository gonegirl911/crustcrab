use indexmap::IndexSet;
use rustc_hash::FxBuildHasher;

pub type FxIndexSet<T> = IndexSet<T, FxBuildHasher>;
