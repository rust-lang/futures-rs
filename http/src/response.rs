use std::io::Write;

use time;

use io2::Serialize;

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
}

impl Serialize for Response {
    fn serialize(&self, buf: &mut Vec<u8>) {
        write!(buf, "\
            HTTP/1.1 200 OK\r\n\
            Server: Example\r\n\
            Content-Length: {}\r\n\
            Date: {}\r\n\
        ", self.response.len(), time::now().rfc822()).unwrap();
        for &(ref k, ref v) in &self.headers {
            buf.extend_from_slice(k.as_bytes());
            buf.extend_from_slice(b": ");
            buf.extend_from_slice(v.as_bytes());
            buf.extend_from_slice(b"\r\n");
        }
        buf.extend_from_slice(b"\r\n");
        buf.extend_from_slice(self.response.as_bytes());
    }
}
