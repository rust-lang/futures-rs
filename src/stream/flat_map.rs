use std::sync::Arc;

use {Wake, Tokens};
use stream::{Stream, StreamResult};

pub struct FlatMap<S>
    where S: Stream,
{
    stream: S,
    next: Option<S::Item>,
}

pub fn new<S>(s: S) -> FlatMap<S>
    where S: Stream,
          S::Item: Stream,
          <S::Item as Stream>::Error: From<S::Error>,
{
    FlatMap {
        stream: s,
        next: None,
    }
}

impl<S> Stream for FlatMap<S>
    where S: Stream,
          S::Item: Stream,
          <S::Item as Stream>::Error: From<S::Error>,
{
    type Item = <S::Item as Stream>::Item;
    type Error = <S::Item as Stream>::Error;

    fn poll(&mut self, tokens: &Tokens)
            -> Option<StreamResult<Self::Item, Self::Error>> {
        loop {
            // TODO: reset tokens on each turn of the loop?
            if self.next.is_none() {
                match self.stream.poll(tokens) {
                    Some(Ok(Some(e))) => self.next = Some(e),
                    Some(Ok(None)) => return Some(Ok(None)),
                    Some(Err(e)) => return Some(Err(From::from(e))),
                    None => return None,
                }
            }
            assert!(self.next.is_some());
            // TODO: Tokens::all() if we just created the stream
            match self.next.as_mut().unwrap().poll(tokens) {
                Some(Ok(None)) => self.next = None,
                other => return other,
            }
        }
    }

    fn schedule(&mut self, wake: Arc<Wake>) {
        match self.next {
            Some(ref mut s) => s.schedule(wake),
            None => self.stream.schedule(wake),
        }
    }
}
