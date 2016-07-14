use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A bloom-filter-like set of "tokens" which is used to prune the amount of
/// calls to `poll` in a future computation graph.
pub struct Tokens {
    repr: Repr,
}

enum Repr {
    All,
    One(usize),
    Empty,
    Bloom(AtomicTokens),
}

// NB. this is not exported currently, that's intentional, just used in forget()
pub struct AtomicTokens {
    data: [AtomicUsize; 16],
}

/// A static which corresponds to a set which contains all tokens, useful for
/// passing around in various combinators.
pub static TOKENS_ALL: Tokens = Tokens { repr: Repr::All };

/// A static which corresponds to a set of no tokens, useful for passing around
/// in various combinators.
pub static TOKENS_EMPTY: Tokens = Tokens { repr: Repr::Empty };

impl Tokens {
    /// Creates a new set of tokens representing that no events have happened.
    ///
    /// The returned set will always return `false` for the `may_contain`
    /// method.
    pub fn empty() -> Tokens {
        Tokens { repr: Repr::Empty }
    }

    /// Creates a new set of tokens representing that all possible events may
    /// have happened.
    ///
    /// The returned set will always return `true` for the `may_contain` method.
    pub fn all() -> Tokens {
        Tokens { repr: Repr::All }
    }

    /// Creates a new set of tokens from the `usize` sentinel provided.
    ///
    /// Note that this may be a lossy conversion as `Tokens` may contain more
    /// bits of information than a `usize`.
    pub fn one(u: usize) -> Tokens {
        Tokens { repr: Repr::One(u) }
    }

    /// Insert a new token into this token set.
    pub fn insert(&mut self, token: usize) {
        let other = match self.repr {
            Repr::Bloom(ref mut b) => return b.insert(token),
            Repr::All => return,
            Repr::Empty => None,
            Repr::One(a) => Some(a),
        };
        let mut filter = AtomicTokens::empty();
        filter.insert(token);
        if let Some(other) = other {
            filter.insert(other);
        }
        self.repr = Repr::Bloom(filter);
    }

    /// Returns whether this set of tokens may contain any token contained in
    /// `other`.
    ///
    /// If `false` is returned then it is known with certainty that `self` does
    /// not contain any events that could be in `other`.
    ///
    /// If `true` is returned then events in `other` **may** be contained in
    /// `self`, and more work must be done to know for sure.
    pub fn may_contain(&self, token: usize) -> bool {
        match self.repr {
            Repr::All => true,
            Repr::One(a) => a == token,
            Repr::Empty => false,
            Repr::Bloom(ref b) => b.may_contain(token),
        }
    }
}

impl AtomicTokens {
    pub fn empty() -> AtomicTokens {
        AtomicTokens {
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

    pub fn all() -> AtomicTokens {
        AtomicTokens {
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

    pub fn get_tokens(&self) -> Tokens {
        let ret = AtomicTokens::empty();
        for (src, dst) in self.data.iter().zip(ret.data.iter()) {
            dst.store(src.swap(0, Ordering::SeqCst), Ordering::SeqCst);
        }
        Tokens { repr: Repr::Bloom(ret) }
    }

    pub fn add(&self, other: &Tokens) {
        match other.repr {
            Repr::Empty => {}
            Repr::All => {
                for dst in self.data.iter() {
                    dst.store(!0, Ordering::SeqCst);
                }
            }
            Repr::One(u) => {
                let (slot, bit) = self.index(u);
                slot.fetch_or(bit, Ordering::SeqCst);
            }
            Repr::Bloom(ref b) => {
                for (src, dst) in b.data.iter().zip(self.data.iter()) {
                    dst.fetch_or(src.load(Ordering::SeqCst), Ordering::SeqCst);
                }
            }
        }
    }

    pub fn insert(&mut self, other: usize) {
        let (slot, bit) = self.index(other);
        // TODO: don't do an atomic here
        slot.fetch_or(bit, Ordering::SeqCst);
    }

    pub fn may_contain(&self, token: usize) -> bool {
        let (slot, bit) = self.index(token);
        slot.load(Ordering::SeqCst) & bit == bit
    }

    pub fn any(&self) -> bool {
        self.data.iter().any(|s| s.load(Ordering::SeqCst) != 0)
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
    use super::{AtomicTokens, Tokens};

    #[test]
    fn all() {
        let a = AtomicTokens::all();
        assert!(a.may_contain(3));
        assert!(a.may_contain(!0));
        assert!(a.may_contain(20384));
        assert!(a.any());
    }

    #[test]
    fn empty() {
        let a = AtomicTokens::empty();
        assert!(!a.may_contain(3));
        assert!(!a.may_contain(!0));
        assert!(!a.may_contain(20384));
        assert!(!a.any());

        a.add(&Tokens::one(3));
        assert!(a.may_contain(3));
        assert!(!a.may_contain(4));
    }

    #[test]
    fn swap() {
        let a = AtomicTokens::all();
        let b = a.get_tokens();
        assert!(b.may_contain(3));
        assert!(b.may_contain(!0));
        assert!(b.may_contain(20384));
        assert!(!a.any());
    }
}
