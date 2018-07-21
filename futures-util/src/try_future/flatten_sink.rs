use core::marker::Unpin;
use core::mem::PinMut;
use futures_core::future::TryFuture;
use futures_core::task::{self, Poll};
use futures_sink::Sink;

#[derive(Debug)]
enum State<Fut, Si> {
    Waiting(Fut),
    Ready(Si),
    Closed,
}
use self::State::*;

/// Future for the `flatten_sink` combinator, flattening a
/// future-of-a-sink to get just the result of the final sink as a sink.
///
/// This is created by the `Future::flatten_sink` method.
#[derive(Debug)]
pub struct FlattenSink<Fut, Si>(State<Fut, Si>);

impl<Fut: Unpin, Si: Unpin> Unpin for FlattenSink<Fut, Si> {}

impl<Fut, Si> FlattenSink<Fut, Si>
where
    Fut: TryFuture<Ok = Si>,
    Si: Sink<SinkError = Fut::Error>,
{
    pub(super) fn new(future: Fut) -> FlattenSink<Fut, Si> {
        FlattenSink(Waiting(future))
    }

    #[allow(needless_lifetimes)] // https://github.com/rust-lang/rust/issues/52675
    fn project_pin(
        self: PinMut<'a, Self>
    ) -> State<PinMut<'a, Fut>, PinMut<'a, Si>> {
        unsafe {
            match &mut PinMut::get_mut_unchecked(self).0 {
                Waiting(f) => Waiting(PinMut::new_unchecked(f)),
                Ready(s) => Ready(PinMut::new_unchecked(s)),
                Closed => Closed,
            }
        }
    }
}

impl<Fut, Si> Sink for FlattenSink<Fut, Si>
where
    Fut: TryFuture<Ok = Si>,
    Si: Sink<SinkError = Fut::Error>,
{
    type SinkItem = Si::SinkItem;
    type SinkError = Si::SinkError;

    fn poll_ready(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Result<(), Self::SinkError>> {
        let resolved_stream = match self.reborrow().project_pin() {
            Ready(s) => return s.poll_ready(cx),
            Waiting(f) => try_ready!(f.try_poll(cx)),
            Closed => panic!("poll_ready called after eof"),
        };
        PinMut::set(self.reborrow(), FlattenSink(Ready(resolved_stream)));
        if let Ready(resolved_stream) = self.project_pin() {
            resolved_stream.poll_ready(cx)
        } else {
            unreachable!()
        }
    }

    fn start_send(
        self: PinMut<Self>,
        item: Self::SinkItem,
    ) -> Result<(), Self::SinkError> {
        match self.project_pin() {
            Ready(s) => s.start_send(item),
            Waiting(_) => panic!("poll_ready not called first"),
            Closed => panic!("start_send called after eof"),
        }
    }

    fn poll_flush(
        self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Result<(), Self::SinkError>> {
        match self.project_pin() {
            Ready(s) => s.poll_flush(cx),
            // if sink not yet resolved, nothing written ==> everything flushed
            Waiting(_) => Poll::Ready(Ok(())),
            Closed => panic!("poll_flush called after eof"),
        }
    }

    fn poll_close(
        mut self: PinMut<Self>,
        cx: &mut task::Context,
    ) -> Poll<Result<(), Self::SinkError>> {
        let res = match self.reborrow().project_pin() {
            Ready(s) => s.poll_close(cx),
            Waiting(_) | Closed => Poll::Ready(Ok(())),
        };
        if res.is_ready() {
            PinMut::set(self, FlattenSink(Closed));
        }
        res
    }
}
