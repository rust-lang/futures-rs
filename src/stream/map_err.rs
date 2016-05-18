use std::sync::Arc;

use {Callback, PollError};
use slot::Slot;
use stream::{Stream, StreamResult};
use util;

pub struct MapErr<S, F> {
    stream: S,
    f: Arc<Slot<F>>,
}

pub fn new<S, F>(s: S, f: F) -> MapErr<S, F> where F: Send + 'static {
    MapErr {
        stream: s,
        f: Arc::new(Slot::new(Some(f))),
    }
}

impl<S, F, U> Stream for MapErr<S, F>
    where S: Stream,
          F: FnMut(S::Error) -> U + Send + 'static,
          U: Send + 'static,
{
    type Item = S::Item;
    type Error = U;

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(StreamResult<Self::Item, Self::Error>) + Send + 'static,
    {
        let mut f = match util::opt2poll(self.f.try_consume().ok()) {
            Ok(f) => f,
            Err(e) => return g(Err(e)),
        };
        let slot = self.f.clone();
        self.stream.schedule(move |res| {
            let (f, res) = match res {
                Ok(e) => (Some(f), Ok(e)),
                Err(PollError::Other(e)) => {
                    match util::recover(|| (f(e), f)) {
                        Ok((r, f)) => (Some(f), Err(PollError::Other(r))),
                        Err(e) => (None, Err(e)),
                    }
                }
                Err(PollError::Panicked(e)) => {
                    (Some(f), Err(PollError::Panicked(e)))
                }
                Err(PollError::Canceled) => {
                    (Some(f), Err(PollError::Canceled))
                }
            };
            if let Some(f) = f {
                slot.try_produce(f).ok().expect("map_err stream failed to produce");
            }
            g(res)
        })
    }

    fn schedule_boxed(&mut self,
                      g: Box<Callback<Option<Self::Item>, Self::Error>>) {
        self.schedule(|r| g.call(r))
    }
}


