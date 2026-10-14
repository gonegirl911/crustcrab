use std::{num::NonZero, thread};

pub fn thread_count(headroom: usize) -> usize {
    thread::available_parallelism()
        .map_or(1, NonZero::get)
        .saturating_sub(headroom)
        .max(1)
}
