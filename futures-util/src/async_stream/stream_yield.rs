/// Yield an item from an `#[async_stream]` function.
///
/// # Examples
///
/// ```no-run
/// #![feature(futures_api, generators)]
/// use futures::prelude::*;
/// use futures::executor::block_on;
/// use futures::{async_stream, stream_yield};
///
/// #[async_stream]
/// fn one_five() -> u32 {
///     stream_yield!(1);
///     stream_yield!(5);
/// }
///
/// assert_eq!(vec![1, 5], block_on(one_five().collect::<Vec<_>>()));
/// ```
#[macro_export]
macro_rules! stream_yield {
    ($e:expr) => {{
        yield $crate::core_reexport::task::Poll::Ready($e)
    }}
}
