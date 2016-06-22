use std::sync::Arc;

use {Future, PollResult, Wake};

impl<F: ?Sized + Future> Future for Box<F> {
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self) -> Option<PollResult<Self::Item, Self::Error>> {
        (**self).poll()
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        (**self).schedule(wake)
    }
}
