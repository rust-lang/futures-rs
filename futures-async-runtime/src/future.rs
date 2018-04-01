use std::ops::{Generator, GeneratorState};

use super::{IsResult, Reset, CTX};

use futures_core::Never;
use futures_core::task;
use futures_core::{Poll, Async, Future};

pub trait MyFuture<T: IsResult>: Future<Item=T::Ok, Error = T::Err> {}

impl<F, T> MyFuture<T> for F
    where F: Future<Item = T::Ok, Error = T::Err > + ?Sized,
          T: IsResult
{}

/// Small shim to translate from a generator to a future.
///
/// This is the translation layer from the generator/coroutine protocol to
/// the futures protocol.
struct GenFuture<T>(T);

pub fn gen_move<T>(gen: T) -> impl MyFuture<T::Return>
    where T: Generator<Yield = Async<Never>>,
          T::Return: IsResult,
{
    GenFuture(gen)
}

impl<T> Future for GenFuture<T>
    where T: Generator<Yield = Async<Never>>,
          T::Return: IsResult,
{
    type Item = <T::Return as IsResult>::Ok;
    type Error = <T::Return as IsResult>::Err;

    fn poll(&mut self, ctx: &mut task::Context) -> Poll<Self::Item, Self::Error> {
        CTX.with(|cell| {
            let _r = Reset::new(ctx, cell);
            // Because we are controlling the creation of our underlying
            // generator, we know that this is definitely a movable generator
            // so calling resume is always safe.
            match unsafe { self.0.resume() } {
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
