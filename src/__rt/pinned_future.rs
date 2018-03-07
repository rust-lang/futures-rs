use std::ops::{Generator, GeneratorState};

use anchor_experiment::{PinMut, Unpin};

use super::{IsResult, Reset, CTX};

use futures::Never;
use stable::StableFuture;
use task;
use prelude::{Poll, Async};

pub trait MyStableFuture<T: IsResult>: StableFuture<Item=T::Ok, Error = T::Err> {}

impl<F, T> MyStableFuture<T> for F
    where F: StableFuture<Item = T::Ok, Error = T::Err> + ?Sized,
          T: IsResult,
{}

struct GenStableFuture<T>(T);

impl<T> !Unpin for GenStableFuture<T> { }

impl<T> StableFuture for GenStableFuture<T>
    where T: Generator<Yield = Async<Never>>,
          T::Return: IsResult,
{
    type Item = <T::Return as IsResult>::Ok;
    type Error = <T::Return as IsResult>::Err;

    fn poll(mut self: PinMut<Self>, ctx: &mut task::Context) -> Poll<Self::Item, Self::Error> {
        CTX.with(|cell| {
            let _r = Reset::new(ctx, cell);
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

pub fn gen_pinned<'a, T>(gen: T) -> impl MyStableFuture<T::Return> + 'a
    where T: Generator<Yield = Async<Never>> + 'a,
          T::Return: IsResult,
{
    GenStableFuture(gen)
}
