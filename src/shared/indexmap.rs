use indexmap::{IndexMap, IndexSet};
use rustc_hash::FxBuildHasher;

pub type FxIndexMap<K, V> = IndexMap<K, V, FxBuildHasher>;

pub type FxIndexSet<T> = IndexSet<T, FxBuildHasher>;
