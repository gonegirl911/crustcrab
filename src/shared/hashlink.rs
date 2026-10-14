use hashlink::LinkedHashMap;
use rustc_hash::FxBuildHasher;

pub type FxLinkedHashMap<K, V> = LinkedHashMap<K, V, FxBuildHasher>;
