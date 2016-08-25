extern crate futures;
extern crate futures_cpupool;
extern crate futures_io;
extern crate futures_mio;

#[macro_use]
extern crate log;

mod connector;
mod select_all_ok;

use std::io;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::str::FromStr;

use futures::{BoxFuture, Future};
use futures_cpupool::CpuPool;
use futures_io::IoFuture;

pub use connector::Connector;
pub use select_all_ok::select_all_ok;

/// The Resolver trait represents an object capable of
/// resolving host names into IP addresses.
pub trait Resolver {
    /// Given a host name, this function returns a Future which
    /// will eventually resolve into a list of IP addresses.
    fn resolve(&self, host: &str) -> BoxFuture<Vec<IpAddr>, io::Error>;
}

/// A resolver based on a thread pool.
///
/// This resolver uses the `ToSocketAddrs` trait inside
/// a thread to provide non-blocking address resolving.
#[derive(Clone)]
pub struct CpuPoolResolver {
    pool: CpuPool,
}

impl CpuPoolResolver {
    /// Create a new CpuPoolResolver with the given number of threads.
    pub fn new(num_threads: usize) -> Self {
        CpuPoolResolver {
            pool: CpuPool::new(num_threads),
        }
    }
}

impl Resolver for CpuPoolResolver {
    fn resolve(&self, host: &str) -> IoFuture<Vec<IpAddr>> {
        let host = format!("{}:0", host);
        self.pool.execute(move || {
            match host[..].to_socket_addrs() {
                Ok(it) => Ok(it.map(|s| s.ip()).collect()),
                Err(e) => Err(e),
            }
        }).then(|res| {
            // CpuFuture cannot fail unless it panics
            res.unwrap()
        }).boxed()
    }
}

/// An Endpoint is a way of identifying the target of a connection.
///
/// It can be a socket address or a host name which needs to be resolved
/// into a list of IP addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Endpoint<'a> {
    Host(&'a str, u16),
    SocketAddr(SocketAddr),
}

/// A trait for objects that can be converted into an Endpoint.
///
/// This trait is implemented for the following types:
///
/// * `SocketAddr`, `&SocketAddr` - a socket address.
/// * `(IpAddr, u16)`, `(&str, u16)` - a target and a port.
/// * `&str` - a string formatted as `<target>:<port>` where
/// `<target>` is a host name or an IP address.
///
/// This trait is similar to the `ToSocketAddrs` trait, except
/// that it does not perform host name resolution.
pub trait ToEndpoint<'a> {
    fn to_endpoint(self) -> io::Result<Endpoint<'a>>;
}

impl<'a> ToEndpoint<'a> for SocketAddr {
    fn to_endpoint(self) -> io::Result<Endpoint<'a>> {
        Ok(Endpoint::SocketAddr(self))
    }
}

impl<'a, 'b> ToEndpoint<'a> for &'b SocketAddr {
    fn to_endpoint(self) -> io::Result<Endpoint<'a>> {
        Ok(Endpoint::SocketAddr(*self))
    }
}

impl <'a> ToEndpoint<'a> for (IpAddr, u16) {
    fn to_endpoint(self) -> io::Result<Endpoint<'a>> {
        Ok(Endpoint::SocketAddr(SocketAddr::new(self.0, self.1)))
    }
}

impl<'a> ToEndpoint<'a> for (&'a str, u16) {
    fn to_endpoint(self) -> io::Result<Endpoint<'a>> {
        match IpAddr::from_str(self.0) {
            Ok(addr) => (addr, self.1).to_endpoint(),
            Err(_) => Ok(Endpoint::Host(self.0, self.1)),
        }
    }
}

impl<'a> ToEndpoint<'a> for &'a str {
    fn to_endpoint(self) -> io::Result<Endpoint<'a>> {
        // try to parse as a socket address first
        if let Ok(addr) = self.parse() {
            return Ok(Endpoint::SocketAddr(addr));
        }

        fn parse_port(port: &str) -> io::Result<u16> {
            u16::from_str(port).map_err(|_| io::Error::new(io::ErrorKind::Other, "invalid port"))
        }

        match self.rfind(":") {
            Some(idx) => {
                let host = &self[..idx];
                let port = try!(parse_port(&self[idx+1..]));
                Ok(Endpoint::Host(host, port))
            }
            None => {
                Err(io::Error::new(io::ErrorKind::Other, "invalid endpoint"))
            }
        }
    }
}

#[test]
fn test_resolve_localhost() {
    let resolver = CpuPoolResolver::new(1);

    let fut = resolver.resolve("localhost").and_then(|addrs| {
        for addr in addrs {
            // TODO 1.12 addr.is_loopback()
            assert!(match addr {
                IpAddr::V4(a) => a.is_loopback(),
                IpAddr::V6(a) => a.is_loopback(),
            });
        }
        Ok(())
    });

    let _ = fut.wait();
}

#[test]
fn test_endpoint_str_port() {
    use std::net::Ipv4Addr;

    let ep = ("0.0.0.0", 1227).to_endpoint().unwrap();
    match ep {
        Endpoint::SocketAddr(addr) => {
            assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));
            assert_eq!(addr.port(), 1227);
        }
        _ => panic!(),
    }
}

#[test]
fn test_endpoint_str() {
    let ep = "localhost:1227".to_endpoint().unwrap();
    match ep {
        Endpoint::Host(host, port) => {
            assert_eq!(host, "localhost");
            assert_eq!(port, 1227);
        }
        _ => panic!(),
    }
}

#[test]
fn test_endpoint_str_ipv4() {
    use std::net::SocketAddrV4;

    let ep = "0.0.0.0:1227".to_endpoint().unwrap();
    match ep {
        Endpoint::SocketAddr(SocketAddr::V4(addr)) => {
            assert_eq!(addr, SocketAddrV4::from_str("0.0.0.0:1227").unwrap());
        }
        _ => panic!(),
    }
}


#[test]
fn test_endpoint_str_ipv6() {
    use std::net::SocketAddrV6;

    let ep = "[::]:1227".to_endpoint().unwrap();
    match ep {
        Endpoint::SocketAddr(SocketAddr::V6(addr)) => {
            assert_eq!(addr, SocketAddrV6::from_str("[::]:1227").unwrap());
        }
        _ => panic!(),
    }
}
