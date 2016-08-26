use futures::{failed, Future};
use futures_io::IoFuture;
use futures_mio::{LoopHandle, TcpStream};

use std::io;
use std::net::{IpAddr, SocketAddr};

use super::{select_all_ok};
use super::{CpuPoolResolver, Endpoint, Resolver, ToEndpoint};

/// The ConnectionMode tells the Connector how to attempt connecting.
pub enum ConnectionMode {
    /// Try to connect to all the addresses simultaneously.
    Parallel,
    /// Try to connect to all the addresses one after another.
    Sequential,
}

/// A `Connector` is a helper for creating connections.
///
/// This object is a wrapper around a `LoopHandle` and a resolver.
/// It helps initiate connections using endpoints, and offers
/// bultin address translation.
pub struct Connector {
    handle: LoopHandle,
    resolver: CpuPoolResolver,
    pub mode: ConnectionMode,
}

impl Connector {

    /// Create a new `Connector` with sensible default values.
    ///
    /// The `handle` can be obtained using the `handle()` method of the `Loop` object.
    /// The resolver will have 3 threads and the connection mode will be sequential.
    pub fn simple(handle: LoopHandle) -> Self {
        Connector::new(handle, 3, ConnectionMode::Sequential)
    }

    /// Create a new `Connector`.
    ///
    /// The `handle` can be obtained using the `handle()` method of the `Loop` object.
    /// The `resolver_threads` parameter is the amount of threads given to the resolver's
    /// thread pool. The `mode` parameter tells the connector how to attempt connection
    /// when the resolver yields more than one IP address.
    pub fn new(handle: LoopHandle, resolver_threads: usize, mode: ConnectionMode) -> Self {
        Connector {
            handle: handle,
            resolver: CpuPoolResolver::new(resolver_threads),
            mode: mode,
        }
    }

    /// Create a new TcpStream connected to the specified Endpoint.
    ///
    /// This function will connect to the specified endpoint using the connector's mode setting.
    pub fn tcp_connect<'a, T>(&self, ep: T) -> IoFuture<TcpStream> where T: ToEndpoint<'a> {
        match self.mode {
            ConnectionMode::Parallel => self.tcp_connect_par(ep),
            ConnectionMode::Sequential => self.tcp_connect_seq(ep),
        }
    }

    /// Create a new TcpStream connected to the specified endpoint.
    ///
    /// This function will connect to the speficied endpoint in parallel, regardless of the
    /// connector's mode setting.
    pub fn tcp_connect_par<'a, T>(&self, ep: T) -> IoFuture<TcpStream>
        where T: ToEndpoint<'a>
    {
        self.if_host_resolve(ep, |handle, port, ip_addrs| {
            debug!("creating {} parallel connection attemps", ip_addrs.len());

            let futs = ip_addrs.into_iter().map(|ip_addr| {
                let addr = SocketAddr::new(ip_addr, port);
                handle.clone().tcp_connect(&addr)
            });

            select_all_ok(futs).map_err(|_| {
                io::Error::new(io::ErrorKind::Other, "all of the connections attempts failed")
            }).boxed()
        })
    }

    /// Create a new TcpStream connected to the specified endpoint.
    ///
    /// This function will connect to the speficied endpoint sequentially, regardless of the
    /// connector's mode setting.
    pub fn tcp_connect_seq<'a, T>(&self, ep: T) -> IoFuture<TcpStream>
        where T: ToEndpoint<'a>
    {
        self.if_host_resolve(ep, |handle, port, ip_addrs| {
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
        })
    }

    // abstraction of the code that is common to tcp_connect_(par|seq).
    fn if_host_resolve<'a, T, F>(&self, ep: T, func: F) -> IoFuture<TcpStream>
            where T: ToEndpoint<'a>,
            F: FnOnce(LoopHandle, u16, Vec<IpAddr>) -> IoFuture<TcpStream> + Send + 'static
    {
        let ep = match ep.to_endpoint() {
            Ok(ep) => ep,
            Err(e) => return failed(e).boxed(),
        };

        match ep {
            Endpoint::Host(host, port) => {
                let handle = self.handle.clone();
                self.resolver.resolve(host).and_then(move |addrs| {
                    func(handle, port, addrs)
                }).boxed()
            }
            Endpoint::SocketAddr(ref addr) => {
                self.handle.clone().tcp_connect(addr)
            }
        }
    }
}
