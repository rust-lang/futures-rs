use self::Attribute::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Attribute {
    Pinned,
    PinnedBoxed,
    PinnedBoxedSend,
    Unpinned,
    UnpinnedBoxed,
    UnpinnedBoxedSend,
}

impl Attribute {
    pub fn boxed(&self) -> bool {
        match *self {
            PinnedBoxed | PinnedBoxedSend | UnpinnedBoxed | UnpinnedBoxedSend => true,
            Pinned | Unpinned => false,
        }
    }

    pub fn pinned(&self) -> bool {
        match *self {
            Pinned | PinnedBoxed | PinnedBoxedSend => true,
            Unpinned | UnpinnedBoxed | UnpinnedBoxedSend => false,
        }
    }
}
