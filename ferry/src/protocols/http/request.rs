use std::io;
use std::slice;
use std::str;

use httparse;

use futures::Poll;
use futuremio::InputBuf;
use mio_server::Decode;

struct Indexes {
    method: Slice,
    path: Slice,
    version: u8,
    // TODO: use a small vec to avoid this unconditional allocation
    headers: Vec<(Slice, Slice)>,
}

pub struct Request {
    indexes: Indexes,
    data: InputBuf,
}

type Slice = (usize, usize);

pub struct RequestHeaders<'req> {
    headers: slice::Iter<'req, (Slice, Slice)>,
    req: &'req Request,
}

impl Request {
    pub fn method(&self) -> &str {
        str::from_utf8(self.slice(&self.indexes.method)).unwrap()
    }

    pub fn path(&self) -> &str {
        str::from_utf8(self.slice(&self.indexes.path)).unwrap()
    }

    pub fn version(&self) -> u8 {
        self.indexes.version
    }

    pub fn headers(&self) -> RequestHeaders {
        RequestHeaders {
            headers: self.indexes.headers.iter(),
            req: self,
        }
    }

    fn slice(&self, s: &Slice) -> &[u8] {
        &self.data[s.0..s.1]
    }
}

impl Decode for Request {
    type Decoder = ();
    // FiXME: probably want a different error type
    type Error = io::Error;

    fn decode(_: &mut (), buf: &mut InputBuf) -> Poll<Request, io::Error> {
        // TODO: we should grow this headers array if parsing fails and asks for
        //       more headers
        let (amt, indexes) = {
            let mut headers = [httparse::EMPTY_HEADER; 16];
            let mut r = httparse::Request::new(&mut headers);
            let status = match r.parse(&buf) {
                Ok(status) => status,
                Err(e) => {
                    return Poll::Err(io::Error::new(io::ErrorKind::Other,
                                                    format!("failed to parse http request: {:?}", e)))
                }
            };
            let toslice = |a: &[u8]| {
                let start = a.as_ptr() as usize - buf.as_ptr() as usize;
                assert!(start < buf.len());
                (start, start + a.len())
            };
            match status {
                httparse::Status::Complete(amt) =>
                    (amt, Indexes {
                        method: toslice(r.method.unwrap().as_bytes()),
                        path: toslice(r.path.unwrap().as_bytes()),
                        version: r.version.unwrap(),
                        headers: r.headers
                            .iter()
                            .map(|h| (toslice(h.name.as_bytes()), toslice(h.value)))
                            .collect(),
                    }),
                httparse::Status::Partial => return Poll::NotReady,
            }
        };

        Poll::Ok(Request {
            indexes: indexes,
            data: buf.take(amt),
        })
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
