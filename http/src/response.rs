use std::io::Write;

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
        write!(buf, "HTTP/1.1 200 OK\r\n");

        for &(ref k, ref v) in &self.headers {
            write!(buf, "{}: {}\r\n", k, v);
        }

        write!(buf, "\r\n");
        write!(buf, "{}", self.response);
    }
}
