use std::mem;
use std::io::{self, Write};
use std::sync::Arc;

use mio_server::{Dispatch, Encode, Decode, DecodeStream};
use Service;

use futures::{Future, Task, Poll, IntoFuture};
use futures::stream::{Stream, Fuse};
use futuremio::{TcpStream, TcpSource, BufWriter, Flush, Reserve};

pub struct PipelineDispatch<S> {
    service: Arc<S>,
}

impl<S> Clone for PipelineDispatch<S> {
    fn clone(&self) -> PipelineDispatch<S> {
        PipelineDispatch { service: self.service.clone() }
    }
}

impl<S> PipelineDispatch<S> {
    pub fn new(service: S) -> PipelineDispatch<S> {
        PipelineDispatch { service: Arc::new(service) }
    }
}

impl<S> Dispatch for PipelineDispatch<S>
    where S: Service,
          S::Req: Decode<Error = <S as Service>::Error>,
          <S as Service>::Resp: Encode<TcpSource, Error = <S as Service>::Error>,
          <S as Service>::Error: From<io::Error>
{
    type Item = ();
    type Error = S::Error;

    fn new_instance(&mut self, stream: TcpStream) -> Box<Future<Item = (), Error = S::Error>> {
        let service = self.service.clone();
        let mut inst = service.new_instance();

        let writer = BufWriter::new(stream.source.clone(), stream.ready_write);
        let input = DecodeStream::new(stream.source, stream.ready_read).map_err(From::from);
        // TODO: pipeline parsing as well
        let responses = input.and_then(move |req| service.process_req(&mut inst, req));

        PipelinedWriter::new(responses, writer).boxed()
    }
}

struct PipelinedWriter<S, W>
    where S: Stream,
          S::Item: Encode<W>,
          W: Send + 'static
{
    items: Fuse<S>,
    state: State<<<S::Item as Encode<W>>::Fut as IntoFuture>::Future, W, S::Item>,
}

impl<S, W> PipelinedWriter<S, W>
    where W: Write + Send + 'static,
          S: Stream,
          S::Item: Encode<W, Error = <S as Stream>::Error>,
          <S as Stream>::Error: From<io::Error>
{
    fn new(stream: S, writer: BufWriter<W>) -> PipelinedWriter<S, W> {
        PipelinedWriter {
            items: stream.fuse(),
            state: State::Read(writer.flush()),
        }
    }
}

enum State<Fut, W, Item> {
    Empty,
    Read(Flush<W>),
    Reserve(Reserve<W>, Item),
    Write(Fut),
    Flush(Flush<W>),
}

impl<S, W> Future for PipelinedWriter<S, W>
    where W: Write + Send + 'static,
          S: Stream,
          S::Item: Encode<W, Error = <S as Stream>::Error>,
          <S as Stream>::Error: From<io::Error>
{
    type Item = ();
    type Error = S::Error;

    fn poll(&mut self, task: &mut Task) -> Poll<(), S::Error> {
        // "steady state" is always Read or Write
        if let State::Flush(_) = self.state {
            panic!("Attempted to poll a PipelinedWriter in Flush state");
        }

        // First read and write (into a buffer) as many items as we can;
        // then, if possible, try to flush them out.
        let mut task = task.scoped();
        loop {
            match mem::replace(&mut self.state, State::Empty) {
                State::Read(flush) => {
                    match self.items.poll(&mut task) {
                        Poll::Err(e) => return Poll::Err(e),
                        Poll::Ok(Some(item)) => {
                            let writer = flush.into_inner();
                            if let Some(n) = item.size_hint() {
                                self.state = State::Reserve(writer.reserve(n), item);
                            } else {
                                self.state = State::Write(item.encode(writer).into_future());
                            }
                        }
                        Poll::Ok(None) | Poll::NotReady => {
                            self.state = State::Flush(flush);
                        }
                    }
                }
                State::Reserve(mut res, item) => {
                    match res.poll(&mut task) {
                        Poll::Err((e, _)) => return Poll::Err(e.into()),
                        Poll::Ok(writer) => {
                            self.state = State::Write(item.encode(writer).into_future())
                        }
                        Poll::NotReady => {
                            self.state = State::Reserve(res, item);
                            return Poll::NotReady;
                        }
                    }
                }
                State::Write(mut fut) => {
                    match fut.poll(&mut task) {
                        Poll::Err(e) => return Poll::Err(e.into()),
                        Poll::Ok(writer) => self.state = State::Read(writer.flush()),
                        Poll::NotReady => {
                            self.state = State::Write(fut);
                            return Poll::NotReady;
                        }
                    }
                }
                State::Flush(mut flush) => {
                    match flush.poll(&mut task) {
                        Poll::Err((e, _)) => return Poll::Err(e.into()),
                        Poll::Ok(writer) => {
                            if self.items.is_done() {
                                // Nothing more to write to sink, and no more incoming items;
                                // we're done!
                                return Poll::Ok(());
                            } else {
                                self.state = State::Read(writer.flush());
                                return Poll::NotReady;
                            }
                        }
                        Poll::NotReady => {
                            self.state = State::Read(flush);
                            return Poll::NotReady;
                        }
                    }
                }
                State::Empty => unreachable!(),
            }

            task.ready();
        }
    }

    fn schedule(&mut self, task: &mut Task) {
        match self.state {
            State::Read(ref mut flush) => {
                self.items.schedule(task);
                if flush.is_dirty() {
                    flush.schedule(task);
                }
            }
            State::Reserve(ref mut res, _) => res.schedule(task),
            State::Write(ref mut fut) => fut.schedule(task),
            State::Flush(_) => unreachable!(),
            State::Empty => unreachable!(),
        }
    }
}
