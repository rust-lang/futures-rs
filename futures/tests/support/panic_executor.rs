use futures::future::FutureObj;
use futures::task::{Executor, SpawnObjError};

pub struct PanicExecutor;

impl Executor for PanicExecutor {
    fn spawn_obj(&mut self, _: FutureObj<'static, ()>) -> Result<(), SpawnObjError> {
        panic!("should not spawn")
    }
}
