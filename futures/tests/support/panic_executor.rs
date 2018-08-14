use futures::future::FutureObj;
use futures::task::{Spawn, SpawnObjError};

pub struct PanicExecutor;

impl Spawn for PanicExecutor {
    fn spawn_obj(&mut self, _: FutureObj<'static, ()>) -> Result<(), SpawnObjError> {
        panic!("should not spawn")
    }
}
