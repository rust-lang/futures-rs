use std::io::{self, Write};

use futuremio::BufWriter;
use mio_server::Encode;

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

impl<W: Write + Send + 'static> Encode<W> for Response {
    type Error = io::Error;
    type Fut = io::Result<BufWriter<W>>;

    fn encode(&self, mut writer: BufWriter<W>) -> io::Result<BufWriter<W>> {
        write!(writer, "\
            HTTP/1.1 200 OK\r\n\
            Server: Example\r\n\
            Content-Length: {}\r\n\
            Date: {}\r\n\
            ", self.response.len(), ::date::now()).unwrap();
        for &(ref k, ref v) in &self.headers {
            writer.extend(k.as_bytes());
            writer.extend(b": ");
            writer.extend(v.as_bytes());
            writer.extend(b"\r\n");
        }
        writer.extend(b"\r\n");
        writer.extend(self.response.as_bytes());
        Ok(writer)
    }

    fn size_hint(&self) -> Option<usize> {
        //Some(80 + self.response.len())
        None
    }
}
