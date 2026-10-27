pub mod global;
mod job_pool;

pub use rayon::*;

use std::{num::NonZero, thread};

fn available_parallelism(headroom: usize) -> usize {
    thread::available_parallelism()
        .map_or(1, NonZero::get)
        .saturating_sub(headroom)
        .max(1)
}
