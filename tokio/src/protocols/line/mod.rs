use std::io::{self, Write};
use std::net::SocketAddr;

use mio_server::{self, SerialDispatch, Encode, Decode};
use {Service, Serve, ServerConfig};

use futures::Poll;
use futuremio::{BufWriter, InputBuf};

pub struct Request {
    pub data: InputBuf,
}

impl Decode for Request {
    type Decoder = ();
    // FiXME: probably want a different error type
    type Error = io::Error;

    fn decode(_: &mut (), buf: &mut InputBuf) -> Poll<Request, io::Error> {
        if let Some(n) = buf.iter().position(|byte| *byte == b'\n') {
            Poll::Ok(Request { data: buf.take(n + 1) })
        } else {
            Poll::NotReady
        }
    }
}

pub struct Response {
    pub data: Vec<u8>,
}

impl<W: Write + Send + 'static> Encode<W> for Response {
    type Error = io::Error;
    type Fut = io::Result<BufWriter<W>>;

    fn encode(&self, mut writer: BufWriter<W>) -> io::Result<BufWriter<W>> {
        writer.extend(&self.data);
        Ok(writer)
    }
}

pub struct Server {
    addr: SocketAddr,
    mio: mio_server::Server,
    cfg: ServerConfig,
}

impl Server {
    pub fn new(addr: SocketAddr) -> Server {
        Server {
            addr: addr,
            mio: Default::default(),
            cfg: Default::default(),
        }
    }
}

impl Serve for Server {
    type Req = Request;
    type Resp = Response;
    type Error = io::Error; // TODO

    fn get_config_mut(&mut self) -> &mut ServerConfig {
        &mut self.cfg
    }

    fn get_mio_mut(&mut self) -> &mut mio_server::Server {
        &mut self.mio
    }

    fn serve_unconfigured<S>(&mut self, service: S) -> Result<(), Self::Error>
        where S: Service<Req = Request, Resp = Response, Error = io::Error>
    {
        self.mio.serve(self.addr, SerialDispatch::new(service))
    }
}
