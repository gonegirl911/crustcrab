use super::utils::thread_count;

pub fn init(headroom: usize) {
    rayon::ThreadPoolBuilder::new()
        .num_threads(thread_count(headroom))
        .build_global()
        .unwrap();
}
