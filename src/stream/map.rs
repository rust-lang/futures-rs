use Callback;
use stream::{Stream, StreamResult};
use util;

pub struct Map<S, F> {
    stream: S,
    f: Option<F>,
}

pub fn new<S, F>(s: S, f: F) -> Map<S, F> {
    Map {
        stream: s,
        f: Some(f),
    }
}

impl<S, F, U> Stream for Map<S, F>
    where S: Stream,
          F: FnMut(S::Item) -> U + Send + 'static,
          U: Send + 'static,
{
    type Item = U;
    type Error = S::Error;

    // fn poll(&mut self) -> Option<StreamResult<Self::Item, Self::Error>> {
    //     self.stream.poll().map(|r| {
    //         r.and_then(|r| {
    //             let mut f = try!(util::opt2poll(self.f.take()));
    //             let e = match r {
    //                 Some(e) => e,
    //                 None => return Ok(None),
    //             };
    //             let (r, f) = try!(util::recover(|| (f(e), f)));
    //             self.f = Some(f);
    //             Ok(Some(r))
    //         })
    //     })
    // }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(StreamResult<Self::Item, Self::Error>) + Send + 'static,
    {
        let mut f = match util::opt2poll(self.f.take()) {
            Ok(f) => f,
            Err(e) => return g(Err(e)),
        };
        self.stream.schedule(move |res| {
            let e = match res {
                Ok(Some(e)) => e,
                Ok(None) => return g(Ok(None)),
                Err(e) => return g(Err(e)),
            };
            match util::recover(|| (f(e), f)) {
                Ok((r, f)) => {
                    // TODO: gotta put this back
                    drop(f);
                    g(Ok(Some(r)))
                }
                Err(e) => g(Err(e)),
            }
        })
    }

    fn schedule_boxed(&mut self,
                      g: Box<Callback<Option<Self::Item>, Self::Error>>) {
        self.schedule(|r| g.call(r))
    }
}

