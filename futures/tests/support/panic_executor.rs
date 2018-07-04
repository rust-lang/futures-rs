use futures::task::{Executor, TaskObj, SpawnObjError};

pub struct PanicExecutor;

impl Executor for PanicExecutor {
    fn spawn_obj(&mut self, _: TaskObj) -> Result<(), SpawnObjError> {
        panic!("should not spawn")
    }
}
