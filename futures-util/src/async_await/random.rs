use core::num::Wrapping;

// Based on [Fisher–Yates shuffle].
//
// [Fisher–Yates shuffle]: https://en.wikipedia.org/wiki/Fisher–Yates_shuffle
#[doc(hidden)]
pub fn shuffle<T>(slice: &mut [T]) {
    for i in (1..slice.len()).rev() {
        slice.swap(i, gen_index(i + 1));
    }
}

/// Return a value from `0..n`.
fn gen_index(n: usize) -> usize {
    (random() % n as u64) as usize
}

/// Pseudorandom number generator based on [xorshift*].
///
/// [xorshift*]: https://en.wikipedia.org/wiki/Xorshift#xorshift*
#[cfg(feature = "std")]
fn random() -> u64 {
    use std::{
        cell::Cell,
        collections::hash_map::DefaultHasher,
        hash::Hasher,
        sync::atomic::{AtomicUsize, Ordering},
    };

    std::thread_local! {
        static RNG: Cell<Wrapping<u64>> = Cell::new(Wrapping(prng_seed()));
    }

    fn prng_seed() -> u64 {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        // Any non-zero seed will do
        let mut seed = 0;
        while seed == 0 {
            let mut hasher = DefaultHasher::new();
            hasher.write_usize(COUNTER.fetch_add(1, Ordering::Relaxed));
            seed = hasher.finish();
        }
        seed
    }

    RNG.with(|rng| {
        let x = rng.get();
        let (next, result) = xorshift64star(x);
        rng.set(next);
        result
    })
}

#[cfg(not(feature = "std"))]
fn random() -> u64 {
    use core::sync::atomic::{AtomicUsize, Ordering};

    static RNG: AtomicUsize = AtomicUsize::new(1);

    let x = RNG.load(Ordering::Relaxed);

    if core::mem::size_of::<usize>() == 4 {
        let next = xorshift32(x as u32);
        RNG.store(next as usize, Ordering::Relaxed);
        next as u64
    } else if core::mem::size_of::<usize>() == 8 {
        let (next, result) = xorshift64star(Wrapping(x as u64));
        RNG.store(next.0 as usize, Ordering::Relaxed);
        result
    } else {
        panic!("random() function is not supported on this platform");
    }
}

/// Xorshift64* algorithm.
/// Returns the next state and the random number; `(next_state, random_number)`.
#[inline]
fn xorshift64star(mut x: Wrapping<u64>) -> (Wrapping<u64>, u64) {
    debug_assert_ne!(x.0, 0);
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    (x, x.0.wrapping_mul(0x2545_f491_4f6c_dd1d))
}

/// Xorshift32 algorithm.
#[cfg(not(feature = "std"))]
#[inline]
fn xorshift32(mut x: u32) -> u32 {
    debug_assert_ne!(x, 0);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}
