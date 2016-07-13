use std::io;
use std::slice;

use httparse;

use io2::Parse;

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

impl Parse for Request {
    type Parser = ();
    // FiXME: probably want a different error type
    type Error = io::Error;

    fn parse(_: &mut (), buf: &[u8]) -> Option<Result<(Request, usize), io::Error>> {
        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut r = httparse::Request::new(&mut headers);
        let status = match r.parse(&buf) {
            Ok(status) => status,
            Err(e) => {
                return Some(Err(io::Error::new(io::ErrorKind::Other,
                                               format!("failed to parse http request: {:?}", e))))
            }
        };
        match status {
            httparse::Status::Complete(amt) => {
                Some(Ok((Request {
                    method: r.method.unwrap().to_string(),
                    path: r.path.unwrap().to_string(),
                    version: r.version.unwrap(),
                    headers: r.headers
                        .iter()
                        .map(|h| (h.name.to_string(), h.value.to_owned()))
                        .collect(),
                }, amt)))
            }
            httparse::Status::Partial => None
        }
    }
}

impl<'req> Iterator for RequestHeaders<'req> {
    type Item = (&'req str, &'req [u8]);

    fn next(&mut self) -> Option<(&'req str, &'req [u8])> {
        self.headers.next().map(|&(ref a, ref b)| (&a[..], &b[..]))
    }
}
