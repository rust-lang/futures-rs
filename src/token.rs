use std::ops::{BitOr, BitAnd};

/// A bloom-filter-like set of "tokens" which is used to prune the amount of
/// calls to `poll` in a future computation graph.
#[derive(Clone, Debug, PartialEq)]
pub struct Tokens {
    set: usize,
}

impl Tokens {
    /// Creates a new set of tokens representing that no events have happened.
    ///
    /// The returned set will always return `false` for the `may_contain`
    /// method.
    pub fn empty() -> Tokens {
        Tokens { set: 0 }
    }

    /// Creates a new set of tokens representing that all possible events may
    /// have happened.
    ///
    /// The returned set will always return `true` for the `may_contain` method.
    pub fn all() -> Tokens {
        Tokens::from_usize(!0)
    }

    /// Creates a new set of tokens from the `usize` sentinel provided.
    ///
    /// Note that this may be a lossy conversion as `Tokens` may contain more
    /// bits of information than a `usize`.
    pub fn from_usize(u: usize) -> Tokens {
        let u = if u == 0 {1} else {u};
        Tokens { set: u }
    }

    /// Returns this set of tokens as a `usize`.
    ///
    /// This can later be passed to `from_usize` to recreate the set of tokens.
    /// Note that this may be a lossy operation where the
    /// `Tokens::from_usize(foo.as_usize())` may return different answers to
    /// `may_contain` than `foo` originally would.
    pub fn as_usize(&self) -> usize {
        self.set
    }

    /// Returns whether this set of tokens may contain any token contained in
    /// `other`.
    ///
    /// If `false` is returned then it is known with certainty that `self` does
    /// not contain any events that could be in `other`.
    ///
    /// If `true` is returned then events in `other` **may** be contained in
    /// `self`, and more work must be done to know for sure.
    pub fn may_contain(&self, other: &Tokens) -> bool {
        self.set & other.set != 0
    }
}

impl<'a, 'b> BitOr<&'b Tokens> for &'a Tokens {
    type Output = Tokens;

    fn bitor(self, other: &'b Tokens) -> Tokens {
        Tokens::from_usize(self.set | other.set)
    }
}

impl<'a, 'b> BitAnd<&'b Tokens> for &'a Tokens {
    type Output = Tokens;

    fn bitand(self, other: &'b Tokens) -> Tokens {
        Tokens::from_usize(self.set & other.set)
    }
}
