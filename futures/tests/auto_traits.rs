//! Assert Send/Sync/Unpin for all public types.

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

/// Assert Send/Sync/Unpin for all public types in `futures::channel`.
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

/// Assert Send/Sync/Unpin for all public types in `futures::compat`.
pub mod compat {
    use super::*;
    use futures::compat::*;

    assert_impl!(Compat<()>: Send);
    assert_impl!(Compat<()>: Sync);
    assert_impl!(Compat<()>: Unpin);
    assert_not_impl!(Compat<*const ()>: Send);
    assert_not_impl!(Compat<*const ()>: Sync);
    assert_not_impl!(Compat<PhantomPinned>: Unpin);

    assert_impl!(Compat01As03<()>: Send);
    assert_impl!(Compat01As03<PhantomPinned>: Unpin);
    assert_not_impl!(Compat01As03<*const ()>: Send);
    assert_not_impl!(Compat01As03<()>: Sync);

    assert_impl!(Compat01As03Sink<(), ()>: Send);
    assert_impl!(Compat01As03Sink<PhantomPinned, PhantomPinned>: Unpin);
    assert_not_impl!(Compat01As03Sink<(), *const ()>: Send);
    assert_not_impl!(Compat01As03Sink<*const (), ()>: Send);
    assert_not_impl!(Compat01As03Sink<(), ()>: Sync);

    assert_impl!(CompatSink<(), *const ()>: Send);
    assert_impl!(CompatSink<(), *const ()>: Sync);
    assert_impl!(CompatSink<(), PhantomPinned>: Unpin);
    assert_not_impl!(CompatSink<*const (), ()>: Send);
    assert_not_impl!(CompatSink<*const (), ()>: Sync);
    assert_not_impl!(CompatSink<PhantomPinned, ()>: Unpin);

    assert_impl!(Executor01As03<()>: Send);
    assert_impl!(Executor01As03<()>: Sync);
    assert_impl!(Executor01As03<()>: Unpin);
    assert_not_impl!(Executor01As03<*const ()>: Send);
    assert_not_impl!(Executor01As03<*const ()>: Sync);
    assert_not_impl!(Executor01As03<PhantomPinned>: Unpin);

    assert_impl!(Executor01Future: Send);
    assert_impl!(Executor01Future: Unpin);
    assert_not_impl!(Executor01Future: Sync);
}

/// Assert Send/Sync/Unpin for all public types in `futures::executor`.
pub mod executor {
    use super::*;
    use futures::executor::*;

    assert_impl!(BlockingStream<SendStream>: Send);
    assert_impl!(BlockingStream<SyncStream>: Sync);
    assert_impl!(BlockingStream<UnpinStream>: Unpin);
    assert_not_impl!(BlockingStream<LocalStream>: Send);
    assert_not_impl!(BlockingStream<LocalStream>: Sync);
    // BlockingStream requires `S: Unpin`
    // assert_not_impl!(BlockingStream<PinnedStream>: Unpin);

    assert_impl!(Enter: Send);
    assert_impl!(Enter: Sync);
    assert_impl!(Enter: Unpin);

    assert_impl!(EnterError: Send);
    assert_impl!(EnterError: Sync);
    assert_impl!(EnterError: Unpin);

    assert_impl!(LocalPool: Unpin);
    assert_not_impl!(LocalPool: Send);
    assert_not_impl!(LocalPool: Sync);

    assert_impl!(LocalSpawner: Unpin);
    assert_not_impl!(LocalSpawner: Send);
    assert_not_impl!(LocalSpawner: Sync);

    assert_impl!(ThreadPool: Send);
    assert_impl!(ThreadPool: Sync);
    assert_impl!(ThreadPool: Unpin);

    assert_impl!(ThreadPoolBuilder: Send);
    assert_impl!(ThreadPoolBuilder: Sync);
    assert_impl!(ThreadPoolBuilder: Unpin);
}

/// Assert Send/Sync/Unpin for all public types in `futures::future`.
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

/// Assert Send/Sync/Unpin for all public types in `futures::io`.
pub mod io {
    use super::*;
    use futures::io::{Sink, *};

    assert_impl!(AllowStdIo<()>: Send);
    assert_impl!(AllowStdIo<()>: Sync);
    assert_impl!(AllowStdIo<PhantomPinned>: Unpin);
    assert_not_impl!(AllowStdIo<*const ()>: Send);
    assert_not_impl!(AllowStdIo<*const ()>: Sync);

    assert_impl!(BufReader<()>: Send);
    assert_impl!(BufReader<()>: Sync);
    assert_impl!(BufReader<()>: Unpin);
    assert_not_impl!(BufReader<*const ()>: Send);
    assert_not_impl!(BufReader<*const ()>: Sync);
    assert_not_impl!(BufReader<PhantomPinned>: Unpin);

    assert_impl!(BufWriter<()>: Send);
    assert_impl!(BufWriter<()>: Sync);
    assert_impl!(BufWriter<()>: Unpin);
    assert_not_impl!(BufWriter<*const ()>: Send);
    assert_not_impl!(BufWriter<*const ()>: Sync);
    assert_not_impl!(BufWriter<PhantomPinned>: Unpin);

    assert_impl!(Chain<(), ()>: Send);
    assert_impl!(Chain<(), ()>: Sync);
    assert_impl!(Chain<(), ()>: Unpin);
    assert_not_impl!(Chain<(), *const ()>: Send);
    assert_not_impl!(Chain<*const (), ()>: Send);
    assert_not_impl!(Chain<(), *const ()>: Sync);
    assert_not_impl!(Chain<*const (), ()>: Sync);
    assert_not_impl!(Chain<(), PhantomPinned>: Unpin);
    assert_not_impl!(Chain<PhantomPinned, ()>: Unpin);

    assert_impl!(Close<'_, ()>: Send);
    assert_impl!(Close<'_, ()>: Sync);
    assert_impl!(Close<'_, ()>: Unpin);
    assert_not_impl!(Close<'_, *const ()>: Send);
    assert_not_impl!(Close<'_, *const ()>: Sync);
    assert_not_impl!(Close<'_, PhantomPinned>: Unpin);

    assert_impl!(Copy<(), ()>: Send);
    assert_impl!(Copy<(), ()>: Sync);
    assert_impl!(Copy<(), PhantomPinned>: Unpin);
    assert_not_impl!(Copy<(), *const ()>: Send);
    assert_not_impl!(Copy<*const (), ()>: Send);
    assert_not_impl!(Copy<(), *const ()>: Sync);
    assert_not_impl!(Copy<*const (), ()>: Sync);
    assert_not_impl!(Copy<PhantomPinned, ()>: Unpin);

    assert_impl!(CopyBuf<(), ()>: Send);
    assert_impl!(CopyBuf<(), ()>: Sync);
    assert_impl!(CopyBuf<(), PhantomPinned>: Unpin);
    assert_not_impl!(CopyBuf<(), *const ()>: Send);
    assert_not_impl!(CopyBuf<*const (), ()>: Send);
    assert_not_impl!(CopyBuf<(), *const ()>: Sync);
    assert_not_impl!(CopyBuf<*const (), ()>: Sync);
    assert_not_impl!(CopyBuf<PhantomPinned, ()>: Unpin);

    assert_impl!(Cursor<()>: Send);
    assert_impl!(Cursor<()>: Sync);
    assert_impl!(Cursor<()>: Unpin);
    assert_not_impl!(Cursor<*const ()>: Send);
    assert_not_impl!(Cursor<*const ()>: Sync);
    assert_not_impl!(Cursor<PhantomPinned>: Unpin);

    assert_impl!(Empty: Send);
    assert_impl!(Empty: Sync);
    assert_impl!(Empty: Unpin);

    assert_impl!(FillBuf<'_, ()>: Send);
    assert_impl!(FillBuf<'_, ()>: Sync);
    assert_impl!(FillBuf<'_, PhantomPinned>: Unpin);
    assert_not_impl!(FillBuf<'_, *const ()>: Send);
    assert_not_impl!(FillBuf<'_, *const ()>: Sync);

    assert_impl!(Flush<'_, ()>: Send);
    assert_impl!(Flush<'_, ()>: Sync);
    assert_impl!(Flush<'_, ()>: Unpin);
    assert_not_impl!(Flush<'_, *const ()>: Send);
    assert_not_impl!(Flush<'_, *const ()>: Sync);
    assert_not_impl!(Flush<'_, PhantomPinned>: Unpin);

    assert_impl!(IntoSink<(), ()>: Send);
    assert_impl!(IntoSink<(), ()>: Sync);
    assert_impl!(IntoSink<(), PhantomPinned>: Unpin);
    assert_not_impl!(IntoSink<(), *const ()>: Send);
    assert_not_impl!(IntoSink<*const (), ()>: Send);
    assert_not_impl!(IntoSink<(), *const ()>: Sync);
    assert_not_impl!(IntoSink<*const (), ()>: Sync);
    assert_not_impl!(IntoSink<PhantomPinned, ()>: Unpin);

    assert_impl!(Lines<()>: Send);
    assert_impl!(Lines<()>: Sync);
    assert_impl!(Lines<()>: Unpin);
    assert_not_impl!(Lines<*const ()>: Send);
    assert_not_impl!(Lines<*const ()>: Sync);
    assert_not_impl!(Lines<PhantomPinned>: Unpin);

    assert_impl!(Read<'_, ()>: Send);
    assert_impl!(Read<'_, ()>: Sync);
    assert_impl!(Read<'_, ()>: Unpin);
    assert_not_impl!(Read<'_, *const ()>: Send);
    assert_not_impl!(Read<'_, *const ()>: Sync);
    assert_not_impl!(Read<'_, PhantomPinned>: Unpin);

    assert_impl!(ReadExact<'_, ()>: Send);
    assert_impl!(ReadExact<'_, ()>: Sync);
    assert_impl!(ReadExact<'_, ()>: Unpin);
    assert_not_impl!(ReadExact<'_, *const ()>: Send);
    assert_not_impl!(ReadExact<'_, *const ()>: Sync);
    assert_not_impl!(ReadExact<'_, PhantomPinned>: Unpin);

    assert_impl!(ReadHalf<()>: Send);
    assert_impl!(ReadHalf<()>: Sync);
    assert_impl!(ReadHalf<PhantomPinned>: Unpin);
    assert_not_impl!(ReadHalf<*const ()>: Send);
    assert_not_impl!(ReadHalf<*const ()>: Sync);

    assert_impl!(ReadLine<'_, ()>: Send);
    assert_impl!(ReadLine<'_, ()>: Sync);
    assert_impl!(ReadLine<'_, ()>: Unpin);
    assert_not_impl!(ReadLine<'_, *const ()>: Send);
    assert_not_impl!(ReadLine<'_, *const ()>: Sync);
    assert_not_impl!(ReadLine<'_, PhantomPinned>: Unpin);

    assert_impl!(ReadToEnd<'_, ()>: Send);
    assert_impl!(ReadToEnd<'_, ()>: Sync);
    assert_impl!(ReadToEnd<'_, ()>: Unpin);
    assert_not_impl!(ReadToEnd<'_, *const ()>: Send);
    assert_not_impl!(ReadToEnd<'_, *const ()>: Sync);
    assert_not_impl!(ReadToEnd<'_, PhantomPinned>: Unpin);

    assert_impl!(ReadToString<'_, ()>: Send);
    assert_impl!(ReadToString<'_, ()>: Sync);
    assert_impl!(ReadToString<'_, ()>: Unpin);
    assert_not_impl!(ReadToString<'_, *const ()>: Send);
    assert_not_impl!(ReadToString<'_, *const ()>: Sync);
    assert_not_impl!(ReadToString<'_, PhantomPinned>: Unpin);

    assert_impl!(ReadUntil<'_, ()>: Send);
    assert_impl!(ReadUntil<'_, ()>: Sync);
    assert_impl!(ReadUntil<'_, ()>: Unpin);
    assert_not_impl!(ReadUntil<'_, *const ()>: Send);
    assert_not_impl!(ReadUntil<'_, *const ()>: Sync);
    assert_not_impl!(ReadUntil<'_, PhantomPinned>: Unpin);

    assert_impl!(ReadVectored<'_, ()>: Send);
    assert_impl!(ReadVectored<'_, ()>: Sync);
    assert_impl!(ReadVectored<'_, ()>: Unpin);
    assert_not_impl!(ReadVectored<'_, *const ()>: Send);
    assert_not_impl!(ReadVectored<'_, *const ()>: Sync);
    assert_not_impl!(ReadVectored<'_, PhantomPinned>: Unpin);

    assert_impl!(Repeat: Send);
    assert_impl!(Repeat: Sync);
    assert_impl!(Repeat: Unpin);

    assert_impl!(ReuniteError<()>: Send);
    assert_impl!(ReuniteError<()>: Sync);
    assert_impl!(ReuniteError<PhantomPinned>: Unpin);
    assert_not_impl!(ReuniteError<*const ()>: Send);
    assert_not_impl!(ReuniteError<*const ()>: Sync);

    assert_impl!(Seek<'_, ()>: Send);
    assert_impl!(Seek<'_, ()>: Sync);
    assert_impl!(Seek<'_, ()>: Unpin);
    assert_not_impl!(Seek<'_, *const ()>: Send);
    assert_not_impl!(Seek<'_, *const ()>: Sync);
    assert_not_impl!(Seek<'_, PhantomPinned>: Unpin);

    assert_impl!(Sink: Send);
    assert_impl!(Sink: Sync);
    assert_impl!(Sink: Unpin);

    assert_impl!(Take<()>: Send);
    assert_impl!(Take<()>: Sync);
    assert_impl!(Take<()>: Unpin);
    assert_not_impl!(Take<*const ()>: Send);
    assert_not_impl!(Take<*const ()>: Sync);
    assert_not_impl!(Take<PhantomPinned>: Unpin);

    assert_impl!(Window<()>: Send);
    assert_impl!(Window<()>: Sync);
    assert_impl!(Window<()>: Unpin);
    assert_not_impl!(Window<*const ()>: Send);
    assert_not_impl!(Window<*const ()>: Sync);
    assert_not_impl!(Window<PhantomPinned>: Unpin);

    assert_impl!(Write<'_, ()>: Send);
    assert_impl!(Write<'_, ()>: Sync);
    assert_impl!(Write<'_, ()>: Unpin);
    assert_not_impl!(Write<'_, *const ()>: Send);
    assert_not_impl!(Write<'_, *const ()>: Sync);
    assert_not_impl!(Write<'_, PhantomPinned>: Unpin);

    assert_impl!(WriteAll<'_, ()>: Send);
    assert_impl!(WriteAll<'_, ()>: Sync);
    assert_impl!(WriteAll<'_, ()>: Unpin);
    assert_not_impl!(WriteAll<'_, *const ()>: Send);
    assert_not_impl!(WriteAll<'_, *const ()>: Sync);
    assert_not_impl!(WriteAll<'_, PhantomPinned>: Unpin);

    #[cfg(feature = "write-all-vectored")]
    assert_impl!(WriteAllVectored<'_, ()>: Send);
    #[cfg(feature = "write-all-vectored")]
    assert_impl!(WriteAllVectored<'_, ()>: Sync);
    #[cfg(feature = "write-all-vectored")]
    assert_impl!(WriteAllVectored<'_, ()>: Unpin);
    #[cfg(feature = "write-all-vectored")]
    assert_not_impl!(WriteAllVectored<'_, *const ()>: Send);
    #[cfg(feature = "write-all-vectored")]
    assert_not_impl!(WriteAllVectored<'_, *const ()>: Sync);
    // WriteAllVectored requires `W: Unpin`
    // #[cfg(feature = "write-all-vectored")]
    // assert_not_impl!(WriteAllVectored<'_, PhantomPinned>: Unpin);

    assert_impl!(WriteHalf<()>: Send);
    assert_impl!(WriteHalf<()>: Sync);
    assert_impl!(WriteHalf<PhantomPinned>: Unpin);
    assert_not_impl!(WriteHalf<*const ()>: Send);
    assert_not_impl!(WriteHalf<*const ()>: Sync);

    assert_impl!(WriteVectored<'_, ()>: Send);
    assert_impl!(WriteVectored<'_, ()>: Sync);
    assert_impl!(WriteVectored<'_, ()>: Unpin);
    assert_not_impl!(WriteVectored<'_, *const ()>: Send);
    assert_not_impl!(WriteVectored<'_, *const ()>: Sync);
    assert_not_impl!(WriteVectored<'_, PhantomPinned>: Unpin);
}

/// Assert Send/Sync/Unpin for all public types in `futures::lock`.
pub mod lock {
    use super::*;
    use futures::lock::*;

    #[cfg(feature = "bilock")]
    assert_impl!(BiLock<()>: Send);
    #[cfg(feature = "bilock")]
    assert_impl!(BiLock<()>: Sync);
    #[cfg(feature = "bilock")]
    assert_impl!(BiLock<PhantomPinned>: Unpin);
    #[cfg(feature = "bilock")]
    assert_not_impl!(BiLock<*const ()>: Send);
    #[cfg(feature = "bilock")]
    assert_not_impl!(BiLock<*const ()>: Sync);

    #[cfg(feature = "bilock")]
    assert_impl!(BiLockAcquire<'_, ()>: Send);
    #[cfg(feature = "bilock")]
    assert_impl!(BiLockAcquire<'_, ()>: Sync);
    #[cfg(feature = "bilock")]
    assert_impl!(BiLockAcquire<'_, PhantomPinned>: Unpin);
    #[cfg(feature = "bilock")]
    assert_not_impl!(BiLockAcquire<'_, *const ()>: Send);
    #[cfg(feature = "bilock")]
    assert_not_impl!(BiLockAcquire<'_, *const ()>: Sync);

    #[cfg(feature = "bilock")]
    assert_impl!(BiLockGuard<'_, ()>: Send);
    #[cfg(feature = "bilock")]
    assert_impl!(BiLockGuard<'_, ()>: Sync);
    #[cfg(feature = "bilock")]
    assert_impl!(BiLockGuard<'_, PhantomPinned>: Unpin);
    #[cfg(feature = "bilock")]
    assert_not_impl!(BiLockGuard<'_, *const ()>: Send);
    #[cfg(feature = "bilock")]
    assert_not_impl!(BiLockGuard<'_, *const ()>: Sync);

    assert_impl!(MappedMutexGuard<'_, (), ()>: Send);
    assert_impl!(MappedMutexGuard<'_, (), ()>: Sync);
    assert_impl!(MappedMutexGuard<'_, PhantomPinned, PhantomPinned>: Unpin);
    assert_not_impl!(MappedMutexGuard<'_, (), *const ()>: Send);
    assert_not_impl!(MappedMutexGuard<'_, *const (), ()>: Send);
    assert_not_impl!(MappedMutexGuard<'_, (), *const ()>: Sync);
    assert_not_impl!(MappedMutexGuard<'_, *const (), ()>: Sync);

    assert_impl!(Mutex<()>: Send);
    assert_impl!(Mutex<()>: Sync);
    assert_impl!(Mutex<()>: Unpin);
    assert_not_impl!(Mutex<*const ()>: Send);
    assert_not_impl!(Mutex<*const ()>: Sync);
    assert_not_impl!(Mutex<PhantomPinned>: Unpin);

    assert_impl!(MutexGuard<'_, ()>: Send);
    assert_impl!(MutexGuard<'_, ()>: Sync);
    assert_impl!(MutexGuard<'_, PhantomPinned>: Unpin);
    assert_not_impl!(MutexGuard<'_, *const ()>: Send);
    assert_not_impl!(MutexGuard<'_, *const ()>: Sync);

    assert_impl!(MutexLockFuture<'_, ()>: Send);
    assert_impl!(MutexLockFuture<'_, *const ()>: Sync);
    assert_impl!(MutexLockFuture<'_, PhantomPinned>: Unpin);
    assert_not_impl!(MutexLockFuture<'_, *const ()>: Send);

    #[cfg(feature = "bilock")]
    assert_impl!(ReuniteError<()>: Send);
    #[cfg(feature = "bilock")]
    assert_impl!(ReuniteError<()>: Sync);
    #[cfg(feature = "bilock")]
    assert_impl!(ReuniteError<PhantomPinned>: Unpin);
    #[cfg(feature = "bilock")]
    assert_not_impl!(ReuniteError<*const ()>: Send);
    #[cfg(feature = "bilock")]
    assert_not_impl!(ReuniteError<*const ()>: Sync);
}

/// Assert Send/Sync/Unpin for all public types in `futures::sink`.
pub mod sink {
    use super::*;
    use futures::sink::{self, *};
    use std::marker::Send;

    assert_impl!(Buffer<(), ()>: Send);
    assert_impl!(Buffer<(), ()>: Sync);
    assert_impl!(Buffer<(), PhantomPinned>: Unpin);
    assert_not_impl!(Buffer<(), *const ()>: Send);
    assert_not_impl!(Buffer<*const (), ()>: Send);
    assert_not_impl!(Buffer<(), *const ()>: Sync);
    assert_not_impl!(Buffer<*const (), ()>: Sync);
    assert_not_impl!(Buffer<PhantomPinned, ()>: Unpin);

    assert_impl!(Close<'_, (), *const ()>: Send);
    assert_impl!(Close<'_, (), *const ()>: Sync);
    assert_impl!(Close<'_, (), PhantomPinned>: Unpin);
    assert_not_impl!(Close<'_, *const (), ()>: Send);
    assert_not_impl!(Close<'_, *const (), ()>: Sync);
    assert_not_impl!(Close<'_, PhantomPinned, ()>: Unpin);

    assert_impl!(Drain<()>: Send);
    assert_impl!(Drain<()>: Sync);
    assert_impl!(Drain<PhantomPinned>: Unpin);
    assert_not_impl!(Drain<*const ()>: Send);
    assert_not_impl!(Drain<*const ()>: Sync);

    assert_impl!(Fanout<(), ()>: Send);
    assert_impl!(Fanout<(), ()>: Sync);
    assert_impl!(Fanout<(), ()>: Unpin);
    assert_not_impl!(Fanout<(), *const ()>: Send);
    assert_not_impl!(Fanout<*const (), ()>: Send);
    assert_not_impl!(Fanout<(), *const ()>: Sync);
    assert_not_impl!(Fanout<*const (), ()>: Sync);
    assert_not_impl!(Fanout<(), PhantomPinned>: Unpin);
    assert_not_impl!(Fanout<PhantomPinned, ()>: Unpin);

    assert_impl!(Feed<'_, (), ()>: Send);
    assert_impl!(Feed<'_, (), ()>: Sync);
    assert_impl!(Feed<'_, (), PhantomPinned>: Unpin);
    assert_not_impl!(Feed<'_, (), *const ()>: Send);
    assert_not_impl!(Feed<'_, *const (), ()>: Send);
    assert_not_impl!(Feed<'_, (), *const ()>: Sync);
    assert_not_impl!(Feed<'_, *const (), ()>: Sync);
    assert_not_impl!(Feed<'_, PhantomPinned, ()>: Unpin);

    assert_impl!(Flush<'_, (), *const ()>: Send);
    assert_impl!(Flush<'_, (), *const ()>: Sync);
    assert_impl!(Flush<'_, (), PhantomPinned>: Unpin);
    assert_not_impl!(Flush<'_, *const (), ()>: Send);
    assert_not_impl!(Flush<'_, *const (), ()>: Sync);
    assert_not_impl!(Flush<'_, PhantomPinned, ()>: Unpin);

    assert_impl!(sink::Send<'_, (), ()>: Send);
    assert_impl!(sink::Send<'_, (), ()>: Sync);
    assert_impl!(sink::Send<'_, (), PhantomPinned>: Unpin);
    assert_not_impl!(sink::Send<'_, (), *const ()>: Send);
    assert_not_impl!(sink::Send<'_, *const (), ()>: Send);
    assert_not_impl!(sink::Send<'_, (), *const ()>: Sync);
    assert_not_impl!(sink::Send<'_, *const (), ()>: Sync);
    assert_not_impl!(sink::Send<'_, PhantomPinned, ()>: Unpin);

    assert_impl!(SendAll<'_, (), SendTryStream<()>>: Send);
    assert_impl!(SendAll<'_, (), SyncTryStream<()>>: Sync);
    assert_impl!(SendAll<'_, (), UnpinTryStream>: Unpin);
    assert_not_impl!(SendAll<'_, (), SendTryStream>: Send);
    assert_not_impl!(SendAll<'_, (), LocalTryStream>: Send);
    assert_not_impl!(SendAll<'_, *const (), SendTryStream<()>>: Send);
    assert_not_impl!(SendAll<'_, (), SyncTryStream>: Sync);
    assert_not_impl!(SendAll<'_, (), LocalTryStream>: Sync);
    assert_not_impl!(SendAll<'_, *const (), SyncTryStream<()>>: Sync);
    assert_not_impl!(SendAll<'_, PhantomPinned, UnpinTryStream>: Unpin);
    assert_not_impl!(SendAll<'_, (), PinnedTryStream>: Unpin);

    assert_impl!(SinkErrInto<SendSink, *const (), *const ()>: Send);
    assert_impl!(SinkErrInto<SyncSink, *const (), *const ()>: Sync);
    assert_impl!(SinkErrInto<UnpinSink, PhantomPinned, PhantomPinned>: Unpin);
    assert_not_impl!(SinkErrInto<LocalSink<()>, (), ()>: Send);
    assert_not_impl!(SinkErrInto<LocalSink<()>, (), ()>: Sync);
    assert_not_impl!(SinkErrInto<PinnedSink<()>, (), ()>: Unpin);

    assert_impl!(SinkMapErr<SendSink, ()>: Send);
    assert_impl!(SinkMapErr<SyncSink, ()>: Sync);
    assert_impl!(SinkMapErr<UnpinSink, PhantomPinned>: Unpin);
    assert_not_impl!(SinkMapErr<SendSink, *const ()>: Send);
    assert_not_impl!(SinkMapErr<LocalSink<()>, ()>: Send);
    assert_not_impl!(SinkMapErr<SyncSink, *const ()>: Sync);
    assert_not_impl!(SinkMapErr<LocalSink<()>, ()>: Sync);
    assert_not_impl!(SinkMapErr<PinnedSink<()>, ()>: Unpin);

    assert_impl!(Unfold<(), (), ()>: Send);
    assert_impl!(Unfold<(), (), ()>: Sync);
    assert_impl!(Unfold<PhantomPinned, PhantomPinned, ()>: Unpin);
    assert_not_impl!(Unfold<*const (), (), ()>: Send);
    assert_not_impl!(Unfold<(), *const (), ()>: Send);
    assert_not_impl!(Unfold<(), (), *const ()>: Send);
    assert_not_impl!(Unfold<*const (), (), ()>: Sync);
    assert_not_impl!(Unfold<(), *const (), ()>: Sync);
    assert_not_impl!(Unfold<(), (), *const ()>: Sync);
    assert_not_impl!(Unfold<PinnedSink<()>, (), PhantomPinned>: Unpin);

    assert_impl!(With<(), *const (), *const (), (), ()>: Send);
    assert_impl!(With<(), *const (), *const (), (), ()>: Sync);
    assert_impl!(With<(), PhantomPinned, PhantomPinned, (), PhantomPinned>: Unpin);
    assert_not_impl!(With<*const (), (), (), (), ()>: Send);
    assert_not_impl!(With<(), (), (), *const (), ()>: Send);
    assert_not_impl!(With<(), (), (), (), *const ()>: Send);
    assert_not_impl!(With<*const (), (), (), (), ()>: Sync);
    assert_not_impl!(With<(), (), (), *const (), ()>: Sync);
    assert_not_impl!(With<(), (), (), (), *const ()>: Sync);
    assert_not_impl!(With<PhantomPinned, (), (), (), ()>: Unpin);
    assert_not_impl!(With<(), (), (), PhantomPinned, ()>: Unpin);

    assert_impl!(WithFlatMap<(), (), *const (), (), ()>: Send);
    assert_impl!(WithFlatMap<(), (), *const (), (), ()>: Sync);
    assert_impl!(WithFlatMap<(), PhantomPinned, PhantomPinned, (), PhantomPinned>: Unpin);
    assert_not_impl!(WithFlatMap<*const (), (), (), (), ()>: Send);
    assert_not_impl!(WithFlatMap<(), *const (), (), (), ()>: Send);
    assert_not_impl!(WithFlatMap<(), (), (), *const (), ()>: Send);
    assert_not_impl!(WithFlatMap<(), (), (), (), *const ()>: Send);
    assert_not_impl!(WithFlatMap<*const (), (), (), (), ()>: Sync);
    assert_not_impl!(WithFlatMap<(), *const (), (), (), ()>: Sync);
    assert_not_impl!(WithFlatMap<(), (), (), *const (), ()>: Sync);
    assert_not_impl!(WithFlatMap<(), (), (), (), *const ()>: Sync);
    assert_not_impl!(WithFlatMap<PhantomPinned, (), (), (), ()>: Unpin);
    assert_not_impl!(WithFlatMap<(), (), (), PhantomPinned, ()>: Unpin);
}

/// Assert Send/Sync/Unpin for all public types in `futures::stream`.
pub mod stream {
    // use super::*;
    // use futures::stream::*;
}

/// Assert Send/Sync/Unpin for all public types in `futures::task`.
pub mod task {
    use super::*;
    use futures::task::*;

    assert_impl!(AtomicWaker: Send);
    assert_impl!(AtomicWaker: Sync);
    assert_impl!(AtomicWaker: Unpin);

    assert_impl!(FutureObj<*const ()>: Send);
    assert_impl!(FutureObj<PhantomPinned>: Unpin);
    assert_not_impl!(FutureObj<()>: Sync);

    assert_impl!(LocalFutureObj<PhantomPinned>: Unpin);
    assert_not_impl!(LocalFutureObj<()>: Send);
    assert_not_impl!(LocalFutureObj<()>: Sync);

    assert_impl!(SpawnError: Send);
    assert_impl!(SpawnError: Sync);
    assert_impl!(SpawnError: Unpin);

    assert_impl!(WakerRef<'_>: Send);
    assert_impl!(WakerRef<'_>: Sync);
    assert_impl!(WakerRef<'_>: Unpin);
}
