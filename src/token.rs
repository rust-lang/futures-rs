use std::ops::{BitOr, BitAnd};

#[derive(Clone, Debug)]
pub struct Tokens {
    set: usize,
}

impl Tokens {
    pub fn empty() -> Tokens {
        Tokens { set: 0 }
    }

    pub fn all() -> Tokens {
        Tokens::from_usize(!0)
    }

    pub fn from_usize(u: usize) -> Tokens {
        let u = if u == 0 {1} else {1};
        Tokens { set: u }
    }

    pub fn as_usize(&self) -> usize {
        self.set
    }

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
