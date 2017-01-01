use {Poll, Async};
use stream::Stream;

/// A stream which checks that `poll` is not called after stream is terminated.
///
/// `Stream` contract prohibit calling `poll` after EOF. However some streams
/// implementations, e. g. `empty`, continue to return EOF after EOF. This
/// stream wrapper can be used for runtime check that poll is not called after
/// EOF, which can be used to debug code or in tests.
#[must_use = "streams do nothing unless polled"]
pub struct Checked<S> {
    stream: Option<S>,
}

pub fn new<S: Stream>(s: S) -> Checked<S> {
    Checked { stream: Some(s) }
}

impl<S: Stream> Stream for Checked<S> {
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<S::Item>, S::Error> {
        let ret = self.stream.as_mut()
            .expect("must not call poll after EOF")
            .poll();
        if let Ok(Async::Ready(None)) = ret {
            self.stream = None;
        }
        ret
    }
}
