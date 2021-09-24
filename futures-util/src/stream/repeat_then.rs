use core::marker::PhantomData;
use core::pin::Pin;

use crate::stream::assert_stream;
use crate::task::{Context, Poll};
use crate::{Future, Stream};

/// Stream for the [`repeat_then`] function.
#[derive(Debug)]
pub struct RepeatThen<'a, OUT, INPUT, FUT: Future<Output = OUT>, GEN: Fn(&'a mut INPUT) -> FUT> {
    /// The generator function for a future
    gen: GEN,

    /// fut references input, so this [`RepeatThen`] is ![`Unpin`]. Since also could contain
    /// _any_ [Future], it is also implied to be !Unpin.
    fut: Option<FUT>,

    /// Invariant: we must only give a mutable reference of input to one future at a time.
    input: INPUT,

    _phantom: PhantomData<&'a INPUT>,
}

unsafe fn to_static<'b, T>(reference: &mut T) -> &'b mut T {
    core::mem::transmute(reference)
}

impl<'a, OUT, INPUT: 'a, FUT: Future<Output = OUT>, GEN: Fn(&'a mut INPUT) -> FUT> Stream
    for RepeatThen<'a, OUT, INPUT, FUT, GEN>
{
    type Item = OUT;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // This is safe because we are upholding invariants. Even though this struct is
        // not Unpin i.e.
        // (1) `fut` referencing `input`,
        // (2) `fut` being a generic future which could be !Unpin
        // we will make sure to
        // (1) not move/change `input` while `fut` is referencing it
        // (2) not move `fut` while it is in use
        // we will also need to make sure to not move the whole struct, but this is guarenteed
        // since we receive a Pin
        let this = unsafe { self.get_unchecked_mut() };

        // This is to coerce lifetimes.
        // We need to coerce this because Pin<&mut self>
        //                                   ^^^^^^^^^
        //                                   is anonymous so might not be 'a.
        //
        // However, we ignore this—Self cannot be unpinned, so its mutable reference should exist
        // for its entire lifetime.
        let input_ref: &'a mut INPUT = unsafe { to_static(&mut this.input) };

        // We need to borrow outside get_or_insert_with or we get double mutable borrow
        let gen = &mut this.gen;

        // We need to move the mutable reference. If we did not, the Future would not have access
        // to it after this function ends
        let fut = this.fut.get_or_insert_with(move || gen(input_ref));

        // this is safe since fut does not move until it is dropped (in place of another future)
        let mut fut = unsafe { Pin::new_unchecked(fut) };

        fut.as_mut().poll(cx).map(|res| {
            this.fut = None;
            Some(res)
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::max_value(), None)
    }
}

impl<'a, OUT, INPUT, FUT: Future<Output = OUT>, GEN: Fn(&'a mut INPUT) -> FUT>
    RepeatThen<'a, OUT, INPUT, FUT, GEN>
{
    fn new(input: INPUT, gen: GEN) -> Self {
        Self { gen, fut: None, input, _phantom: PhantomData::default() }
    }
}

/// Turn a closure generating a future into an infinite stream which recreates and reruns
/// the future each time it polls [`Poll::Ready`].
///
/// All futures can use a mutable pointer to `input` to create. This is not possible with
/// `repeat` or `then`, as Rust ownership rules are not quite powerful enough.
///
/// # Why use repeat_then
/// - Cycle consumes input and yields a mutable reference to it for each closure.
/// - Input needs to be consumed in order to generate mutable references.
/// We might think instead of creating a closure, we could only accept an `FnMut() -> impl Future`.
/// However, this is not possible as any mutable references `FnMut` takes, `impl Future` contains a
/// reference to. Rust does not know if only one `impl Future` will be generated at a time.
/// Instead, [`repeat_then`] upholds this invariant when it consumes the input.
/// ```
/// # futures::executor::block_on(async {
/// #[derive(Default)]
/// struct State {
///     inner: i32
/// }
///
/// impl State {
///     async fn increment_and_return(&mut self) -> i32 {
///         self.inner+=1;
///         self.inner
///     }
/// }
///
/// use futures::stream::{self, StreamExt};
/// {
///     let stream = stream::repeat_then(State::default(), |state| state.increment_and_return());
///     assert_eq!(vec![1, 2, 3, 4, 5], stream.take(5).collect::<Vec<i32>>().await);
/// }
/// {
///     let mut state = State::default();
///     let stream = stream::repeat_then(&mut state, |state| state.increment_and_return());
///     assert_eq!(vec![1, 2, 3, 4, 5], stream.take(5).collect::<Vec<i32>>().await);
///     assert_eq!(5, state.inner);
///     assert_eq!(6, state.increment_and_return().await)
/// }
///
///
///
/// # });
/// ```
pub fn repeat_then<'a, OUT, INPUT: 'a, FUT: Future<Output = OUT>, GEN: Fn(&'a mut INPUT) -> FUT>(
    input: INPUT,
    gen: GEN,
) -> RepeatThen<'a, OUT, INPUT, FUT, GEN> {
    assert_stream::<FUT::Output, _>(RepeatThen::new(input, gen))
}
