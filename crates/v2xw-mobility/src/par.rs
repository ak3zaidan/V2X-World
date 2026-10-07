//! Ordered parallel maps: how the mobility step uses more than one core without the
//! thread count reaching a single output bit (ADR 0004 decision 5).
//!
//! [`map_ordered`] maps a pure function over a slice on rayon's pool and returns the
//! results **in the slice's order** — rayon's indexed `collect` places each result at its
//! input's index, so the merge is the slice order whatever thread computed what, and how
//! many threads there are (`RAYON_NUM_THREADS`) changes the wall time and nothing else.
//! Below [`PAR_MIN_ITEMS`] the map runs on the calling thread: a pool hand-off costs more
//! than a few dozen decisions.

use rayon::prelude::*;

/// The fewest items worth sending to the pool.
pub const PAR_MIN_ITEMS: usize = 64;

/// `items.iter().map(f).collect()`, on the pool when it is worth it, in `items` order.
pub fn map_ordered<T, R, F>(items: &[T], f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync + Send,
{
    if items.len() < PAR_MIN_ITEMS || rayon::current_num_threads() < 2 {
        items.iter().map(f).collect()
    } else {
        items.par_iter().map(f).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_merge_is_in_input_order_on_any_pool() {
        let items: Vec<u64> = (0..10_000).collect();
        let f = |x: &u64| x.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(17);
        let serial: Vec<u64> = items.iter().map(f).collect();
        for threads in [1, 2, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("pool");
            let got = pool.install(|| map_ordered(&items, f));
            assert_eq!(got, serial, "{threads} threads");
        }
    }
}
