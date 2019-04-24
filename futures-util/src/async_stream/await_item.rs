/// Await an item from a stream inside an `async` function.
///
/// You should pass an object implementing [`Stream`] to this macro, it will
/// implicitly `yield` while that stream returns [`Poll::Pending`] and evaluate
/// to a [`Result`] containing the next item or error from that stream.
///
/// If you want to iterate over all items in a `Stream` you should instead see
/// the documentation on `#[async] for` in the main `#[async]` documentation.
///
/// # Examples
///
/// ```no-run
/// #![feature(futures_api, async_await, generators)]
/// use futures::prelude::*;
/// use futures::executor::block_on;
/// use futures::await_item;
///
/// async fn eventually_ten() -> u32 {
///     let mut stream = stream::repeat(5);
///     if let Some(first) = await_item!(&mut stream) {
///         if let Some(second) = await_item!(&mut stream) {
///             return first + second;
///         }
///     }
///     0
/// }
///
/// assert_eq!(10, block_on(eventually_ten()));
/// ```
#[macro_export]
macro_rules! await_item {
    ($e:expr) => {{
        let mut pinned = $e;
        loop {
            if let $crate::core_reexport::task::Poll::Ready(x) =
                $crate::async_stream::poll_next_with_tls_context(unsafe {
                    $crate::core_reexport::pin::Pin::new_unchecked(&mut pinned)
                })
            {
                break x;
            }

            yield
        }
    }}
}
