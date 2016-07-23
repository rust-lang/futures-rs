use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A bloom-filter-like set of "tokens" which is used to prune the amount of
/// calls to `poll` in a future computation graph.
pub struct Tokens {
    data: [AtomicUsize; 16],
}

impl Tokens {
    pub fn empty() -> Tokens {
        Tokens {
            data: [
                AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
                AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
                AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
                AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
                AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
                AtomicUsize::new(0),
            ]
        }
    }

    pub fn all() -> Tokens {
        Tokens {
            data: [
                AtomicUsize::new(!0), AtomicUsize::new(!0), AtomicUsize::new(!0),
                AtomicUsize::new(!0), AtomicUsize::new(!0), AtomicUsize::new(!0),
                AtomicUsize::new(!0), AtomicUsize::new(!0), AtomicUsize::new(!0),
                AtomicUsize::new(!0), AtomicUsize::new(!0), AtomicUsize::new(!0),
                AtomicUsize::new(!0), AtomicUsize::new(!0), AtomicUsize::new(!0),
                AtomicUsize::new(!0),
            ],
        }
    }

    // Note that the atomics below are all `Relaxed`. None of this should be
    // relevant for memory safety (these are just "interest bits") and in
    // general there's other synchronization to ensure that all these writes are
    // eventually visible to some thread that's reading them.
    //
    // That, and this has been seen in profiles before, so...

    pub fn take(&self) -> Tokens {
        let ret = Tokens::empty();
        for (src, dst) in self.data.iter().zip(ret.data.iter()) {
            let v = src.swap(0, Ordering::Relaxed);
            if v != 0 {
                dst.store(v, Ordering::Relaxed);
            }
        }
        return ret
    }

    pub fn insert(&self, other: usize) {
        let (slot, bit) = self.index(other);
        slot.fetch_or(bit, Ordering::Relaxed);
    }

    pub fn may_contain(&self, token: usize) -> bool {
        let (slot, bit) = self.index(token);
        slot.load(Ordering::Relaxed) & bit == bit
    }

    pub fn any(&self) -> bool {
        self.data.iter().any(|s| s.load(Ordering::Relaxed) != 0)
    }

    /// Returns the slot in which that token will go along with the value
    /// that'll be or'd in.
    // TODO: this is a really bad bloom filter
    fn index(&self, token: usize) -> (&AtomicUsize, usize) {
        let bits_per_slot = mem::size_of_val(&self.data[0]) * 8;
        let size = self.data.len() * bits_per_slot;
        let bit = token % size;
        let slot = bit / bits_per_slot;
        let shift = bit % bits_per_slot;
        (&self.data[slot], 1 << shift)
    }
}

#[cfg(test)]
mod tests {
    use super::{Tokens};

    #[test]
    fn all() {
        let a = Tokens::all();
        assert!(a.may_contain(3));
        assert!(a.may_contain(!0));
        assert!(a.may_contain(20384));
        assert!(a.any());
    }

    #[test]
    fn empty() {
        let a = Tokens::empty();
        assert!(!a.may_contain(3));
        assert!(!a.may_contain(!0));
        assert!(!a.may_contain(20384));
        assert!(!a.any());

        a.insert(3);
        assert!(a.may_contain(3));
        assert!(!a.may_contain(4));
    }

    #[test]
    fn swap() {
        let a = Tokens::all();
        let b = a.take();
        assert!(b.may_contain(3));
        assert!(b.may_contain(!0));
        assert!(b.may_contain(20384));
        assert!(!a.any());
    }
}
