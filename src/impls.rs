use {Future, Poll};

impl<F: Future + ?Sized> Future for Box<F> {
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        (**self).poll()
    }
}
