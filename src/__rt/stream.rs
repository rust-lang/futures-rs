use std::ops::{Generator, GeneratorState};
use std::marker::PhantomData;

use futures::task;
use futures::prelude::{Poll, Async, Stream};

use super::{CTX, Reset, IsResult};

pub trait MyStream<T, U: IsResult<Ok=()>>: Stream<Item=T, Error=U::Err> {}

impl<F, T, U> MyStream<T, U> for F
    where F: Stream<Item = T, Error = U::Err> + ?Sized,
          U: IsResult<Ok=()>
{}

/// Small shim to translate from a generator to a stream.
struct GenStream<U, T> {
    gen: T,
    done: bool,
    phantom: PhantomData<U>,
}

pub fn gen_stream<T, U>(gen: T) -> impl MyStream<U, T::Return>
    where T: Generator<Yield = Async<U>>,
          T::Return: IsResult<Ok = ()>,
{
    GenStream { gen, done: false, phantom: PhantomData }
}

impl<U, T> Stream for GenStream<U, T>
    where T: Generator<Yield = Async<U>>,
          T::Return: IsResult<Ok = ()>,
{
    type Item = U;
    type Error = <T::Return as IsResult>::Err;

    fn poll_next(&mut self, ctx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        CTX.with(|cell| {
            let _r = Reset::new(ctx, cell);
            if self.done { return Ok(Async::Ready(None)) }
            match self.gen.resume() {
                GeneratorState::Yielded(Async::Ready(e)) => {
                    Ok(Async::Ready(Some(e)))
                }
                GeneratorState::Yielded(Async::Pending) => {
                    Ok(Async::Pending)
                }
                GeneratorState::Complete(e) => {
                    self.done = true;
                    e.into_result().map(|()| Async::Ready(None))
                }
            }
        })
    }
}
