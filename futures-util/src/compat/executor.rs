
use super::Compat;
use crate::{TryFutureExt, FutureExt, future::NeverError};
use futures::future::Executor as Executor01;
use futures_core::task::Executor as Executor03;
use futures_core::task as task03;
use futures_core::future::FutureObj;

pub struct BoxedExecutor03(Box<dyn Executor03 + Send>);

impl Executor03 for BoxedExecutor03 {
    fn spawn_obj(
        &mut self,
        future: FutureObj<'static, ()>,
    ) -> Result<(), task03::SpawnObjError> {
        (&mut *self.0).spawn_obj(future)
    }
}

/// A future that can run on a futures 0.1 executor.
pub type Executor01Future = Compat<NeverError<FutureObj<'static, ()>>, BoxedExecutor03>;

/// Extension trait for futures 0.1 Executors.
pub trait Executor01CompatExt: Executor01<Executor01Future> +
                               Clone + Send + 'static
{
    /// Creates an `Executor` compatable with futures 0.3.
    fn compat(self) -> Executor01As03<Self>
        where Self: Sized;
}

impl<Ex> Executor01CompatExt for Ex
where Ex: Executor01<Executor01Future> + Clone + Send + 'static
{
    fn compat(self) -> Executor01As03<Self> {
        Executor01As03 {
            executor01: self,
        }
    }
}

/// Converts a futures 0.1 `Executor` into a futures 0.3 `Executor`.
#[derive(Clone)]
pub struct Executor01As03<Ex> {
    executor01: Ex
}

impl<Ex> Executor03 for Executor01As03<Ex>
where Ex: Executor01<Executor01Future>,
      Ex: Clone + Send + 'static,
{
    fn spawn_obj(
        &mut self,
        future: FutureObj<'static, ()>,
    ) -> Result<(), task03::SpawnObjError> {
        let future = future.never_error().compat(BoxedExecutor03(Box::new(self.clone())));

        match self.executor01.execute(future) {
            Ok(()) => Ok(()),
            Err(err) => {
                use futures_core::task::{SpawnObjError, SpawnErrorKind};

                let fut = err.into_future().into_inner().unwrap_or_else(|_| ());
                Err(SpawnObjError {
                    kind: SpawnErrorKind::shutdown(),
                    future: Box::new(fut).into(),
                })
            }
        }
    }
}
