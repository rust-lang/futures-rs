use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Attribute {
    pub boxed: bool,
    pub send: bool,
}

impl Attribute {
    pub const NONE: Attribute = Attribute { boxed: false, send: false };
    pub const SEND: Attribute = Attribute { boxed: false, send: true };
    pub const BOXED: Attribute = Attribute { boxed: true, send: false };
    pub const BOXED_SEND: Attribute = Attribute { boxed: true, send: true };
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
                _ => {
                    panic!("unexpected macro argument '{}'", arg);
                }
            }
        }

        attribute
    }
}
