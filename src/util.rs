use {Future, Task, Poll};

/// A helpful structure for representing a future that is collapsed over time.
///
/// This basically implements `Future` without literally implementing the trait,
/// and the primary `collapse` method will perform tail call optimization
/// except that it won't propagate upwards, but rather store the result
/// immediately.
pub enum Collapsed<T: Future> {
    Start(T),
    Tail(Box<Future<Item=T::Item, Error=T::Error>>),
}

unsafe impl<T: Send + Future> Send for Collapsed<T> {}
unsafe impl<T: Sync + Future> Sync for Collapsed<T> {}

impl<T: Future> Collapsed<T> {
    pub fn poll(&mut self, task: &mut Task) -> Poll<T::Item, T::Error> {
        match *self {
            Collapsed::Start(ref mut a) => a.poll(task),
            Collapsed::Tail(ref mut a) => a.poll(task),
        }
    }

    pub fn schedule(&mut self, task: &mut Task) {
        match *self {
            Collapsed::Start(ref mut a) => a.schedule(task),
            Collapsed::Tail(ref mut a) => a.schedule(task),
        }
    }

    pub unsafe fn collapse(&mut self) {
        let a = match *self {
            Collapsed::Start(ref mut a) => {
                match a.tailcall() {
                    Some(a) => a,
                    None => return,
                }
            }
            Collapsed::Tail(ref mut a) => {
                if let Some(b) = a.tailcall() {
                    *a = b;
                }
                return
            }
        };
        *self = Collapsed::Tail(a);
    }
}
