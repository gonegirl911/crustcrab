use super::available_parallelism;

pub fn init(headroom: usize) {
    rayon::ThreadPoolBuilder::new()
        .num_threads(available_parallelism(headroom))
        .build_global()
        .unwrap();
}
