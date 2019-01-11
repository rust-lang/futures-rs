use core::marker::Unpin;
use core::pin::Pin;
use futures_core::future::TryFuture;
use futures_core::task::{LocalWaker, Poll};
use futures_sink::Sink;

#[derive(Debug)]
enum State<Fut, Si> {
    Waiting(Fut),
    Ready(Si),
    Closed,
}
use self::State::*;

/// Future for the [`flatten_sink`](super::TryFutureExt::flatten_sink)
/// combinator.
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

    #[allow(clippy::needless_lifetimes)] // https://github.com/rust-lang/rust/issues/52675
    fn project_pin<'a>(
        self: Pin<&'a mut Self>
    ) -> State<Pin<&'a mut Fut>, Pin<&'a mut Si>> {
        unsafe {
            match &mut Pin::get_unchecked_mut(self).0 {
                Waiting(f) => Waiting(Pin::new_unchecked(f)),
                Ready(s) => Ready(Pin::new_unchecked(s)),
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
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Result<(), Self::SinkError>> {
        let resolved_stream = match self.as_mut().project_pin() {
            Ready(s) => return s.poll_ready(lw),
            Waiting(f) => try_ready!(f.try_poll(lw)),
            Closed => panic!("poll_ready called after eof"),
        };
        Pin::set(&mut self.as_mut(), FlattenSink(Ready(resolved_stream)));
        if let Ready(resolved_stream) = self.project_pin() {
            resolved_stream.poll_ready(lw)
        } else {
            unreachable!()
        }
    }

    fn start_send(
        self: Pin<&mut Self>,
        item: Self::SinkItem,
    ) -> Result<(), Self::SinkError> {
        match self.project_pin() {
            Ready(s) => s.start_send(item),
            Waiting(_) => panic!("poll_ready not called first"),
            Closed => panic!("start_send called after eof"),
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Result<(), Self::SinkError>> {
        match self.project_pin() {
            Ready(s) => s.poll_flush(lw),
            // if sink not yet resolved, nothing written ==> everything flushed
            Waiting(_) => Poll::Ready(Ok(())),
            Closed => panic!("poll_flush called after eof"),
        }
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Result<(), Self::SinkError>> {
        let res = match self.as_mut().project_pin() {
            Ready(s) => s.poll_close(lw),
            Waiting(_) | Closed => Poll::Ready(Ok(())),
        };
        if res.is_ready() {
            Pin::set(&mut self, FlattenSink(Closed));
        }
        res
    }
}
