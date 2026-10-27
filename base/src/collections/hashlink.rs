pub(crate) use hashlink::*;

use rustc_hash::FxBuildHasher;

pub(crate) type FxLinkedHashMap<K, V> = LinkedHashMap<K, V, FxBuildHasher>;
