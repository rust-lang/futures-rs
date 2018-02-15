//! dox

if_std! {
    use std::boxed::Box;
    use Future;

    /// TODO: dox
    pub trait Executor {
        /// TODO: dox
        fn spawn(&mut self, f: Box<Future<Item = (), Error = ()> + Send>) -> Result<(), SpawnError>;

        /// TODO: dox
        fn status(&self) -> Result<(), SpawnError> {
            Ok(())
        }

        // TODO: downcasting hooks
    }

    /// TODO: dox
    #[derive(Debug)]
    pub struct SpawnError {
        _a: ()
    }

    impl SpawnError {
        /// todo: dox
        pub fn shutdown() -> SpawnError {
            SpawnError { _a: () }
        }
    }
}
