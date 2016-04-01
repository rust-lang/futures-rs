use std::mem;

use {Future, Callback, PollResult, FutureResult, PollError, FutureError};
use util;

pub struct Collect<I> where I: Iterator, I::Item: Future {
    remaining: Option<I>,
    cur: Option<I::Item>,
    result: Vec<<I::Item as Future>::Item>,
    canceled: bool,
}

pub fn collect<I>(i: I) -> Collect<I::IntoIter>
    where I: IntoIterator,
          I::Item: Future,
          I::IntoIter: Send + 'static,
{
    let mut i = i.into_iter();
    Collect {
        cur: i.next(),
        remaining: Some(i),
        result: Vec::new(),
        canceled: false,
    }
}

impl<I> Future for Collect<I>
    where I: Iterator + Send + 'static,
          I::Item: Future,
{
    type Item = Vec<<I::Item as Future>::Item>;
    type Error = <I::Item as Future>::Error;

    fn poll(&mut self) -> Option<PollResult<Self::Item, Self::Error>> {
        if self.canceled {
            return Some(Err(PollError::Canceled))
        }
        let remaining = match util::opt2poll(self.remaining.as_mut()) {
            Ok(i) => i,
            Err(e) => return Some(Err(e)),
        };
        loop {
            match self.cur.as_mut().map(Future::poll) {
                Some(Some(Ok(i))) => {
                    self.result.push(i);
                    self.cur = remaining.next();
                }
                Some(Some(Err(e))) => {
                    for mut item in remaining {
                        item.cancel();
                    }
                    return Some(Err(e))
                }
                Some(None) => return None,

                // TODO: two poll() calls should probably panic the second
                None => return Some(Ok(mem::replace(&mut self.result, Vec::new()))),
            }
        }
    }

    fn await(&mut self) -> FutureResult<Self::Item, Self::Error> {
        if self.canceled {
            return Err(FutureError::Canceled)
        }
        let remaining = try!(util::opt2poll(self.remaining.as_mut()));
        let mut err = None;
        for mut item in self.cur.take().into_iter().chain(remaining) {
            if err.is_some() {
                item.cancel();
            } else {
                match item.await() {
                    Ok(item) => self.result.push(item),
                    Err(e) => err = Some(e),
                }
            }
        }
        match err {
            Some(err) => Err(err),
            None => Ok(mem::replace(&mut self.result, Vec::new()))
        }
    }

    fn cancel(&mut self) {
        let remaining = match self.remaining.as_mut() {
            Some(iter) => iter,
            None => return,
        };
        let mut any_canceled = false;
        for mut item in self.cur.take().into_iter().chain(remaining) {
            item.cancel();
            any_canceled = true;
        }
        if any_canceled {
            self.canceled = true;
        }
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(PollResult<Self::Item, Self::Error>) + Send + 'static
    {
        loop {}
        // let mut result = mem::replace(&mut self.result, Vec::new());
        // let mut next = match self.cur.take() {
        //     Some(next) => next,
        //     None => return g(Ok(result)),
        // };
        // let mut remaining = match self.remaining.take() {
        //     Some(i) => i,
        //     None => return g(Err(FutureError::Panicked)),
        // };
        // next.schedule(move |res| {
        //     let item = match res {
        //         Ok(i) => i,
        //         Err(e) => return g(Err(e)),
        //     };
        //     result.push(item);
        //     Collect {
        //         cur: remaining.next(),
        //         remaining: Some(remaining),
        //         result: result,
        //     }.schedule(g);
        // })
    }

    fn schedule_boxed(&mut self, cb: Box<Callback<Self::Item, Self::Error>>) {
        self.schedule(|r| cb.call(r))
    }
}
