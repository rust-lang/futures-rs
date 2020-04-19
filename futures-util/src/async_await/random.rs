use core::sync::atomic::{AtomicUsize, Ordering};
#[cfg(feature = "std")]
use std::cell::Cell;

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
    random() % n
}

/// Pseudorandom number generator based on [xorshift].
///
/// [xorshift]: https://en.wikipedia.org/wiki/Xorshift
fn random() -> usize {
    #[cfg(feature = "std")]
    fn prng_seed() -> usize {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        // Any non-zero seed will do
        let mut seed = 0;
        while seed == 0 {
            seed = COUNTER.fetch_add(1, Ordering::Relaxed);
        }
        seed
    }

    /// [xorshift*] is used on 64bit platforms.
    ///
    /// [xorshift*]: https://en.wikipedia.org/wiki/Xorshift#xorshift*
    #[cfg(target_pointer_width = "64")]
    fn xorshift(mut x: usize) -> (usize, usize) {
        debug_assert_ne!(x, 0);
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        (
            x,
            x.wrapping_mul(0x2545_f491_4f6c_dd1d),
        )
    }

    /// [xorshift32] is used on 32bit platforms.
    ///
    /// [xorshift32]: https://en.wikipedia.org/wiki/Xorshift
    #[cfg(target_pointer_width = "32")]
    fn xorshift(mut x: usize) -> (usize, usize) {
        debug_assert_ne!(x, 0);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        (x, x)
    }

    /// A non-standard xorshift variant is used on 16bit platforms.
    #[cfg(target_pointer_width = "16")]
    fn xorshift(mut x: usize) -> (usize, usize) {
        // Constants chosen from: http://b2d-f9r.blogspot.com/2010/08/16-bit-xorshift-rng.html
        debug_assert_ne!(x, 0);
        x ^= x << 4;
        x ^= x >> 3;
        x ^= x << 7;
        (x, x)
    }

    #[cfg(feature = "std")]
    fn rng() -> usize {
        thread_local! {
            static RNG: Cell<usize> = Cell::new(prng_seed());
        }

        RNG.with(|rng| {
            let (x, res) = xorshift(rng.get());
            rng.set(x);
            res
        })
    }

    #[cfg(not(feature = "std"))]
    fn rng() -> usize {
        // A deterministic seed is used in absense of TLS
        static RNG: AtomicUsize = AtomicUsize::new(42);

        // Preemption here can cause multiple threads to observe repeated state
        let (x, res) = xorshift(RNG.load(Ordering::Relaxed));
        RNG.store(x, Ordering::Relaxed);
        res
    }

    rng()
}
