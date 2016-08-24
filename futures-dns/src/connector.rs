use futures::{failed, Future};
use futures_io::IoFuture;
use futures_mio::{LoopHandle, TcpStream};

use std::io;
use std::net::SocketAddr;

use super::{CpuPoolResolver, Endpoint, Resolver, ToEndpoint};

/// A `Connector` is a helper for creating connections.
///
/// This object is a wrapper around a `LoopHandle` and a resolver.
/// It helps initiate connections using endpoints, and offers
/// bultin address translation.
pub struct Connector {
    handle: LoopHandle,
    resolver: CpuPoolResolver,
}

impl Connector {
    /// Create a new `Connector`.
    ///
    /// The handle can be obtained using the `handle()` method of the `Loop` object.
    /// The resolver_threads parameter is the amount of threads given to the resolver's
    /// thread pool.
    pub fn new(handle: LoopHandle, resolver_threads: usize) -> Self {
        Connector {
            handle: handle,
            resolver: CpuPoolResolver::new(resolver_threads),
        }
    }

    /// Create a TcpStream connected to the given endpoint.
    ///
    /// If an address resolves to many IP addresses, they will be tried
    /// sequentially and in order.
    pub fn tcp_connect<'a, T>(&self, ep: T) -> IoFuture<TcpStream> where T: ToEndpoint<'a> {
        let ep = match ep.to_endpoint() {
            Ok(ep) => ep,
            Err(e) => return failed(e).boxed(),
        };

        match ep {
            Endpoint::Host(host, port) => {
                debug!("resolving {}", host);
                let handle = self.handle.clone();

                self.resolver.resolve(host).and_then(move |ip_addrs| {
                    debug!("chaining {} connection attempts", ip_addrs.len());

                    let mut prev: Option<IoFuture<TcpStream>> = None;

                    // This loop chains futures one after another so they each try
                    // to connect to an address in a sequential way.
                    for ip_addr in ip_addrs {
                        let addr = SocketAddr::new(ip_addr, port);
                        let handle = handle.clone();

                        prev = Some(match prev.take() {
                            None => handle.tcp_connect(&addr).boxed(),
                            Some(prev) => prev.or_else(move |_| handle.tcp_connect(&addr)).boxed(),
                        });
                    }

                    // If this Option is None, it means that there were no addresses in the list.
                    match prev.take() {
                        Some(fut) => fut,
                        None => failed(io::Error::new(io::ErrorKind::Other, "resolve returned no addresses")).boxed(),
                    }
                }).boxed()
            }
            Endpoint::SocketAddr(ref addr) => {
                self.handle.clone().tcp_connect(addr)
            }
        }
    }
}
