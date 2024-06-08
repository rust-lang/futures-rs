use core::pin::Pin;

use pin_project_lite::pin_project;

pin_project! {
    /// UnfoldState used for stream and sink unfolds
    #[project = UnfoldStateProj]
    #[project_replace = UnfoldStateProjReplace]
    #[derive(Debug)]
    pub(crate) enum UnfoldState<T, Fut> {
        Value {
            value: T,
        },
        Future {
            #[pin]
            future: Fut,
        },
        Empty,
    }
}

impl<T, Fut> UnfoldState<T, Fut> {
    pub(crate) fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    pub(crate) fn is_future(&self) -> bool {
        matches!(self, Self::Future { .. })
    }

    pub(crate) fn project_future(self: Pin<&mut Self>) -> Option<Pin<&mut Fut>> {
        match self.project() {
            UnfoldStateProj::Future { future } => Some(future),
            _ => None,
        }
    }

    pub(crate) fn take_value(self: Pin<&mut Self>) -> Option<T> {
        match &*self {
            Self::Value { .. } => match self.project_replace(Self::Empty) {
                UnfoldStateProjReplace::Value { value } => Some(value),
                _ => unreachable!(),
            },
            _ => None,
        }
    }
}
