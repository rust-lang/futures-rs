use std::sync::Arc;

use {Wake, Future, Tokens, TOKENS_EMPTY};
use executor::{Executor, DEFAULT};

pub enum Collapsed<T: Future> {
    Start(T),
    Tail(Box<Future<Item=T::Item, Error=T::Error>>),
}

impl<T: Future> Collapsed<T> {
    pub fn poll(&mut self, tokens: &Tokens) -> Option<Result<T::Item, T::Error>> {
        match *self {
            Collapsed::Start(ref mut a) => a.poll(tokens),
            Collapsed::Tail(ref mut a) => a.poll(tokens),
        }
    }

    pub fn schedule(&mut self, wake: Arc<Wake>) {
        match *self {
            Collapsed::Start(ref mut a) => a.schedule(wake),
            Collapsed::Tail(ref mut a) => a.schedule(wake),
        }
    }

    pub fn collapse(&mut self) {
        let a = match *self {
            Collapsed::Start(ref mut a) => {
                match a.tailcall() {
                    Some(a) => a,
                    None => return,
                }
            }
            Collapsed::Tail(ref mut a) => {
                if let Some(b) = a.tailcall() {
                    *a = b;
                }
                return
            }
        };
        *self = Collapsed::Tail(a);
    }
}

pub fn done(wake: Arc<Wake>) {
    DEFAULT.execute(move || wake.wake(&TOKENS_EMPTY));
}
