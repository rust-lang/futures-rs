use core::pin::Pin;

/// UnfoldState used for stream and sink unfolds
#[derive(Debug)]
pub(crate) enum UnfoldState<T, R> {
    Value(T),
    Future(/* #[pin] */ R),
    Empty,
}

impl<T, R> UnfoldState<T, R> {
    pub(crate) fn project_future(self: Pin<&mut Self>) -> Option<Pin<&mut R>> {
        // SAFETY Normal pin projection on the `Future` variant
        unsafe {
            match self.get_unchecked_mut() {
                Self::Future(f) => Some(Pin::new_unchecked(f)),
                _ => None,
            }
        }
    }

    pub(crate) fn take_value(self: Pin<&mut Self>) -> Option<T> {
        // SAFETY We only move out of the `Value` variant which is not pinned
        match *self {
            Self::Value(_) => unsafe {
                match core::mem::replace(self.get_unchecked_mut(), UnfoldState::Empty) {
                    UnfoldState::Value(v) => Some(v),
                    _ => core::hint::unreachable_unchecked(),
                }
            },
            _ => None,
        }
    }
}
