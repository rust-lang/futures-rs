use std::mem;
use std::ops::{Generator, GeneratorState};

use anchor_experiment::PinMut;

use super::{IsResult, Mu, Reset, CTX};

use stable::PinnedFuture;
use task;
use prelude::{Poll, Async};

pub trait MyPinnedFuture<T: IsResult>: PinnedFuture<Item=T::Ok, Error = T::Err> {}

impl<F, T> MyPinnedFuture<T> for F
    where F: PinnedFuture<Item = T::Ok, Error = T::Err> + ?Sized,
          T: IsResult,
{}

struct GenPinnedFuture<T>(T);

impl<T> PinnedFuture for GenPinnedFuture<T>
    where T: Generator<Yield = Async<Mu>>,
          T::Return: IsResult,
{
    type Item = <T::Return as IsResult>::Ok;
    type Error = <T::Return as IsResult>::Err;

    fn poll(mut self: PinMut<Self>, ctx: &mut task::Context) -> Poll<Self::Item, Self::Error> {
        CTX.with(|cell| {
            let _r = Reset(cell.get(), cell);
            cell.set(unsafe { mem::transmute(ctx) });
            let this: &mut Self = unsafe { PinMut::get_mut(&mut self) };
            match this.0.resume() {
                GeneratorState::Yielded(Async::Pending)
                    => Ok(Async::Pending),
                GeneratorState::Yielded(Async::Ready(mu))
                    => match mu {},
                GeneratorState::Complete(e)
                    => e.into_result().map(Async::Ready),
            }
        })
    }
}

pub fn gen_pinned<'a, T>(gen: T) -> impl MyPinnedFuture<T::Return> + 'a
    where T: Generator<Yield = Async<Mu>> + 'a,
          T::Return: IsResult,
{
    GenPinnedFuture(gen)
}
