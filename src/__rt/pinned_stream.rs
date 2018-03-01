use std::ops::{Generator, GeneratorState};
use std::marker::PhantomData;
use std::mem;

use anchor_experiment::{Pin, MovePinned};
use futures::task;
use futures::prelude::{Poll, Async};
use stable::StableStream;

use super::{CTX, Reset, IsResult};

pub trait MyStableStream<T, U: IsResult<Ok=()>>: StableStream<Item=T, Error=U::Err> {}

impl<F, T, U> MyStableStream<T, U> for F
    where F: StableStream<Item = T, Error = U::Err> + ?Sized,
          U: IsResult<Ok=()>
{}

/// Small shim to translate from a generator to a stream.
struct GenStableStream<U, T> {
    gen: T,
    done: bool,
    phantom: PhantomData<U>,
}

impl<U, T> !MovePinned for GenStableStream<U, T> { }

pub fn gen_stream_pinned<T, U>(gen: T) -> impl MyStableStream<U, T::Return>
    where T: Generator<Yield = Async<U>>,
          T::Return: IsResult<Ok = ()>,
{
    GenStableStream { gen, done: false, phantom: PhantomData }
}

impl<U, T> StableStream for GenStableStream<U, T>
    where T: Generator<Yield = Async<U>>,
          T::Return: IsResult<Ok = ()>,
{
    type Item = U;
    type Error = <T::Return as IsResult>::Err;

    fn poll_next(mut self: Pin<Self>, ctx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        CTX.with(|cell| {
            let _r = Reset(cell.get(), cell);
            cell.set(unsafe { mem::transmute(ctx) });
            let this: &mut Self = unsafe { Pin::get_mut(&mut self) };
            if this.done { return Ok(Async::Ready(None)) }
            match this.gen.resume() {
                GeneratorState::Yielded(Async::Ready(e)) => {
                    Ok(Async::Ready(Some(e)))
                }
                GeneratorState::Yielded(Async::Pending) => {
                    Ok(Async::Pending)
                }
                GeneratorState::Complete(e) => {
                    this.done = true;
                    e.into_result().map(|()| Async::Ready(None))
                }
            }
        })
    }
}
