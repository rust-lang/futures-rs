use futures_core::stream::Stream;

#[doc(hidden)]
pub fn assert_is_unpin_stream<S: Stream + Unpin>(_: &mut S) {}

/// Assert that the next poll to the provided stream will return
/// [`Poll::Pending`](futures_core::task::Poll::Pending).
///
/// # Examples
///
/// ```
/// use core::pin::pin;
///
/// use futures::stream;
/// use futures_test::future::FutureTestExt;
/// use futures_test::assert_stream_pending;
/// use futures_test::assert_stream_next;
/// use futures_test::assert_stream_done;
///
/// let stream = stream::once((async { 5 }).pending_once());
/// let mut stream = pin!(stream);
///
/// assert_stream_pending!(stream);
/// assert_stream_next!(stream, 5);
/// assert_stream_done!(stream);
/// ```
#[macro_export]
macro_rules! assert_stream_pending {
    ($stream:expr) => {
        #[allow(clippy::if_then_panic)]
        {
            let mut stream = &mut $stream;
            $crate::__private::assert::assert_is_unpin_stream(stream);
            let stream = $crate::__private::Pin::new(stream);
            let mut cx = $crate::task::noop_context();
            let poll = $crate::__private::stream::Stream::poll_next(stream, &mut cx);
            if poll.is_ready() {
                panic!("assertion failed: stream is not pending");
            }
        }
    };
}

/// Assert that the next poll to the provided stream will return
/// [`Poll::Ready`](futures_core::task::Poll::Ready) with the provided item.
///
/// # Examples
///
/// ```
/// use core::pin::pin;
///
/// use futures::stream;
/// use futures_test::future::FutureTestExt;
/// use futures_test::assert_stream_pending;
/// use futures_test::assert_stream_next;
/// use futures_test::assert_stream_done;
///
/// let stream = stream::once((async { 5 }).pending_once());
/// let mut stream = pin!(stream);
///
/// assert_stream_pending!(stream);
/// assert_stream_next!(stream, 5);
/// assert_stream_done!(stream);
/// ```
#[macro_export]
macro_rules! assert_stream_next {
    ($stream:expr, $item:expr) => {{
        let mut stream = &mut $stream;
        $crate::__private::assert::assert_is_unpin_stream(stream);
        let stream = $crate::__private::Pin::new(stream);
        let mut cx = $crate::task::noop_context();
        match $crate::__private::stream::Stream::poll_next(stream, &mut cx) {
            $crate::__private::task::Poll::Ready($crate::__private::Some(x)) => {
                assert_eq!(x, $item);
            }
            $crate::__private::task::Poll::Ready($crate::__private::None) => {
                panic!(
                    "assertion failed: expected stream to provide item but stream is at its end"
                );
            }
            $crate::__private::task::Poll::Pending => {
                panic!("assertion failed: expected stream to provide item but stream wasn't ready");
            }
        }
    }};
}

/// Assert that the next poll to the provided stream will return an empty
/// [`Poll::Ready`](futures_core::task::Poll::Ready) signalling the
/// completion of the stream.
///
/// # Examples
///
/// ```
/// use core::pin::pin;
///
/// use futures::stream;
/// use futures_test::future::FutureTestExt;
/// use futures_test::assert_stream_pending;
/// use futures_test::assert_stream_next;
/// use futures_test::assert_stream_done;
///
/// let stream = stream::once((async { 5 }).pending_once());
/// let mut stream = pin!(stream);
///
/// assert_stream_pending!(stream);
/// assert_stream_next!(stream, 5);
/// assert_stream_done!(stream);
/// ```
#[macro_export]
macro_rules! assert_stream_done {
    ($stream:expr) => {{
        let mut stream = &mut $stream;
        $crate::__private::assert::assert_is_unpin_stream(stream);
        let stream = $crate::__private::Pin::new(stream);
        let mut cx = $crate::task::noop_context();
        match $crate::__private::stream::Stream::poll_next(stream, &mut cx) {
            $crate::__private::task::Poll::Ready($crate::__private::Some(_)) => {
                panic!("assertion failed: expected stream to be done but had more elements");
            }
            $crate::__private::task::Poll::Ready($crate::__private::None) => {}
            $crate::__private::task::Poll::Pending => {
                panic!("assertion failed: expected stream to be done but was pending");
            }
        }
    }};
}
