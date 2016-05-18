use std::sync::Arc;

use Callback;
use slot::Slot;
use stream::{Stream, StreamResult};
use util;

pub struct Filter<S, F> {
    inner: Arc<_Filter<S, F>>,
}

struct _Filter<S, F> {
    stream: Slot<S>,
    f: Slot<F>,
}

pub fn new<S, F>(s: S, f: F) -> Filter<S, F>
    where F: Send + 'static,
          S: Stream,
{
    Filter {
        inner: Arc::new(_Filter {
            stream: Slot::new(Some(s)),
            f: Slot::new(Some(f)),
        }),
    }
}

impl<S, F> Filter<S, F>
    where S: Stream,
          F: FnMut(&S::Item) -> bool + Send + 'static,
{
    fn doit<G>(&self,
               mut stream: S,
               mut filter: F,
               g: G)
        where G: FnOnce(StreamResult<S::Item, S::Error>) + Send + 'static,
    {
        let slot = self.inner.clone();
        stream.schedule(move |res| {
            let (f, res) = match res {
                Ok(Some(e)) => {
                    match util::recover(|| (filter(&e), e, filter)) {
                        Ok((true, e, f)) => (Some(f), Ok(Some(e))),
                        Ok((false, _, f)) => {
                            let slot2 = slot.clone();
                            slot.stream.on_full(|slot| {
                                let stream = slot.try_consume().ok().unwrap();
                                Filter {
                                    inner: slot2,
                                }.doit(stream, f, g)
                            });
                            return
                        }
                        Err(e) => (None, Err(e)),
                    }
                }
                Ok(None) => (Some(filter), Ok(None)),
                Err(e) => (Some(filter), Err(e)),
            };
            if let Some(f) = f {
                slot.f.try_produce(f).ok().expect("map stream failed to produce");
            }
            g(res)
        });
        self.inner.stream.try_produce(stream).ok()
            .expect("filter failed to produce stream");
    }
}

impl<S, F> Stream for Filter<S, F>
    where S: Stream,
          F: FnMut(&S::Item) -> bool + Send + 'static,
{
    type Item = S::Item;
    type Error = S::Error;

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(StreamResult<Self::Item, Self::Error>) + Send + 'static,
    {
        let f = match util::opt2poll(self.inner.f.try_consume().ok()) {
            Ok(f) => f,
            Err(e) => return g(Err(e)),
        };
        let stream = self.inner.stream.try_consume().ok()
                             .expect("filter got closure, failed to get stream");
        self.doit(stream, f, g)
    }

    fn schedule_boxed(&mut self,
                      g: Box<Callback<Option<Self::Item>, Self::Error>>) {
        self.schedule(|r| g.call(r))
    }
}
