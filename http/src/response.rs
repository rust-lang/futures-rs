use std::fmt::Write;

use futures::*;
use futuremio::*;

use io2::*;

pub struct Response {
    headers: Vec<(String, String)>,
    response: String,
}

impl Response {
    pub fn new() -> Response {
        Response {
            headers: Vec::new(),
            response: String::new(),
        }
    }

    pub fn header(&mut self, name: &str, val: &str) -> &mut Response {
        self.headers.push((name.to_string(), val.to_string()));
        self
    }

    pub fn body(&mut self, s: &str) -> &mut Response {
        self.response = s.to_string();
        self
    }

    pub fn send(mut self, on: TcpStream) -> Box<IoFuture<()>> {
        let on = BufWriter::new(on);
        let header = format!("HTTP/1.1 200 OK\r\n");
        let mut f = on.write_all(0, header.into_bytes());

        for (k, v) in self.headers.drain(..) {
            f = f.and_then(move |(s, _, buf)| {
                let mut buf = b2s(buf);
                let _ = write!(buf, "{}: {}\r\n", k, v);
                s.write_all(0, buf.into_bytes())
            }).boxed();
        }

        f.and_then(|(s, _, buf)| {
            let mut buf = b2s(buf);
            let _ = write!(buf, "\r\n");
            s.write_all(0, buf.into_bytes())
        }).and_then(move |(s, _, _)| {
            s.write_all(0, self.response.into_bytes())
        }).map_err(From::from).and_then(|(mut s, _, _)| {
            s.flush()
        }).boxed()
        // }).map_err(From::from).map(|_| ()).boxed()
    }
}

fn b2s(mut b: Vec<u8>) -> String {
    b.truncate(0);
    String::from_utf8(b).unwrap()
}
