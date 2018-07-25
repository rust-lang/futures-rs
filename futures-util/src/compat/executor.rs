
pub trait ExecCompat: Executor01<
        Compat<FutureObj<'static, ()>, BoxedExecutor>
    > + Clone + Send + 'static
{
    fn compat(self) -> ExecutorCompat<Self> 
        where Self: Sized;
}

impl<E> ExecCompat for E
where E: Executor01<
        Compat<FutureObj<'static, ()>, BoxedExecutor>
      >,
      E: Clone + Send + 'static
{
    fn compat(self) -> ExecutorCompat<Self> {
        ExecutorCompat {
            exec: self,
        }
    }
}

#[derive(Clone)]
pub struct ExecutorCompat<E> {
    exec: E
}

impl<E> Executor03 for ExecutorCompat<E> 
    where E: Executor01<
        Compat<FutureObj<'static, ()>, Box<Executor03>>
    >,
    E: Clone + Send + 'static,
{
    fn spawn_obj(&mut self, obj: FutureObj<'static, ()>) -> Result<(), task::SpawnObjError> {
        
        self.exec.execute(obj.compat(Box::new(self.clone())))
                 .map_err(|exec_err| {
                     use futures_core::task::{SpawnObjError, SpawnErrorKind};
                     
                     let fut = exec_err.into_future().compat().map(|_| ());
                     SpawnObjError {
                         kind: SpawnErrorKind::shutdown(),
                         task: Box::new(fut).into(),   
                     }
                 })
    }
}
