use std::mem;
use std::ops::{Generator, GeneratorState};

use anchor_experiment::{Pin, MovePinned};

use super::{IsResult, Mu, Reset, CTX};

use stable::StableFuture;
use task;
use prelude::{Poll, Async};

pub trait MyStableFuture<T: IsResult>: StableFuture<Item=T::Ok, Error = T::Err> {}

impl<F, T> MyStableFuture<T> for F
    where F: StableFuture<Item = T::Ok, Error = T::Err> + ?Sized,
          T: IsResult,
{}

struct GenStableFuture<T>(T);

impl<T> !MovePinned for GenStableFuture<T> { }

impl<T> StableFuture for GenStableFuture<T>
    where T: Generator<Yield = Async<Mu>>,
          T::Return: IsResult,
{
    type Item = <T::Return as IsResult>::Ok;
    type Error = <T::Return as IsResult>::Err;

    fn poll(mut self: Pin<Self>, ctx: &mut task::Context) -> Poll<Self::Item, Self::Error> {
        CTX.with(|cell| {
            let _r = Reset(cell.get(), cell);
            cell.set(unsafe { mem::transmute(ctx) });
            let this: &mut Self = unsafe { Pin::get_mut(&mut self) };
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

pub fn gen_pinned<'a, T>(gen: T) -> impl MyStableFuture<T::Return> + 'a
    where T: Generator<Yield = Async<Mu>> + 'a,
          T::Return: IsResult,
{
    GenStableFuture(gen)
}
