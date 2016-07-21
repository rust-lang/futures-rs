use std::io;
use std::sync::Arc;

use mio_server::{Dispatch, Encode, Decode, DecodeStream};
use Service;

use futures::{IntoFuture, Future};
use futures::stream::Stream;
use futuremio::{TcpStream, TcpSource, BufWriter};

pub struct SerialDispatch<S> {
    service: Arc<S>,
}

impl<S> Clone for SerialDispatch<S> {
    fn clone(&self) -> SerialDispatch<S> {
        SerialDispatch { service: self.service.clone() }
    }
}

impl<S> SerialDispatch<S> {
    pub fn new(service: S) -> SerialDispatch<S> {
        SerialDispatch { service: Arc::new(service) }
    }
}

impl<S> Dispatch for SerialDispatch<S>
    where S: Service,
          S::Req: Decode<Error = <S as Service>::Error>,
          S::Resp: Encode<TcpSource, Error = <S as Service>::Error>,
          <S as Service>::Error: From<io::Error>
{
    type Item = ();
    type Error = S::Error;

    fn new_instance(&mut self, stream: TcpStream) -> Box<Future<Item = (), Error = S::Error>> {
        let writer = BufWriter::new(stream.source.clone(), stream.ready_write);
        let service = self.service.clone();
        let mut inst = service.new_instance();

        DecodeStream::new(stream.source, stream.ready_read)
            .and_then(move |req| service.process_req(&mut inst, req))
            .fold(writer, |writer, resp| {
                resp.encode(writer).into_future()
                    .and_then(|writer| writer.flush().map_err(|(e, _)| e.into()))
            })
            .map(|_| ())
            .boxed()
    }
}

// type Wrapped = SourceWrapper<Arc<mio::tcp::TcpStream>>;
// type Writer = BufWriter<Wrapped>;
//
//
// enum State<WriteFut> {
// Empty,
// Read(BufWriter<Wrapped>),
// Write(WriteFut),
// Flush(Flush<Wrapped>),
// }
//
// impl<S> Future for SerialDispatchInstance<S>
// where S: Service,
// S::Req: Decode,
// S::Resp: Encode,
// S::Error: From<<S::Req as Decode>::Error> + From<io::Error>
// {
// type Item = ();
// type Error = S::Error;
//
// fn poll(&mut self, mut tokens: &Tokens) -> Option<Result<(), S::Error>> {
// loop {
// match mem::replace(&mut self.state, State::Empty) {
// State::Read(writer) => {
// match self.decode_stream.poll(tokens) {
// Some(Err(e)) => return Some(Err(e.into())),
// Some(Ok(Some(item))) => {
// debug!("got an item to encode!");
// self.state = State::Write(item.encode(writer));
// }
// Some(Ok(None)) => {
// all existing data should've been flushed already
// assert!(!writer.is_dirty());
// return Some(Ok(()));
// }
// None => {
// self.state = State::Read(writer);
// return None;
// }
// }
// }
//
// State::Write(mut fut) => {
// match fut.poll(tokens) {
// Some(Err(e)) => return Some(Err(e.into())),
// Some(Ok(writer)) => {
// self.state = State::Flush(writer.flush());
// }
// None => {
// self.state = State::Write(fut);
// return None;
// }
// }
// }
//
// State::Flush(flush) => {
// match flush.poll(tokens) {
// Some(Err(e)) => return Some(Err(e.into())),
// Some(Ok(writer)) => {
// self.state = State::Read(writer);
// }
// None => {
// self.state = State::Flush(fut);
// return None;
// }
// }
// }
//
// State::Empty => unreachable!(),
// }
//
// tokens = &TOKENS_ALL;
// }
// }
// }
//

// impl Dispatch for SerialDispatch {
// fn dispatch<S>(&mut self, stream: TcpStream, service: Arc<S>, mut inst: S::Instance)
// where S: Service,
// S::Req: Decode,
// S::Resp: Encode,
// S::Error: From<<S::Req as Decode>::Error> + From<io::Error>
// {
// hack around lack of Read/Write impl on Arc<...>
// let read = SourceWrapper(stream.source.clone());
// let write = SourceWrapper(stream.source);
//
// let writer = Writer::new(write, stream.ready_write);
// DecodeStream::new(read, stream.ready_read)
// .map_err(From::from)
// .and_then(move |req| service.process_req(&mut inst, req));
// .fold(writer, |x| x.0);
//
// move |(writer, resp)| writer.write_item(resp).map_err(|(e, _)| e.into()));
// .forget();
// }
// }
//
//
