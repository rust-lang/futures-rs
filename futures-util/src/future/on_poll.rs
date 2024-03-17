use super::assert_future;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`on_poll`] function.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    #[derive(Debug)]
    pub struct OnPoll<T, F> {
        #[pin]
        future: Option<T>,
        function: Option<F>,
    }
}

/// Wrapps a future and a function in an [`OnPoll`]. On each [poll](core::future::Future::poll())
/// the wrapped future is polled internally and the function is invoked.
///
/// The function provides mutable access to the future's [`Context`] and to its [`Poll`].
///
/// After the internal future has returned [`Poll::Ready`] additional polls to an [`OnPoll`]
/// will lead to a panic.
///
/// **Warning:** You should only call the future's waker in this function when it is safe to
/// poll the future another time. Otherwise this might lead to a panic.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future::on_poll;
/// use futures::pending;
/// use futures::task::{Context, Poll};
/// let mut x = 0;
/// let future = async {
///     pending!(); // returns Poll::Pending, so x = 1
///     pending!(); // returns Poll::Pending, so x = 2
///     pending!(); // returns Poll::Pending, so x = 3
///     "Some future" // returns Poll::Ready, so x = 4
/// };
/// futures::pin_mut!(future);
/// let funct = |cx : &mut Context<'_>, poll : &mut Poll<_>| {
///     x += 1;
///     if poll.is_pending() {
///         cx.waker().wake_by_ref(); // only to let the future make progress
///     }
/// };
/// let res = on_poll(future, funct).await;
/// assert_eq!(x, 4);
/// assert_eq!(res, "Some future");
/// # })
/// ```
pub fn on_poll<T, F>(future: T, function: F) -> OnPoll<T, F>
where
    T: Future,
    F: FnMut(&mut Context<'_>, &mut Poll<T::Output>),
{
    assert_future::<T::Output, _>(OnPoll { future: Some(future), function: Some(function) })
}

impl<T, F> Future for OnPoll<T, F>
where
    T: Future,
    F: FnMut(&mut Context<'_>, &mut Poll<T::Output>),
{
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let future =
            this.future.as_mut().as_pin_mut().expect("cannot poll OnPoll after it was ready");

        let mut poll = future.poll(cx);
        if poll.is_ready() {
            this.future.set(None);
            unwrap_option(this.function.take())(cx, &mut poll);
            return poll;
        }

        unwrap_option(this.function.as_mut())(cx, &mut poll);
        poll
    }
}

impl<T, F> FusedFuture for OnPoll<T, F>
where
    T: Future,
    F: FnMut(&mut Context<'_>, &mut Poll<T::Output>),
{
    fn is_terminated(&self) -> bool {
        self.future.is_none()
    }
}

pin_project! {
    /// Future for the [`on_poll_pending`] function.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    #[derive(Debug)]
    pub struct OnPollPending<T, F> {
        #[pin]
        future: Option<T>,
        function: Option<F>,
    }
}

/// Wrapps a future and a function in an [`OnPollPending`]. On each [poll](core::future::Future::poll())
/// the wrapped future is polled internally. The function is invoked each time the internal future returns
/// [`Poll::Pending`].
///
/// The function provides mutable access to the future's [`Context`] and to its [`Poll`].
///
/// After the internal future has returned [`Poll::Ready`] additional polls to an [`OnPollPending`]
/// will lead to a panic.
///
/// Can be used to abort a future after a specific number of pending polls by changing its [`Poll`]
/// to [`Poll::Ready`].
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future::on_poll_pending;
/// use futures::pending;
/// use futures::task::{Context, Poll};
/// let mut x = 0;
/// let future = async {
///     pending!(); // returns Poll::Pending ,so x = 1
///     pending!(); // returns Poll::Pending, so x = 2
///     pending!(); // returns Poll::Pending, so x = 3
///     "Some future" // returns Poll::Ready, so x is unchanged
/// };
/// futures::pin_mut!(future);
/// let funct = |cx : &mut Context<'_>, poll : &mut Poll<_>| {
///     *&mut x += 1;
///     cx.waker().wake_by_ref(); // only to let the future make progress
/// };
/// let res = on_poll_pending(future, funct).await;
/// assert_eq!(x, 3);
/// assert_eq!(res, "Some future");
/// # })
/// ```
pub fn on_poll_pending<T, F>(future: T, function: F) -> OnPollPending<T, F>
where
    T: Future,
    F: FnMut(&mut Context<'_>, &mut Poll<T::Output>),
{
    assert_future::<T::Output, _>(OnPollPending { future: Some(future), function: Some(function) })
}

impl<T, F> Future for OnPollPending<T, F>
where
    T: Future,
    F: FnMut(&mut Context<'_>, &mut Poll<T::Output>),
{
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let future = this
            .future
            .as_mut()
            .as_pin_mut()
            .expect("cannot poll OnPollPending after it was ready");

        let mut poll = future.poll(cx);
        if poll.is_ready() {
            this.future.set(None);
            unwrap_option(this.function.take());
            return poll;
        }

        unwrap_option(this.function.as_mut())(cx, &mut poll);
        // Function was called so we need to check if the poll was manipulated to Poll::Ready
        if poll.is_ready() {
            this.future.set(None);
            unwrap_option(this.function.take());
        }
        poll
    }
}

impl<T, F> FusedFuture for OnPollPending<T, F>
where
    T: Future,
    F: FnMut(&mut Context<'_>, &mut Poll<T::Output>),
{
    fn is_terminated(&self) -> bool {
        self.future.is_none()
    }
}

pin_project! {
    /// Future for the [`on_poll_ready`] function.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    #[derive(Debug)]
    pub struct OnPollReady<T, F> {
        #[pin]
        future: Option<T>,
        function: Option<F>,
    }
}

/// Wrapps a future and a function in an [`OnPollReady`]. On each [poll](core::future::Future::poll())
/// the wrapped future is polled internally. The function is invoked when the internal future returns
/// [`Poll::Ready`].
///
/// The function provides mutable access to the future's [`Context`] and to its [`Output`](core::future::Future::Output).
///
/// After the internal future has returned [`Poll::Ready`] additional polls to an [`OnPollReady`]
/// will lead to a panic.
///
/// **Warning:** You should not call the waker in this function as this might lead to
/// another poll of the future. This will lead to a panic!
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::future::on_poll_ready;
/// let mut x = 0;
/// let future = async {"Some future"};
/// futures::pin_mut!(future);
///
/// let res = on_poll_ready(future, |_, _| *&mut x += 1).await;
/// assert_eq!(x, 1);
/// assert_eq!(res, "Some future");
/// # })
/// ```
pub fn on_poll_ready<T, F>(future: T, function: F) -> OnPollReady<T, F>
where
    T: Future,
    F: FnOnce(&mut Context<'_>, &mut T::Output),
{
    assert_future::<T::Output, _>(OnPollReady { future: Some(future), function: Some(function) })
}

impl<T, F> Future for OnPollReady<T, F>
where
    T: Future,
    F: FnOnce(&mut Context<'_>, &mut T::Output),
{
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let future =
            this.future.as_mut().as_pin_mut().expect("cannot poll OnPollReady after it was ready");

        if let Poll::Ready(mut val) = future.poll(cx) {
            this.future.set(None);
            unwrap_option(this.function.take())(cx, &mut val);
            return Poll::Ready(val);
        }

        Poll::Pending
    }
}

impl<T, F> FusedFuture for OnPollReady<T, F>
where
    T: Future,
    F: FnMut(&mut Context<'_>, &mut T::Output),
{
    fn is_terminated(&self) -> bool {
        self.future.is_none()
    }
}

/// When compiled with `-C opt-level=z`, this function will help the
/// compiler eliminate the `None` branch, where `Option::unwrap` does not.
#[inline(always)]
fn unwrap_option<T>(value: Option<T>) -> T {
    match value {
        None => unreachable!(),
        Some(value) => value,
    }
}
