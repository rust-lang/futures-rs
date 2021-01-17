//! Assert Send/Sync/Unpin for all public types

use futures::{
    future::Future,
    sink::Sink,
    stream::Stream,
    task::{Context, Poll},
};
use static_assertions::{assert_impl_all as assert_impl, assert_not_impl_all as assert_not_impl};
use std::marker::PhantomPinned;
use std::{marker::PhantomData, pin::Pin};

pub type LocalFuture<T = *const ()> = Pin<Box<dyn Future<Output = T>>>;
pub type LocalTryFuture<T = *const (), E = *const ()> = LocalFuture<Result<T, E>>;
pub type SendFuture<T = *const ()> = Pin<Box<dyn Future<Output = T> + Send>>;
pub type SendTryFuture<T = *const (), E = *const ()> = SendFuture<Result<T, E>>;
pub type SyncFuture<T = *const ()> = Pin<Box<dyn Future<Output = T> + Sync>>;
pub type SyncTryFuture<T = *const (), E = *const ()> = SyncFuture<Result<T, E>>;
pub type UnpinFuture<T = PhantomPinned> = LocalFuture<T>;
pub type UnpinTryFuture<T = PhantomPinned, E = PhantomPinned> = UnpinFuture<Result<T, E>>;
pub struct PinnedFuture<T = PhantomPinned>(PhantomPinned, PhantomData<T>);
impl<T> Future for PinnedFuture<T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        unimplemented!()
    }
}
pub type PinnedTryFuture<T = PhantomPinned, E = PhantomPinned> = PinnedFuture<Result<T, E>>;

pub type LocalStream<T = *const ()> = Pin<Box<dyn Stream<Item = T>>>;
pub type LocalTryStream<T = *const (), E = *const ()> = LocalStream<Result<T, E>>;
pub type SendStream<T = *const ()> = Pin<Box<dyn Stream<Item = T> + Send>>;
pub type SendTryStream<T = *const (), E = *const ()> = SendStream<Result<T, E>>;
pub type SyncStream<T = *const ()> = Pin<Box<dyn Stream<Item = T> + Sync>>;
pub type SyncTryStream<T = *const (), E = *const ()> = SyncStream<Result<T, E>>;
pub type UnpinStream<T = PhantomPinned> = LocalStream<T>;
pub type UnpinTryStream<T = PhantomPinned, E = PhantomPinned> = UnpinStream<Result<T, E>>;
pub struct PinnedStream<T = PhantomPinned>(PhantomPinned, PhantomData<T>);
impl<T> Stream for PinnedStream<T> {
    type Item = T;
    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        unimplemented!()
    }
}
pub type PinnedTryStream<T = PhantomPinned, E = PhantomPinned> = PinnedStream<Result<T, E>>;

pub type LocalSink<T = *const (), E = *const ()> = Pin<Box<dyn Sink<T, Error = E>>>;
pub type SendSink<T = *const (), E = *const ()> = Pin<Box<dyn Sink<T, Error = E> + Send>>;
pub type SyncSink<T = *const (), E = *const ()> = Pin<Box<dyn Sink<T, Error = E> + Sync>>;
pub type UnpinSink<T = PhantomPinned, E = PhantomPinned> = LocalSink<T, E>;
pub struct PinnedSink<T = PhantomPinned, E = PhantomPinned>(PhantomPinned, PhantomData<(T, E)>);
impl<T, E> Sink<T> for PinnedSink<T, E> {
    type Error = E;
    fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        unimplemented!()
    }
    fn start_send(self: Pin<&mut Self>, _: T) -> Result<(), Self::Error> {
        unimplemented!()
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        unimplemented!()
    }
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        unimplemented!()
    }
}

/// Assert Send/Sync/Unpin for all public types in `futures::channel`
pub mod channel {
    use super::*;
    use futures::channel::*;

    assert_impl!(mpsc::Receiver<()>: Send);
    assert_impl!(mpsc::Receiver<()>: Sync);
    assert_impl!(mpsc::Receiver<PhantomPinned>: Unpin);
    assert_not_impl!(mpsc::Receiver<*const ()>: Send);
    assert_not_impl!(mpsc::Receiver<*const ()>: Sync);

    assert_impl!(mpsc::SendError: Send);
    assert_impl!(mpsc::SendError: Sync);
    assert_impl!(mpsc::SendError: Unpin);

    assert_impl!(mpsc::Sender<()>: Send);
    assert_impl!(mpsc::Sender<()>: Sync);
    assert_impl!(mpsc::Sender<PhantomPinned>: Unpin);
    assert_not_impl!(mpsc::Sender<*const ()>: Send);
    assert_not_impl!(mpsc::Sender<*const ()>: Sync);

    assert_impl!(mpsc::TryRecvError: Send);
    assert_impl!(mpsc::TryRecvError: Sync);
    assert_impl!(mpsc::TryRecvError: Unpin);

    assert_impl!(mpsc::TrySendError<()>: Send);
    assert_impl!(mpsc::TrySendError<()>: Sync);
    assert_impl!(mpsc::TrySendError<()>: Unpin);
    assert_not_impl!(mpsc::TrySendError<*const ()>: Send);
    assert_not_impl!(mpsc::TrySendError<*const ()>: Sync);
    assert_not_impl!(mpsc::TrySendError<PhantomPinned>: Unpin);

    assert_impl!(mpsc::UnboundedReceiver<()>: Send);
    assert_impl!(mpsc::UnboundedReceiver<()>: Sync);
    assert_impl!(mpsc::UnboundedReceiver<PhantomPinned>: Unpin);
    assert_not_impl!(mpsc::UnboundedReceiver<*const ()>: Send);
    assert_not_impl!(mpsc::UnboundedReceiver<*const ()>: Sync);

    assert_impl!(mpsc::UnboundedReceiver<()>: Send);
    assert_impl!(mpsc::UnboundedReceiver<()>: Sync);
    assert_impl!(mpsc::UnboundedReceiver<PhantomPinned>: Unpin);
    assert_not_impl!(mpsc::UnboundedReceiver<*const ()>: Send);
    assert_not_impl!(mpsc::UnboundedReceiver<*const ()>: Sync);

    assert_impl!(oneshot::Canceled: Send);
    assert_impl!(oneshot::Canceled: Sync);
    assert_impl!(oneshot::Canceled: Unpin);

    assert_impl!(oneshot::Cancellation<()>: Send);
    assert_impl!(oneshot::Cancellation<()>: Sync);
    assert_impl!(oneshot::Cancellation<PhantomPinned>: Unpin);
    assert_not_impl!(oneshot::Cancellation<*const ()>: Send);
    assert_not_impl!(oneshot::Cancellation<*const ()>: Sync);

    assert_impl!(oneshot::Receiver<()>: Send);
    assert_impl!(oneshot::Receiver<()>: Sync);
    assert_impl!(oneshot::Receiver<PhantomPinned>: Unpin);
    assert_not_impl!(oneshot::Receiver<*const ()>: Send);
    assert_not_impl!(oneshot::Receiver<*const ()>: Sync);

    assert_impl!(oneshot::Sender<()>: Send);
    assert_impl!(oneshot::Sender<()>: Sync);
    assert_impl!(oneshot::Sender<PhantomPinned>: Unpin);
    assert_not_impl!(oneshot::Sender<*const ()>: Send);
    assert_not_impl!(oneshot::Sender<*const ()>: Sync);
}

/// Assert Send/Sync/Unpin for all public types in `futures::future`
pub mod future {
    use super::*;
    use futures::future::*;

    assert_impl!(AbortHandle: Send);
    assert_impl!(AbortHandle: Sync);
    assert_impl!(AbortHandle: Unpin);

    assert_impl!(AbortRegistration: Send);
    assert_impl!(AbortRegistration: Sync);
    assert_impl!(AbortRegistration: Unpin);

    assert_impl!(Abortable<SendFuture>: Send);
    assert_impl!(Abortable<SyncFuture>: Sync);
    assert_impl!(Abortable<UnpinFuture>: Unpin);
    assert_not_impl!(Abortable<LocalFuture>: Send);
    assert_not_impl!(Abortable<LocalFuture>: Sync);
    assert_not_impl!(Abortable<PinnedFuture>: Unpin);

    assert_impl!(Aborted: Send);
    assert_impl!(Aborted: Sync);
    assert_impl!(Aborted: Unpin);

    assert_impl!(AndThen<SendFuture, SendFuture, ()>: Send);
    assert_impl!(AndThen<SyncFuture, SyncFuture, ()>: Sync);
    assert_impl!(AndThen<UnpinFuture, UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(AndThen<SendFuture, LocalFuture, ()>: Send);
    assert_not_impl!(AndThen<LocalFuture, SendFuture, ()>: Send);
    assert_not_impl!(AndThen<SendFuture, SendFuture, *const ()>: Send);
    assert_not_impl!(AndThen<SyncFuture, LocalFuture, ()>: Sync);
    assert_not_impl!(AndThen<LocalFuture, SyncFuture, ()>: Sync);
    assert_not_impl!(AndThen<SyncFuture, SyncFuture, *const ()>: Sync);
    assert_not_impl!(AndThen<PinnedFuture, UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(AndThen<UnpinFuture, PinnedFuture, PhantomPinned>: Unpin);

    assert_impl!(CatchUnwind<SendFuture>: Send);
    assert_impl!(CatchUnwind<SyncFuture>: Sync);
    assert_impl!(CatchUnwind<UnpinFuture>: Unpin);
    assert_not_impl!(CatchUnwind<LocalFuture>: Send);
    assert_not_impl!(CatchUnwind<LocalFuture>: Sync);
    assert_not_impl!(CatchUnwind<PinnedFuture>: Unpin);

    assert_impl!(ErrInto<SendTryFuture, *const ()>: Send);
    assert_impl!(ErrInto<SyncTryFuture, *const ()>: Sync);
    assert_impl!(ErrInto<UnpinTryFuture, PhantomPinned>: Unpin);
    assert_not_impl!(ErrInto<LocalTryFuture, ()>: Send);
    assert_not_impl!(ErrInto<LocalTryFuture, ()>: Sync);
    assert_not_impl!(ErrInto<PinnedTryFuture, PhantomPinned>: Unpin);

    assert_impl!(Flatten<SendFuture<()>>: Send);
    assert_impl!(Flatten<SyncFuture<()>>: Sync);
    assert_impl!(Flatten<UnpinFuture<()>>: Unpin);
    assert_not_impl!(Flatten<LocalFuture>: Send);
    assert_not_impl!(Flatten<SendFuture>: Send);
    assert_not_impl!(Flatten<LocalFuture>: Sync);
    assert_not_impl!(Flatten<SyncFuture>: Sync);
    assert_not_impl!(Flatten<PinnedFuture>: Unpin);
    assert_not_impl!(Flatten<UnpinFuture>: Unpin);

    assert_impl!(FlattenSink<SendFuture, ()>: Send);
    assert_impl!(FlattenSink<SyncFuture, ()>: Sync);
    assert_impl!(FlattenSink<UnpinFuture, ()>: Unpin);
    assert_not_impl!(FlattenSink<SendFuture, *const ()>: Send);
    assert_not_impl!(FlattenSink<LocalFuture, ()>: Send);
    assert_not_impl!(FlattenSink<SyncFuture, *const ()>: Sync);
    assert_not_impl!(FlattenSink<LocalFuture, ()>: Sync);
    assert_not_impl!(FlattenSink<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(FlattenSink<PinnedFuture, ()>: Unpin);

    assert_impl!(FlattenStream<SendFuture<()>>: Send);
    assert_impl!(FlattenStream<SyncFuture<()>>: Sync);
    assert_impl!(FlattenStream<UnpinFuture<()>>: Unpin);
    assert_not_impl!(FlattenStream<LocalFuture>: Send);
    assert_not_impl!(FlattenStream<SendFuture>: Send);
    assert_not_impl!(FlattenStream<LocalFuture>: Sync);
    assert_not_impl!(FlattenStream<SyncFuture>: Sync);
    assert_not_impl!(FlattenStream<PinnedFuture>: Unpin);
    assert_not_impl!(FlattenStream<UnpinFuture>: Unpin);

    assert_impl!(Fuse<SendFuture>: Send);
    assert_impl!(Fuse<SyncFuture>: Sync);
    assert_impl!(Fuse<UnpinFuture>: Unpin);
    assert_not_impl!(Fuse<LocalFuture>: Send);
    assert_not_impl!(Fuse<LocalFuture>: Sync);
    assert_not_impl!(Fuse<PinnedFuture>: Unpin);

    assert_impl!(FutureObj<*const ()>: Send);
    assert_impl!(FutureObj<PhantomPinned>: Unpin);
    assert_not_impl!(FutureObj<()>: Sync);

    assert_impl!(Inspect<SendFuture, ()>: Send);
    assert_impl!(Inspect<SyncFuture, ()>: Sync);
    assert_impl!(Inspect<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(Inspect<SendFuture, *const ()>: Send);
    assert_not_impl!(Inspect<LocalFuture, ()>: Send);
    assert_not_impl!(Inspect<SyncFuture, *const ()>: Sync);
    assert_not_impl!(Inspect<LocalFuture, ()>: Sync);
    assert_not_impl!(Inspect<PhantomPinned, PhantomPinned>: Unpin);

    assert_impl!(InspectErr<SendFuture, ()>: Send);
    assert_impl!(InspectErr<SyncFuture, ()>: Sync);
    assert_impl!(InspectErr<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(InspectErr<SendFuture, *const ()>: Send);
    assert_not_impl!(InspectErr<LocalFuture, ()>: Send);
    assert_not_impl!(InspectErr<SyncFuture, *const ()>: Sync);
    assert_not_impl!(InspectErr<LocalFuture, ()>: Sync);
    assert_not_impl!(InspectErr<PhantomPinned, PhantomPinned>: Unpin);

    assert_impl!(InspectOk<SendFuture, ()>: Send);
    assert_impl!(InspectOk<SyncFuture, ()>: Sync);
    assert_impl!(InspectOk<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(InspectOk<SendFuture, *const ()>: Send);
    assert_not_impl!(InspectOk<LocalFuture, ()>: Send);
    assert_not_impl!(InspectOk<SyncFuture, *const ()>: Sync);
    assert_not_impl!(InspectOk<LocalFuture, ()>: Sync);
    assert_not_impl!(InspectOk<PhantomPinned, PhantomPinned>: Unpin);

    assert_impl!(IntoFuture<SendFuture>: Send);
    assert_impl!(IntoFuture<SyncFuture>: Sync);
    assert_impl!(IntoFuture<UnpinFuture>: Unpin);
    assert_not_impl!(IntoFuture<LocalFuture>: Send);
    assert_not_impl!(IntoFuture<LocalFuture>: Sync);
    assert_not_impl!(IntoFuture<PinnedFuture>: Unpin);

    assert_impl!(IntoStream<SendFuture>: Send);
    assert_impl!(IntoStream<SyncFuture>: Sync);
    assert_impl!(IntoStream<UnpinFuture>: Unpin);
    assert_not_impl!(IntoStream<LocalFuture>: Send);
    assert_not_impl!(IntoStream<LocalFuture>: Sync);
    assert_not_impl!(IntoStream<PinnedFuture>: Unpin);

    assert_impl!(Join<SendFuture<()>, SendFuture<()>>: Send);
    assert_impl!(Join<SyncFuture<()>, SyncFuture<()>>: Sync);
    assert_impl!(Join<UnpinFuture, UnpinFuture>: Unpin);
    assert_not_impl!(Join<SendFuture<()>, SendFuture>: Send);
    assert_not_impl!(Join<SendFuture, SendFuture<()>>: Send);
    assert_not_impl!(Join<SendFuture, LocalFuture>: Send);
    assert_not_impl!(Join<LocalFuture, SendFuture>: Send);
    assert_not_impl!(Join<SyncFuture<()>, SyncFuture>: Sync);
    assert_not_impl!(Join<SyncFuture, SyncFuture<()>>: Sync);
    assert_not_impl!(Join<SyncFuture, LocalFuture>: Sync);
    assert_not_impl!(Join<LocalFuture, SyncFuture>: Sync);
    assert_not_impl!(Join<PinnedFuture, UnpinFuture>: Unpin);
    assert_not_impl!(Join<UnpinFuture, PinnedFuture>: Unpin);

    // Join3, Join4, Join5 are the same as Join

    assert_impl!(JoinAll<SendFuture<()>>: Send);
    assert_impl!(JoinAll<SyncFuture<()>>: Sync);
    assert_impl!(JoinAll<PinnedFuture>: Unpin);
    assert_not_impl!(JoinAll<LocalFuture>: Send);
    assert_not_impl!(JoinAll<SendFuture>: Send);
    assert_not_impl!(JoinAll<LocalFuture>: Sync);
    assert_not_impl!(JoinAll<SyncFuture>: Sync);

    assert_impl!(Lazy<()>: Send);
    assert_impl!(Lazy<()>: Sync);
    assert_impl!(Lazy<PhantomPinned>: Unpin);
    assert_not_impl!(Lazy<*const ()>: Send);
    assert_not_impl!(Lazy<*const ()>: Sync);

    assert_impl!(LocalFutureObj<PhantomPinned>: Unpin);
    assert_not_impl!(LocalFutureObj<()>: Send);
    assert_not_impl!(LocalFutureObj<()>: Sync);

    assert_impl!(Map<SendFuture, ()>: Send);
    assert_impl!(Map<SyncFuture, ()>: Sync);
    assert_impl!(Map<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(Map<SendFuture, *const ()>: Send);
    assert_not_impl!(Map<LocalFuture, ()>: Send);
    assert_not_impl!(Map<SyncFuture, *const ()>: Sync);
    assert_not_impl!(Map<LocalFuture, ()>: Sync);
    assert_not_impl!(Map<PhantomPinned, ()>: Unpin);

    assert_impl!(MapErr<SendFuture, ()>: Send);
    assert_impl!(MapErr<SyncFuture, ()>: Sync);
    assert_impl!(MapErr<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(MapErr<SendFuture, *const ()>: Send);
    assert_not_impl!(MapErr<LocalFuture, ()>: Send);
    assert_not_impl!(MapErr<SyncFuture, *const ()>: Sync);
    assert_not_impl!(MapErr<LocalFuture, ()>: Sync);
    assert_not_impl!(MapErr<PhantomPinned, ()>: Unpin);

    assert_impl!(MapInto<SendFuture, *const ()>: Send);
    assert_impl!(MapInto<SyncFuture, *const ()>: Sync);
    assert_impl!(MapInto<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(MapInto<LocalFuture, ()>: Send);
    assert_not_impl!(MapInto<LocalFuture, ()>: Sync);
    assert_not_impl!(MapInto<PhantomPinned, ()>: Unpin);

    assert_impl!(MapOk<SendFuture, ()>: Send);
    assert_impl!(MapOk<SyncFuture, ()>: Sync);
    assert_impl!(MapOk<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(MapOk<SendFuture, *const ()>: Send);
    assert_not_impl!(MapOk<LocalFuture, ()>: Send);
    assert_not_impl!(MapOk<SyncFuture, *const ()>: Sync);
    assert_not_impl!(MapOk<LocalFuture, ()>: Sync);
    assert_not_impl!(MapOk<PhantomPinned, ()>: Unpin);

    assert_impl!(MapOkOrElse<SendFuture, (), ()>: Send);
    assert_impl!(MapOkOrElse<SyncFuture, (), ()>: Sync);
    assert_impl!(MapOkOrElse<UnpinFuture, PhantomPinned, PhantomPinned>: Unpin);
    assert_not_impl!(MapOkOrElse<SendFuture, (), *const ()>: Send);
    assert_not_impl!(MapOkOrElse<SendFuture, *const (), ()>: Send);
    assert_not_impl!(MapOkOrElse<LocalFuture, (), ()>: Send);
    assert_not_impl!(MapOkOrElse<SyncFuture, (), *const ()>: Sync);
    assert_not_impl!(MapOkOrElse<SyncFuture, *const (), ()>: Sync);
    assert_not_impl!(MapOkOrElse<LocalFuture, (), ()>: Sync);
    assert_not_impl!(MapOkOrElse<PhantomPinned, (), ()>: Unpin);

    assert_impl!(NeverError<SendFuture>: Send);
    assert_impl!(NeverError<SyncFuture>: Sync);
    assert_impl!(NeverError<UnpinFuture>: Unpin);
    assert_not_impl!(NeverError<LocalFuture>: Send);
    assert_not_impl!(NeverError<LocalFuture>: Sync);
    assert_not_impl!(NeverError<PinnedFuture>: Unpin);

    assert_impl!(OkInto<SendFuture, *const ()>: Send);
    assert_impl!(OkInto<SyncFuture, *const ()>: Sync);
    assert_impl!(OkInto<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(OkInto<LocalFuture, ()>: Send);
    assert_not_impl!(OkInto<LocalFuture, ()>: Sync);
    assert_not_impl!(OkInto<PhantomPinned, ()>: Unpin);

    assert_impl!(OptionFuture<SendFuture>: Send);
    assert_impl!(OptionFuture<SyncFuture>: Sync);
    assert_impl!(OptionFuture<UnpinFuture>: Unpin);
    assert_not_impl!(OptionFuture<LocalFuture>: Send);
    assert_not_impl!(OptionFuture<LocalFuture>: Sync);
    assert_not_impl!(OptionFuture<PinnedFuture>: Unpin);

    assert_impl!(OrElse<SendFuture, SendFuture, ()>: Send);
    assert_impl!(OrElse<SyncFuture, SyncFuture, ()>: Sync);
    assert_impl!(OrElse<UnpinFuture, UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(OrElse<SendFuture, LocalFuture, ()>: Send);
    assert_not_impl!(OrElse<LocalFuture, SendFuture, ()>: Send);
    assert_not_impl!(OrElse<SendFuture, SendFuture, *const ()>: Send);
    assert_not_impl!(OrElse<SyncFuture, LocalFuture, ()>: Sync);
    assert_not_impl!(OrElse<LocalFuture, SyncFuture, ()>: Sync);
    assert_not_impl!(OrElse<SyncFuture, SyncFuture, *const ()>: Sync);
    assert_not_impl!(OrElse<PinnedFuture, UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(OrElse<UnpinFuture, PinnedFuture, PhantomPinned>: Unpin);

    assert_impl!(Pending<()>: Send);
    assert_impl!(Pending<()>: Sync);
    assert_impl!(Pending<PhantomPinned>: Unpin);
    assert_not_impl!(Pending<*const ()>: Send);
    assert_not_impl!(Pending<*const ()>: Sync);

    assert_impl!(PollFn<()>: Send);
    assert_impl!(PollFn<()>: Sync);
    assert_impl!(PollFn<PhantomPinned>: Unpin);
    assert_not_impl!(PollFn<*const ()>: Send);
    assert_not_impl!(PollFn<*const ()>: Sync);

    assert_impl!(Ready<()>: Send);
    assert_impl!(Ready<()>: Sync);
    assert_impl!(Ready<PhantomPinned>: Unpin);
    assert_not_impl!(Ready<*const ()>: Send);
    assert_not_impl!(Ready<*const ()>: Sync);

    assert_impl!(Remote<SendFuture<()>>: Send);
    assert_impl!(Remote<SyncFuture<()>>: Sync);
    assert_impl!(Remote<UnpinFuture>: Unpin);
    assert_not_impl!(Remote<LocalFuture>: Send);
    assert_not_impl!(Remote<SendFuture>: Send);
    assert_not_impl!(Remote<LocalFuture>: Sync);
    assert_not_impl!(Remote<SyncFuture>: Sync);
    assert_not_impl!(Remote<PinnedFuture>: Unpin);

    assert_impl!(RemoteHandle<()>: Send);
    assert_impl!(RemoteHandle<()>: Sync);
    assert_impl!(RemoteHandle<PhantomPinned>: Unpin);
    assert_not_impl!(RemoteHandle<*const ()>: Send);
    assert_not_impl!(RemoteHandle<*const ()>: Sync);

    assert_impl!(Select<SendFuture, SendFuture>: Send);
    assert_impl!(Select<SyncFuture, SyncFuture>: Sync);
    assert_impl!(Select<UnpinFuture, UnpinFuture>: Unpin);
    assert_not_impl!(Select<SendFuture, LocalFuture>: Send);
    assert_not_impl!(Select<LocalFuture, SendFuture>: Send);
    assert_not_impl!(Select<SyncFuture, LocalFuture>: Sync);
    assert_not_impl!(Select<LocalFuture, SyncFuture>: Sync);
    assert_not_impl!(Select<PinnedFuture, UnpinFuture>: Unpin);
    assert_not_impl!(Select<UnpinFuture, PinnedFuture>: Unpin);

    assert_impl!(SelectAll<SendFuture>: Send);
    assert_impl!(SelectAll<SyncFuture>: Sync);
    assert_impl!(SelectAll<UnpinFuture>: Unpin);
    assert_not_impl!(SelectAll<LocalFuture>: Send);
    assert_not_impl!(SelectAll<LocalFuture>: Sync);
    assert_not_impl!(SelectAll<PinnedFuture>: Unpin);

    assert_impl!(SelectOk<SendFuture>: Send);
    assert_impl!(SelectOk<SyncFuture>: Sync);
    assert_impl!(SelectOk<UnpinFuture>: Unpin);
    assert_not_impl!(SelectOk<LocalFuture>: Send);
    assert_not_impl!(SelectOk<LocalFuture>: Sync);
    assert_not_impl!(SelectOk<PinnedFuture>: Unpin);

    assert_impl!(Shared<SendFuture<()>>: Send);
    assert_impl!(Shared<PinnedFuture>: Unpin);
    assert_not_impl!(Shared<SendFuture>: Send);
    assert_not_impl!(Shared<LocalFuture>: Send);
    assert_not_impl!(Shared<SyncFuture<()>>: Sync);

    assert_impl!(Then<SendFuture, SendFuture, ()>: Send);
    assert_impl!(Then<SyncFuture, SyncFuture, ()>: Sync);
    assert_impl!(Then<UnpinFuture, UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(Then<SendFuture, SendFuture, *const ()>: Send);
    assert_not_impl!(Then<SendFuture, LocalFuture, ()>: Send);
    assert_not_impl!(Then<LocalFuture, SendFuture, ()>: Send);
    assert_not_impl!(Then<SyncFuture, SyncFuture, *const ()>: Sync);
    assert_not_impl!(Then<SyncFuture, LocalFuture, ()>: Sync);
    assert_not_impl!(Then<LocalFuture, SyncFuture, ()>: Sync);
    assert_not_impl!(Then<PinnedFuture, UnpinFuture, ()>: Unpin);
    assert_not_impl!(Then<UnpinFuture, PinnedFuture, ()>: Unpin);

    assert_impl!(TryFlatten<SendTryFuture<()>, ()>: Send);
    assert_impl!(TryFlatten<SyncTryFuture<()>, ()>: Sync);
    assert_impl!(TryFlatten<UnpinTryFuture<()>, ()>: Unpin);
    assert_not_impl!(TryFlatten<LocalTryFuture, ()>: Send);
    assert_not_impl!(TryFlatten<SendTryFuture, *const ()>: Send);
    assert_not_impl!(TryFlatten<LocalTryFuture, ()>: Sync);
    assert_not_impl!(TryFlatten<SyncTryFuture, *const ()>: Sync);
    assert_not_impl!(TryFlatten<PinnedTryFuture, ()>: Unpin);
    assert_not_impl!(TryFlatten<UnpinTryFuture, PhantomPinned>: Unpin);

    assert_impl!(TryFlattenStream<SendTryFuture<()>>: Send);
    assert_impl!(TryFlattenStream<SyncTryFuture<()>>: Sync);
    assert_impl!(TryFlattenStream<UnpinTryFuture<()>>: Unpin);
    assert_not_impl!(TryFlattenStream<LocalTryFuture>: Send);
    assert_not_impl!(TryFlattenStream<SendTryFuture>: Send);
    assert_not_impl!(TryFlattenStream<LocalTryFuture>: Sync);
    assert_not_impl!(TryFlattenStream<SyncTryFuture>: Sync);
    assert_not_impl!(TryFlattenStream<PinnedTryFuture>: Unpin);
    assert_not_impl!(TryFlattenStream<UnpinTryFuture>: Unpin);

    assert_impl!(TryJoin<SendTryFuture<()>, SendTryFuture<()>>: Send);
    assert_impl!(TryJoin<SyncTryFuture<()>, SyncTryFuture<()>>: Sync);
    assert_impl!(TryJoin<UnpinTryFuture, UnpinTryFuture>: Unpin);
    assert_not_impl!(TryJoin<SendTryFuture<()>, SendTryFuture>: Send);
    assert_not_impl!(TryJoin<SendTryFuture, SendTryFuture<()>>: Send);
    assert_not_impl!(TryJoin<SendTryFuture, LocalTryFuture>: Send);
    assert_not_impl!(TryJoin<LocalTryFuture, SendTryFuture>: Send);
    assert_not_impl!(TryJoin<SyncTryFuture<()>, SyncTryFuture>: Sync);
    assert_not_impl!(TryJoin<SyncTryFuture, SyncTryFuture<()>>: Sync);
    assert_not_impl!(TryJoin<SyncTryFuture, LocalTryFuture>: Sync);
    assert_not_impl!(TryJoin<LocalTryFuture, SyncTryFuture>: Sync);
    assert_not_impl!(TryJoin<PinnedTryFuture, UnpinTryFuture>: Unpin);
    assert_not_impl!(TryJoin<UnpinTryFuture, PinnedTryFuture>: Unpin);

    // TryJoin3, TryJoin4, TryJoin5 are the same as TryJoin

    assert_impl!(TryJoinAll<SendTryFuture<()>>: Send);
    assert_impl!(TryJoinAll<SyncTryFuture<()>>: Sync);
    assert_impl!(TryJoinAll<PinnedTryFuture>: Unpin);
    assert_not_impl!(TryJoinAll<LocalTryFuture>: Send);
    assert_not_impl!(TryJoinAll<SendTryFuture>: Send);
    assert_not_impl!(TryJoinAll<LocalTryFuture>: Sync);
    assert_not_impl!(TryJoinAll<SyncTryFuture>: Sync);

    assert_impl!(TrySelect<SendFuture, SendFuture>: Send);
    assert_impl!(TrySelect<SyncFuture, SyncFuture>: Sync);
    assert_impl!(TrySelect<UnpinFuture, UnpinFuture>: Unpin);
    assert_not_impl!(TrySelect<SendFuture, LocalFuture>: Send);
    assert_not_impl!(TrySelect<LocalFuture, SendFuture>: Send);
    assert_not_impl!(TrySelect<SyncFuture, LocalFuture>: Sync);
    assert_not_impl!(TrySelect<LocalFuture, SyncFuture>: Sync);
    assert_not_impl!(TrySelect<PinnedFuture, UnpinFuture>: Unpin);
    assert_not_impl!(TrySelect<UnpinFuture, PinnedFuture>: Unpin);

    assert_impl!(UnitError<SendFuture>: Send);
    assert_impl!(UnitError<SyncFuture>: Sync);
    assert_impl!(UnitError<UnpinFuture>: Unpin);
    assert_not_impl!(UnitError<LocalFuture>: Send);
    assert_not_impl!(UnitError<LocalFuture>: Sync);
    assert_not_impl!(UnitError<PinnedFuture>: Unpin);

    assert_impl!(UnwrapOrElse<SendFuture, ()>: Send);
    assert_impl!(UnwrapOrElse<SyncFuture, ()>: Sync);
    assert_impl!(UnwrapOrElse<UnpinFuture, PhantomPinned>: Unpin);
    assert_not_impl!(UnwrapOrElse<SendFuture, *const ()>: Send);
    assert_not_impl!(UnwrapOrElse<LocalFuture, ()>: Send);
    assert_not_impl!(UnwrapOrElse<SyncFuture, *const ()>: Sync);
    assert_not_impl!(UnwrapOrElse<LocalFuture, ()>: Sync);
    assert_not_impl!(UnwrapOrElse<PhantomPinned, ()>: Unpin);

    assert_impl!(WeakShared<SendFuture<()>>: Send);
    assert_impl!(WeakShared<PinnedFuture>: Unpin);
    assert_not_impl!(WeakShared<SendFuture>: Send);
    assert_not_impl!(WeakShared<LocalFuture>: Send);
    assert_not_impl!(WeakShared<SyncFuture<()>>: Sync);

    assert_impl!(Either<SendFuture, SendFuture>: Send);
    assert_impl!(Either<SyncFuture, SyncFuture>: Sync);
    assert_impl!(Either<UnpinFuture, UnpinFuture>: Unpin);
    assert_not_impl!(Either<SendFuture, LocalFuture>: Send);
    assert_not_impl!(Either<LocalFuture, SendFuture>: Send);
    assert_not_impl!(Either<SyncFuture, LocalFuture>: Sync);
    assert_not_impl!(Either<LocalFuture, SyncFuture>: Sync);
    assert_not_impl!(Either<UnpinFuture, PinnedFuture>: Unpin);
    assert_not_impl!(Either<PinnedFuture, UnpinFuture>: Unpin);

    assert_impl!(MaybeDone<SendFuture<()>>: Send);
    assert_impl!(MaybeDone<SyncFuture<()>>: Sync);
    assert_impl!(MaybeDone<UnpinFuture>: Unpin);
    assert_not_impl!(MaybeDone<SendFuture>: Send);
    assert_not_impl!(MaybeDone<LocalFuture>: Send);
    assert_not_impl!(MaybeDone<SyncFuture>: Sync);
    assert_not_impl!(MaybeDone<LocalFuture>: Sync);
    assert_not_impl!(MaybeDone<PinnedFuture>: Unpin);

    assert_impl!(TryMaybeDone<SendTryFuture<()>>: Send);
    assert_impl!(TryMaybeDone<SyncTryFuture<()>>: Sync);
    assert_impl!(TryMaybeDone<UnpinTryFuture>: Unpin);
    assert_not_impl!(TryMaybeDone<SendTryFuture>: Send);
    assert_not_impl!(TryMaybeDone<LocalTryFuture>: Send);
    assert_not_impl!(TryMaybeDone<SyncTryFuture>: Sync);
    assert_not_impl!(TryMaybeDone<LocalTryFuture>: Sync);
    assert_not_impl!(TryMaybeDone<PinnedTryFuture>: Unpin);
}
