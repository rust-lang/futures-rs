use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{FusedStream, Stream};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use futures_core::task::{Context, Poll};
use pin_project::{pin_project, project};

#[pin_project]
#[derive(Debug)]
pub enum Flatten<Fut1, Fut2> {
    First(#[pin] Fut1),
    Second(#[pin] Fut2),
    Empty,
}

impl<Fut1, Fut2> Flatten<Fut1, Fut2> {
    pub(crate) fn new(future: Fut1) -> Self {
        Flatten::First(future)
    }
}

impl<Fut> FusedFuture for Flatten<Fut, Fut::Output>
    where Fut: Future,
          Fut::Output: Future,
{
    fn is_terminated(&self) -> bool {
        match self {
            Flatten::Empty => true,
            _ => false,
        }
    }
}

impl<Fut> Future for Flatten<Fut, Fut::Output>
    where Fut: Future,
          Fut::Output: Future,
{
    type Output = <Fut::Output as Future>::Output;

    #[project]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(loop {
            #[project]
            match self.as_mut().project() {
                Flatten::First(f) => {
                    let f = ready!(f.poll(cx));
                    self.set(Flatten::Second(f));
                },
                Flatten::Second(f) => {
                    let output = ready!(f.poll(cx));
                    self.set(Flatten::Empty);
                    break output;
                },
                Flatten::Empty => panic!("Flatten polled after completion"),
            }
        })
    }
}

impl<Fut> FusedStream for Flatten<Fut, Fut::Output>
    where Fut: Future,
          Fut::Output: Stream,
{
    fn is_terminated(&self) -> bool {
        match self {
            Flatten::Empty => true,
            _ => false,
        }
    }
}

impl<Fut> Stream for Flatten<Fut, Fut::Output>
    where Fut: Future,
          Fut::Output: Stream,
{
    type Item = <Fut::Output as Stream>::Item;

    #[project]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(loop {
            #[project]
            match self.as_mut().project() {
                Flatten::First(f) => {
                    let f = ready!(f.poll(cx));
                    self.set(Flatten::Second(f));
                },
                Flatten::Second(f) => {
                    let output = ready!(f.poll_next(cx));
                    if output.is_none() {
                        self.set(Flatten::Empty);
                    }
                    break output;
                },
                Flatten::Empty => break None,
            }
        })
    }
}


#[cfg(feature = "sink")]
impl<Fut, Item> Sink<Item> for Flatten<Fut, Fut::Output>
where
    Fut: Future,
    Fut::Output: Sink<Item>,
{
    type Error = <Fut::Output as Sink<Item>>::Error;

    #[project]
    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(loop {
            #[project]
            match self.as_mut().project() {
                Flatten::First(f) => {
                    let f = ready!(f.poll(cx));
                    self.set(Flatten::Second(f));
                },
                Flatten::Second(f) => {
                    break ready!(f.poll_ready(cx));
                },
                Flatten::Empty => panic!("poll_ready called after eof"),
            }
        })
    }

    #[project]
    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        #[project]
        match self.project() {
            Flatten::First(_) => panic!("poll_ready not called first"),
            Flatten::Second(f) => f.start_send(item),
            Flatten::Empty => panic!("start_send called after eof"),
        }
    }

    #[project]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        #[project]
        match self.project() {
            Flatten::First(_) => Poll::Ready(Ok(())),
            Flatten::Second(f) => f.poll_flush(cx),
            Flatten::Empty => panic!("poll_flush called after eof"),
        }
    }

    #[project]
    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        #[project]
        let res = match self.as_mut().project() {
            Flatten::Second(f) => f.poll_close(cx),
            _ => Poll::Ready(Ok(())),
        };
        if res.is_ready() {
            self.set(Flatten::Empty);
        }
        res
    }
}
