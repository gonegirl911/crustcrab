use dashmap::DashMap;
use rustc_hash::FxHasher;
use std::hash::BuildHasherDefault;

pub type FxDashMap<K, V> = DashMap<K, V, BuildHasherDefault<FxHasher>>;
