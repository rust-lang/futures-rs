use {Future, Poll};
/// Combines two different futures yielding the same item and error
/// types into a single type.
pub enum Either<A, B> {
    /// First branch of the type
    A(A),
    /// Second branch of the type
    B(B),
}

impl<A, B, Item, Error> Future for Either<A, B>
    where A: Future<Item = Item, Error = Error>,
          B: Future<Item = Item, Error = Error>
{
    type Item = Item;
    type Error = Error;
    fn poll(&mut self) -> Poll<Item, Error> {
        match *self {
            Either::A(ref mut a) => a.poll(),
            Either::B(ref mut b) => b.poll(),
        }
    }
}
