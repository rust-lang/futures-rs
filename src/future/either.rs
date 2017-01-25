use {Future, Poll};
use task::Task;

/// Combines two different futures yielding the same item and error
/// types into a single type.
pub enum Either<A, B> {
    /// First branch of the type
    A(A),
    /// Second branch of the type
    B(B),
}

impl<A, B> Future for Either<A, B>
    where A: Future,
          B: Future<Item = A::Item, Error = A::Error>
{
    type Item = A::Item;
    type Error = A::Error;

    fn poll(&mut self, task: &Task) -> Poll<A::Item, A::Error> {
         match *self {
            Either::A(ref mut a) => a.poll(task),
            Either::B(ref mut b) => b.poll(task),
        }
    }
}
