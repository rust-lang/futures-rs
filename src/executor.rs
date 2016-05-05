use util;

pub trait Executor {
    fn execute<F>(&self, f: F)
        where F: FnOnce() + Send + 'static,
              Self: Sized
    {
        self.execute_boxed(Box::new(f))
    }

    fn execute_boxed(&self, f: Box<ExecuteCallback>);
}

impl<T: Executor + ?Sized> Executor for Box<T> {
    fn execute_boxed(&self, f: Box<ExecuteCallback>) {
        (**self).execute_boxed(f)
    }
}

pub trait ExecuteCallback: Send + 'static {
    fn call(self: Box<Self>);
}

impl<F: FnOnce() + Send + 'static> ExecuteCallback for F {
    fn call(self: Box<F>) {
        (*self)()
    }
}

pub struct DefaultExecutor;

impl Executor for DefaultExecutor {
    fn execute<F: FnOnce() + Send + 'static>(&self, f: F) {
        f()
    }

    fn execute_boxed(&self, f: Box<ExecuteCallback>) {
        f.call()
    }
}

pub struct CatchUnwindExecutor;

impl Executor for CatchUnwindExecutor {
    fn execute<F: FnOnce() + Send + 'static>(&self, f: F) {
        let res = util::recover::<_, _, i32>(f);
        drop(res);
    }

    fn execute_boxed(&self, f: Box<ExecuteCallback>) {
        let res = util::recover::<_, _, i32>(|| f.call());
        drop(res);
    }
}
