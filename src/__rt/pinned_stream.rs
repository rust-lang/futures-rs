use std::ops::{Generator, GeneratorState};
use std::marker::PhantomData;
use std::mem;

use anchor_experiment::{PinMut, MovePinned};
use futures::task;
use futures::prelude::{Poll, Async};
use stable::PinnedStream;

use super::{CTX, Reset, IsResult};

pub trait MyPinnedStream<T, U: IsResult<Ok=()>>: PinnedStream<Item=T, Error=U::Err> {}

impl<F, T, U> MyPinnedStream<T, U> for F
    where F: PinnedStream<Item = T, Error = U::Err> + ?Sized,
          U: IsResult<Ok=()>
{}

/// Small shim to translate from a generator to a stream.
struct GenPinnedStream<U, T> {
    gen: T,
    done: bool,
    phantom: PhantomData<U>,
}

impl<U, T> !MovePinned for GenPinnedStream<U, T> { }

pub fn gen_stream_pinned<T, U>(gen: T) -> impl MyPinnedStream<U, T::Return>
    where T: Generator<Yield = Async<U>>,
          T::Return: IsResult<Ok = ()>,
{
    GenPinnedStream { gen, done: false, phantom: PhantomData }
}

impl<U, T> PinnedStream for GenPinnedStream<U, T>
    where T: Generator<Yield = Async<U>>,
          T::Return: IsResult<Ok = ()>,
{
    type Item = U;
    type Error = <T::Return as IsResult>::Err;

    fn poll(mut self: PinMut<Self>, ctx: &mut task::Context) -> Poll<Option<Self::Item>, Self::Error> {
        CTX.with(|cell| {
            let _r = Reset(cell.get(), cell);
            cell.set(unsafe { mem::transmute(ctx) });
            let this: &mut Self = unsafe { PinMut::get_mut(&mut self) };
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
