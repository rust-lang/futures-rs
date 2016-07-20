use futures::{IntoFuture, Future};
use futuremio::TcpStream;
use futuremio::BufWriter;

mod decode;
pub use self::decode::{Decode, DecodeStream};

mod server;
pub use self::server::Server;

mod serial_dispatch;
pub use self::serial_dispatch::SerialDispatch;

mod pipeline_dispatch;
pub use self::pipeline_dispatch::PipelineDispatch;

pub trait Dispatch: Send + 'static + Clone {
    type Item: Send + 'static;
    type Error: Send + 'static;

    fn new_instance(&mut self,
                    stream: TcpStream)
                    -> Box<Future<Item = Self::Item, Error = Self::Error>>;
}

pub trait Encode<W: Send + 'static>: Send + 'static {
    type Error: Send + 'static;
    type Fut: IntoFuture<Item=BufWriter<W>, Error=Self::Error> + Send + 'static;

    fn encode(&self, writer: BufWriter<W>) -> Self::Fut;

    fn size_hint(&self) -> Option<usize> {
        None
    }
}
