use core::fmt;
#[cfg(feature = "use_std")]
use std::error::Error;

/// A value that can never happen!
///
/// For context:
///
/// - The boolean type `bool` has two values: `true` and `false`
/// - The unit type `()` has one value: `()`
/// - The empty type `Never` has no values!
///
/// You may see it in the wild in `Future<Error = Never>`,
/// which means that this future will never fail.
#[derive(Copy, Clone, Debug)]
pub enum Never {}

impl Never {
    /// Convert into any other type.
    ///
    /// Since it is impossible for `Never` to exist,
    /// we can exploit this to convert it into any type.
    ///
    /// This is also called the [Principle of Explosion]
    /// (https://en.wikipedia.org/wiki/Principle_of_explosion).
    #[inline(always)]
    pub fn never<T>(self) -> T {
        match self {}
    }
}

impl fmt::Display for Never {
    fn fmt(&self, _: &mut fmt::Formatter) -> fmt::Result {
        self.never()
    }
}

#[cfg(feature = "use_std")]
impl Error for Never {
    fn description(&self) -> &str {
        self.never()
    }

    fn cause(&self) -> Option<&Error> {
        self.never()
    }
}

pub trait InfallibleResultExt {
    type Item;

    fn infallible(self) -> Self::Item;
}

impl<T> InfallibleResultExt for Result<T, Never> {
    type Item = T;

    #[inline]
    fn infallible(self) -> Self::Item {
        self.unwrap_or_else(|x| x.never())
    }
}