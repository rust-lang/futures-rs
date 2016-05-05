use {Future, PollResult, Callback};

impl<F: ?Sized + Future> Future for Box<F> {
    type Item = F::Item;
    type Error = F::Error;

    // fn poll(&mut self) -> Option<PollResult<F::Item, F::Error>> {
    //     (**self).poll()
    // }

    // fn cancel(&mut self) {
    //     (**self).cancel()
    // }

    // fn await(&mut self) -> FutureResult<F::Item, F::Error> {
    //     (**self).await()
    // }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<F::Item, F::Error>) + Send + 'static,
    {
        (**self).schedule_boxed(Box::new(g))
    }

    fn schedule_boxed(&mut self, f: Box<Callback<F::Item, F::Error>>) {
        (**self).schedule_boxed(f)
    }
}

// impl<'a, F: ?Sized + Future> Future for &'a mut F {
//     type Item = F::Item;
//     type Error = F::Error;
//
//     fn poll(&mut self) -> Option<PollResult<F::Item, F::Error>> {
//         (**self).poll()
//     }
//
//     fn cancel(&mut self) {
//         (**self).cancel()
//     }
//
//     fn await(&mut self) -> FutureResult<F::Item, F::Error> {
//         (**self).await()
//     }
//
//     fn schedule<G>(&mut self, g: G)
//         where G: FnOnce(PollResult<F::Item, F::Error>) + Send + 'static,
//     {
//         (**self).schedule_boxed(Box::new(g))
//     }
//
//     fn schedule_boxed(&mut self, f: Box<Callback<F::Item, F::Error>>) {
//         (**self).schedule_boxed(f)
//     }
// }
