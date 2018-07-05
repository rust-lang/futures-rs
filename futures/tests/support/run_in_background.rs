use futures::prelude::*;
use futures::executor::block_on;
use std::thread;

pub trait RunInBackgroundExt {
    fn run_in_background(self);
}

impl<F> RunInBackgroundExt for F
    where F: Future + Sized + Send + 'static,
          F::Output: Send,
{
    fn run_in_background(self) {
        thread::spawn(|| block_on(self));
    }
}
