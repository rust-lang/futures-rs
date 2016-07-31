use std::io;
use std::net::SocketAddr;
use std::thread;

use mio_server::Dispatch;

use net2;
use futures::{self, Future};
use futures::stream::Stream;
use futuremio::{Loop, TcpListener};

#[derive(Clone)]
pub struct Server {
    pub workers: u32,
}

impl Default for Server {
    fn default() -> Self {
        Server {
            workers: 1,
        }
    }
}

impl Server {
    #[cfg(unix)]
    fn configure_tcp(&self, tcp: &net2::TcpBuilder) -> io::Result<()> {
        use net2::unix::*;

        if self.workers > 1 {
            try!(tcp.reuse_port(true));
        }

        Ok(())
    }

    #[cfg(windows)]
    fn configure_tcp(&self, _tcp: &net2::TcpBuilder) -> io::Result<()> {
        Ok(())
    }

    // TODO: return error?
    fn serve_one<D: Dispatch>(&self, addr: SocketAddr, mut dispatcher: D) {
        let mut lp = Loop::new().unwrap();
        let handle = lp.handle();

        let listener =
            (|| {
                let listener = try!(net2::TcpBuilder::new_v4());
                try!(self.configure_tcp(&listener));
                try!(listener.reuse_address(true));
                try!(listener.bind(addr));
                listener.listen(1024)
            })();
        let server = match listener {
            Ok(l) => TcpListener::from_listener(l, &addr, handle),
            Err(e) => futures::failed(e).boxed(),
        }.and_then(move |l| {
            l.incoming().for_each(move |(stream, _)| {
                // Crucially use `.forget()` here instead of returning the future, allows
                // processing multiple separate connections concurrently.
                dispatcher.new_instance(stream).forget();

                Ok(()) // TODO: error handling
            })
        });

        // TODO: error handling
        lp.run(server).unwrap();
    }

    pub fn serve<D: Dispatch>(&self, addr: SocketAddr, dispatcher: D) -> io::Result<()> {
        let threads = (0..self.workers - 1)
            .map(|i| {
                let cfg = self.clone();
                let dispatcher = dispatcher.clone();
                thread::Builder::new()
                    .name(format!("worker{}", i))
                    .spawn(move || cfg.serve_one(addr, dispatcher))
                    .unwrap()
            })
            .collect::<Vec<_>>();

        self.serve_one(addr, dispatcher);

        for thread in threads {
            thread.join().unwrap();
        }

        Ok(())
    }
}
