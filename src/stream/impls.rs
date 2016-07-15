use std::sync::Arc;

use {Tokens, Wake};
use stream::{Stream, StreamResult};

impl<S: ?Sized + Stream> Stream for Box<S> {
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self, tokens: &Tokens)
            -> Option<StreamResult<Self::Item, Self::Error>> {
        (**self).poll(tokens)
    }

    fn schedule(&mut self, wake: &Arc<Wake>) {
        (**self).schedule(wake)
    }
}
