use std::io;
use std::slice;

use futures::*;
use futuremio::*;
use httparse;

pub struct Request {
    method: String,
    path: String,
    version: u8,
    headers: Vec<(String, Vec<u8>)>,
}

pub struct RequestHeaders<'req> {
    headers: slice::Iter<'req, (String, Vec<u8>)>,
}

impl Request {
    pub fn new(s: TcpStream) -> Box<IoFuture<(Request, TcpStream)>> {
        Request::parse(s, Vec::with_capacity(16))
    }

    fn parse(s: TcpStream,
             mut buf: Vec<u8>) -> Box<IoFuture<(Request, TcpStream)>> {
        buf.reserve(1);
        let contents = s.read(buf);
        contents.map_err(From::from).and_then(move |buf| {
            {
                let mut headers = [httparse::EMPTY_HEADER; 16];
                let mut r = httparse::Request::new(&mut headers);
                let status = match r.parse(&buf) {
                    Ok(status) => status,
                    Err(e) => {
                        let err = io::Error::new(io::ErrorKind::Other,
                                                 format!("failed to parse http \
                                                          request: {:?}", e));
                        return failed(err).boxed()
                    }
                };
                match status {
                    httparse::Status::Complete(amt) => {
                        assert_eq!(amt, buf.len());
                        return finished((Request {
                            method: r.method.unwrap().to_string(),
                            path: r.path.unwrap().to_string(),
                            version: r.version.unwrap(),
                            headers: r.headers.iter().map(|h| {
                                (h.name.to_string(), h.value.to_owned())
                            }).collect(),
                        }, s)).boxed()
                    }
                    httparse::Status::Partial => {}
                }
            }
            Request::parse(s, buf).boxed()
        }).boxed()
    }

    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn version(&self) -> u8 {
        self.version
    }

    pub fn headers(&self) -> RequestHeaders {
        RequestHeaders { headers: self.headers.iter() }
    }
}

impl<'req> Iterator for RequestHeaders<'req> {
    type Item = (&'req str, &'req [u8]);

    fn next(&mut self) -> Option<(&'req str, &'req [u8])> {
        self.headers.next().map(|&(ref a, ref b)| {
            (&a[..], &b[..])
        })
    }
}
