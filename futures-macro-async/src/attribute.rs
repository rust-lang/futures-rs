use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Attribute {
    pub unpin: bool,
    pub boxed: bool,
    pub send: bool,
}

impl Attribute {
    pub const NONE: Attribute = Attribute { unpin: false, boxed: false, send: false };
    pub const SEND: Attribute = Attribute { unpin: false, boxed: false, send: true };
    pub const BOXED: Attribute = Attribute { unpin: false, boxed: true, send: false };
    pub const BOXED_SEND: Attribute = Attribute { unpin: false, boxed: true, send: true };
    pub const UNPIN: Attribute = Attribute { unpin: true, boxed: false, send: false };
    pub const UNPIN_SEND: Attribute = Attribute { unpin: true, boxed: false, send: true };
    pub const UNPIN_BOXED: Attribute = Attribute { unpin: true, boxed: true, send: false };
    pub const UNPIN_BOXED_SEND: Attribute = Attribute { unpin: true, boxed: true, send: true };
}

impl<I> From<I> for Attribute where I: Iterator, I::Item: fmt::Display + AsRef<str> {
    fn from(args: I) -> Self {
        let mut attribute = Attribute::NONE;

        for arg in args {
            match arg.as_ref() {
                "boxed" => {
                    if attribute.boxed {
                        panic!("duplicate 'boxed' argument");
                    }
                    attribute.boxed = true;
                }
                "send" => {
                    if attribute.send {
                        panic!("duplicate 'send' argument");
                    }
                    attribute.send = true;
                }
                "unpin" => {
                    if attribute.unpin {
                        panic!("duplicate 'unpin' argument");
                    }
                    attribute.unpin = true;
                }
                _ => {
                    panic!("unexpected macro argument '{}'", arg);
                }
            }
        }

        attribute
    }
}
