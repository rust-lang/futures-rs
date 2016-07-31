use std::io;
use std::slice;
use std::str;
use std::sync::Arc;

use httparse;

use io2::Parse;

pub struct Request {
    method: Slice,
    path: Slice,
    version: u8,
    // TODO: use a small vec to avoid this unconditional allocation
    headers: Vec<(Slice, Slice)>,
    data: Arc<Vec<u8>>,
}

type Slice = (usize, usize);

pub struct RequestHeaders<'req> {
    headers: slice::Iter<'req, (Slice, Slice)>,
    req: &'req Request,
}

impl Request {
    pub fn method(&self) -> &str {
        str::from_utf8(self.slice(&self.method)).unwrap()
    }

    pub fn path(&self) -> &str {
        str::from_utf8(self.slice(&self.path)).unwrap()
    }

    pub fn version(&self) -> u8 {
        self.version
    }

    pub fn headers(&self) -> RequestHeaders {
        RequestHeaders {
            headers: self.headers.iter(),
            req: self,
        }
    }

    fn slice(&self, s: &Slice) -> &[u8] {
        &self.data[s.0..s.1]
    }
}

impl Parse for Request {
    type Parser = ();
    // FiXME: probably want a different error type
    type Error = io::Error;

    fn parse(_: &mut (),
             buf: &Arc<Vec<u8>>,
             offset: usize)
             -> Option<Result<(Request, usize), io::Error>> {
        // TODO: we should grow this headers array if parsing fails and asks for
        //       more headers
        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut r = httparse::Request::new(&mut headers);
        let status = match r.parse(&buf[offset..]) {
            Ok(status) => status,
            Err(e) => {
                return Some(Err(io::Error::new(io::ErrorKind::Other,
                                               format!("failed to parse http request: {:?}", e))))
            }
        };
        let toslice = |a: &[u8]| {
            let start = a.as_ptr() as usize - buf.as_ptr() as usize;
            assert!(start < buf.len());
            (start, start + a.len())
        };
        match status {
            httparse::Status::Complete(amt) => {
                Some(Ok((Request {
                    method: toslice(r.method.unwrap().as_bytes()),
                    path: toslice(r.path.unwrap().as_bytes()),
                    version: r.version.unwrap(),
                    headers: r.headers
                        .iter()
                        .map(|h| (toslice(h.name.as_bytes()), toslice(h.value)))
                        .collect(),
                    data: buf.clone(),
                }, amt)))
            }
            httparse::Status::Partial => None
        }
    }
}

impl<'req> Iterator for RequestHeaders<'req> {
    type Item = (&'req str, &'req [u8]);

    fn next(&mut self) -> Option<(&'req str, &'req [u8])> {
        self.headers.next().map(|&(ref a, ref b)| {
            let a = str::from_utf8(self.req.slice(a)).unwrap();
            let b = self.req.slice(b);
            (a, b)
        })
    }
}
