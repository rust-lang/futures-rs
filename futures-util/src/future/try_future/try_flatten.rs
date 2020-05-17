use core::pin::Pin;
use futures_core::future::{FusedFuture, Future, TryFuture};
use futures_core::stream::{FusedStream, Stream, TryStream};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, project};

#[pin_project]
#[derive(Debug)]
pub enum TryFlatten<Fut1, Fut2> {
    First(#[pin] Fut1),
    Second(#[pin] Fut2),
    Empty,
}

impl<Fut1, Fut2> TryFlatten<Fut1, Fut2> {
    pub(crate) fn new(future: Fut1) -> Self {
        TryFlatten::First(future)
    }
}

impl<Fut> FusedFuture for TryFlatten<Fut, Fut::Ok>
    where Fut: TryFuture,
          Fut::Ok: TryFuture<Error=Fut::Error>,
{
    fn is_terminated(&self) -> bool {
        match self {
            TryFlatten::Empty => true,
            _ => false,
        }
    }
}

impl<Fut> Future for TryFlatten<Fut, Fut::Ok>
    where Fut: TryFuture,
          Fut::Ok: TryFuture<Error=Fut::Error>,
{
    type Output = Result<<Fut::Ok as TryFuture>::Ok, Fut::Error>;

    #[project]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(loop {
            #[project]
            match self.as_mut().project() {
                TryFlatten::First(f) => {
                    match ready!(f.try_poll(cx)) {
                        Ok(f) => self.set(TryFlatten::Second(f)),
                        Err(e) => {
                            self.set(TryFlatten::Empty);
                            break Err(e);
                        }
                    }
                },
                TryFlatten::Second(f) => {
                    let output = ready!(f.try_poll(cx));
                    self.set(TryFlatten::Empty);
                    break output;
                },
                TryFlatten::Empty => panic!("TryFlatten polled after completion"),
            }
        })
    }
}

impl<Fut> FusedStream for TryFlatten<Fut, Fut::Ok>
    where Fut: TryFuture,
          Fut::Ok: TryStream<Error=Fut::Error>,
{
    fn is_terminated(&self) -> bool {
        match self {
            TryFlatten::Empty => true,
            _ => false,
        }
    }
}

impl<Fut> Stream for TryFlatten<Fut, Fut::Ok>
    where Fut: TryFuture,
          Fut::Ok: TryStream<Error=Fut::Error>,
{
    type Item = Result<<Fut::Ok as TryStream>::Ok, Fut::Error>;

    #[project]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(loop {
            #[project]
            match self.as_mut().project() {
                TryFlatten::First(f) => {
                    match ready!(f.try_poll(cx)) {
                        Ok(f) => self.set(TryFlatten::Second(f)),
                        Err(e) => {
                            self.set(TryFlatten::Empty);
                            break Some(Err(e));
                        }
                    }
                },
                TryFlatten::Second(f) => {
                    let output = ready!(f.try_poll_next(cx));
                    if output.is_none() {
                        self.set(TryFlatten::Empty);
                    }
                    break output;
                },
                TryFlatten::Empty => break None,
            }
        })
    }
}


#[cfg(feature = "sink")]
impl<Fut, Item> Sink<Item> for TryFlatten<Fut, Fut::Ok>
where
    Fut: TryFuture,
    Fut::Ok: Sink<Item, Error=Fut::Error>,
{
    type Error = Fut::Error;

    #[project]
    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(loop {
            #[project]
            match self.as_mut().project() {
                TryFlatten::First(f) => {
                    match ready!(f.try_poll(cx)) {
                        Ok(f) => self.set(TryFlatten::Second(f)),
                        Err(e) => {
                            self.set(TryFlatten::Empty);
                            break Err(e);
                        }
                    }
                },
                TryFlatten::Second(f) => {
                    break ready!(f.poll_ready(cx));
                },
                TryFlatten::Empty => panic!("poll_ready called after eof"),
            }
        })
    }

    #[project]
    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        #[project]
        match self.project() {
            TryFlatten::First(_) => panic!("poll_ready not called first"),
            TryFlatten::Second(f) => f.start_send(item),
            TryFlatten::Empty => panic!("start_send called after eof"),
        }
    }

    #[project]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        #[project]
        match self.project() {
            TryFlatten::First(_) => Poll::Ready(Ok(())),
            TryFlatten::Second(f) => f.poll_flush(cx),
            TryFlatten::Empty => panic!("poll_flush called after eof"),
        }
    }

    #[project]
    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        #[project]
        let res = match self.as_mut().project() {
            TryFlatten::Second(f) => f.poll_close(cx),
            _ => Poll::Ready(Ok(())),
        };
        if res.is_ready() {
            self.set(TryFlatten::Empty);
        }
        res
    }
}
