use poll::Poll;
use stream::{Stream, Fuse};
use std::prelude::v1::*;
use std::sync::{Arc, Mutex};

/// A stream which records all of the values of a stream as a history.
///
/// A history of type `HistoryType::Full` will keep a full history
/// and any future consumers that create a clone of the stream
/// will recieve all items produced so far
///
/// A history of type `HistoryType::Partial` will only keep a history
/// of items of items starting from the first clone. Subsequent clones
/// will not recieve any history from before the clone. This is similiar
/// to a Pub/sub model.
///
/// This stream is created by the `Stream::history` or `Stream::full_history`
/// method.
pub struct History<S: Stream>
    where <S as Stream>::Item: Clone {
    cursor: (usize, usize),
    history: Vec<S::Item>,
    inner: Arc<Mutex<ProtectedHistory<S>>>
}

pub enum HistoryType {
    Full(Option<usize>),
    Partial(Option<usize>),
}


pub fn new<S: Stream>(stream: S, type_of: HistoryType) -> History<S>
    where <S as Stream>::Item: Clone {
    let mut v = Vec::new();
    v.push(0);
    History {
        cursor: (0,0),
        history: Vec::new(),
        inner: Arc::new(Mutex::new(ProtectedHistory {
            inner: stream.fuse(),
            history: history::<S>(type_of),
            index: 0,
            cursors: v
        }))
    }
}

struct ProtectedHistory<S: Stream>
    where <S as Stream>::Item: Clone
{
    inner: Fuse<S>,
    history: HistoryHelper<S::Item>,
    index: usize,
    cursors: Vec<usize>
}

impl<S: Stream> ProtectedHistory<S>
    where <S as Stream>::Item: Clone
{
    fn poll(&mut self) -> Poll<Option<S::Item>, S::Error> {
        let res = self.inner.poll();
        match res {
            Poll::Ok(Some(item)) => {
                self.history.push(item.clone());
                self.index = self.index.wrapping_add(1);
                Poll::Ok(Some(item))
            }
            otherwise => otherwise
        }
    }
}

enum HistoryHelper<I: Clone> {
    Full(Vec<I>, Option<usize>),
    Partial(Vec<I>, Option<usize>),
}

impl<I: Clone> HistoryHelper<I> {
    fn is_full(&self) -> bool {
        match *self {
            HistoryHelper::Full(_,_) => true,
            _ => false
        }
    }

    fn clone_vec(&self) -> Vec<I> {
        match self {
            &HistoryHelper::Full(ref v,_) => v.clone(),
            &HistoryHelper::Partial(ref v,_) => v.clone(),
        }
    }

    fn push(&mut self, item: I) {
        match *self {
            HistoryHelper::Full(ref mut v, _limit) => {
                v.push(item)
            },
            HistoryHelper::Partial(ref mut v,_limit) => {
                v.push(item);
                // maybe flush items
            },
        }
    }

    fn get_history(&self, cursor: (usize, usize)) -> Option<Vec<I>> {
        match self {
            &HistoryHelper::Full(ref v,_) => {
                if v.len() > cursor.0 {
                    let mut ret = Vec::new();
                    let slice = &v[cursor.0..];
                    ret.reserve_exact(slice.len());
                    unsafe {
                        ret.set_len(slice.len());
                    }
                    ret.clone_from_slice(slice);
                    return Some(ret)
                }
                None
            }
            &HistoryHelper::Partial(ref v,_) => {
                if v.len() > cursor.0 {
                    let mut ret = Vec::new();
                    let slice = &v[cursor.0..];
                    ret.reserve_exact(slice.len());
                    unsafe {
                        ret.set_len(slice.len());
                    }
                    ret.clone_from_slice(slice);
                    return Some(ret)
                }
                None
            }
        }
    }
}

fn history<S: Stream>(type_of: HistoryType) -> HistoryHelper<S::Item>
    where <S as Stream>::Item: Clone
{
    match type_of {
        HistoryType::Full(limit) => HistoryHelper::Full(Vec::new(), limit),
        HistoryType::Partial(limit) => HistoryHelper::Partial(Vec::new(), limit),
    }
}

impl<S> Stream for History<S>
    where S: Stream,
         <S as Stream>::Item: Clone
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if let Some(item) = self.history.pop() {
            return Poll::Ok(Some(item))
        }
        let mut protected = self.inner.lock().unwrap();
        // first try fetch some history
        if let Some(mut history) = protected.history.get_history(self.cursor) {
            self.cursor.0 += history.len();
            history.reverse();
            let item = history.pop().unwrap();
            self.history = history;
            return Poll::Ok(Some(item))            
        }
        // if that fails then poll
        let ret = protected.poll();
        if let Poll::Ok(Some(_)) = ret {
            self.cursor.0 += 1;
        }
        ret
    }
}

impl<S: Stream> Clone for History<S> 
    where <S as Stream>::Item: Clone
{
    fn clone(&self) -> History<S> {
        let (cursor, history) = {
            let mut protected = self.inner.lock().unwrap();
            let is_full = protected.history.is_full();
            if is_full {
                let mut history = protected.history.clone_vec();
                history.reverse();
                protected.cursors.push(0);
                let cursor = (history.len(), protected.cursors.len());
                (cursor, history)
            } else {
                let cursor = protected.index;
                let history = Vec::new();
                protected.cursors.push(cursor);
                let cursor = (cursor, protected.cursors.len());
                (cursor, history)
            }
        };
        History {
            cursor: cursor,
            history: history,
            inner: self.inner.clone()
        }
    }
}
