use std::io;
use std::net::SocketAddr;

use mio_server::{self, PipelineDispatch};
use {Service, Serve, ServerConfig};

mod request;
pub use self::request::{Request, RequestHeaders};

mod response;
pub use self::response::Response;

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
        self.mio.serve(self.addr, PipelineDispatch::new(service))
    }
}
