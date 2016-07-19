use std::sync::Arc;

use {Tokens, Wake, Poll};
use stream::Stream;

impl<S: ?Sized + Stream> Stream for Box<S> {
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self, t: &Tokens) -> Poll<Option<Self::Item>, Self::Error> {
        (**self).poll(t)
    }

    fn schedule(&mut self, wake: &Arc<Wake>) {
        (**self).schedule(wake)
    }
}
