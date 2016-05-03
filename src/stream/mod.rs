use {Future, PollResult, Callback};

mod channel;
pub use self::channel::{channel, Sender, Receiver};

pub type StreamResult<T, E> = PollResult<Option<T>, E>;

// TODO: Send + 'static is unfortunate
pub trait Stream/*: Send + 'static*/ {
    type Item: Send + 'static;
    type Error: Send + 'static;

    fn poll(&mut self) -> Option<StreamResult<Self::Item, Self::Error>>;

    fn cancel(&mut self);

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(StreamResult<Self::Item, Self::Error>) + Send + 'static,
              Self: Sized;

    fn schedule_boxed(&mut self,
                      g: Box<Callback<Option<Self::Item>, Self::Error>>);

    // fn map<U, F>(self, f: F) -> Map<Self, F>
    //     where F: FnMut(Self::Item) -> U + Send + 'static,
    //           U: Send + 'static,
    //           Self: Sized
    // {
    //     Map {
    //         stream: self,
    //         f: f,
    //     }
    // }
    //
    // fn map_err<U, F>(self, f: F) -> MapErr<Self, F>
    //     where F: FnMut(Self::Error) -> U + Send + 'static,
    //           U: Send + 'static,
    //           Self: Sized
    // {
    //     MapErr {
    //         stream: self,
    //         f: f,
    //     }
    // }
    //
    // fn filter<F>(self, f: F) -> Filter<Self, F>
    //     where F: FnMut(&Self::Item) -> bool + Send + 'static,
    //           Self: Sized
    // {
    //     Filter {
    //         stream: self,
    //         f: f,
    //     }
    // }
    //
    // // TODO: is this the same as map + flatten?
    // fn and_then<F, U>(self, f: F) -> AndThen<Self, F, U>
    //     where F: FnMut(Self::Item) -> U + Send + 'static,
    //           U: IntoFuture<Error=Self::Error>,
    //           Self: Sized
    // {
    //     AndThen {
    //         stream: self,
    //         f: f,
    //         future: None,
    //     }
    // }
    //
    // // TODO: is this the same as map_err + flatten?
    // fn or_else<F, U>(self, f: F) -> OrElse<Self, F, U>
    //     where F: FnMut(Self::Error) -> U + Send + 'static,
    //           U: IntoFuture<Item=Self::Item>,
    //           Self: Sized
    // {
    //     OrElse {
    //         stream: self,
    //         f: f,
    //         future: None,
    //     }
    // }
    //
    // // TODO: compare with fold + push
    // fn collect(self) -> Collect<Self> where Self: Sized {
    //     Collect {
    //         stream: self,
    //         elems: Vec::new(),
    //     }
    // }
    //
    // fn fold<F, T>(self, init: T, f: F) -> Fold<Self, F, T>
    //     where F: FnMut(T, Self::Item) -> T + Send + 'static,
    //           T: Send + 'static,
    //           Self: Sized
    // {
    //     Fold {
    //         stream: self,
    //         f: f,
    //         val: Some(init),
    //     }
    // }
    //
    // fn flatten(self) -> Flatten<Self>
    //     where Self::Item: IntoFuture,
    //           <<Self as Stream>::Item as IntoFuture>::Error:
    //                 From<<Self as Stream>::Error>,
    //           Self: Sized
    // {
    //     Flatten {
    //         stream: self,
    //         future: None,
    //     }
    // }
    //
    // fn flat_map(self) -> FlatMap<Self>
    //     where Self::Item: Stream,
    //           <<Self as Stream>::Item as Stream>::Error:
    //                 From<<Self as Stream>::Error>,
    //           Self: Sized
    // {
    //     FlatMap {
    //         outer: self,
    //         inner: None,
    //     }
    // }
}

impl<S: ?Sized + Stream> Stream for Box<S> {
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Option<StreamResult<Self::Item, Self::Error>> {
        (**self).poll()
    }

    fn cancel(&mut self) {
        (**self).cancel()
    }

    fn schedule<G>(&mut self, g: G)
        where G: FnOnce(StreamResult<Self::Item, Self::Error>) +
                    Send + 'static {
        self.schedule_boxed(Box::new(g))
    }

    fn schedule_boxed(&mut self,
                      g: Box<Callback<Option<Self::Item>, Self::Error>>) {
        (**self).schedule_boxed(g)
    }
}

// pub struct FutureStream<F> {
//     inner: Option<F>,
// }
//
// impl<T, F> FutureStream<F>
//     where F: Future<Item=Option<(T, F)>>,
//           T: Send + 'static,
// {
//     pub fn new(future: F) -> FutureStream<F> {
//         FutureStream { inner: Some(future) }
//     }
// }
//
// impl<T, F> Stream for FutureStream<F>
//     where F: Future<Item=Option<(T, F)>>,
//           T: Send + 'static,
// {
//     type Item = T;
//     type Error = F::Error;
//
//     fn poll(&mut self) -> Result<Self::Item, PollError<Self::Error>> {
//         let future = match self.inner.take() {
//             Some(f) => f,
//             None => return Err(PollError::Empty),
//         };
//         match future.poll() {
//             Ok(Ok(Some((t, future)))) => {
//                 self.inner = Some(future);
//                 Ok(t)
//             }
//             Ok(Ok(None)) => Err(PollError::Empty),
//             Ok(Err(e)) => Err(PollError::Other(e)),
//             Err(future) => {
//                 self.inner = Some(future);
//                 Err(PollError::NotReady)
//             }
//         }
//     }
//
//     fn schedule<G>(mut self, g: G)
//         where G: FnOnce(Option<Result<Self::Item, Self::Error>>, Self) +
//                     Send + 'static
//     {
//         match self.inner.take() {
//             Some(future) => {
//                 // TODO: on error this means that when we hit an error the
//                 //       future disappears into the ether, is... that ok?
//                 future.schedule(move |item| {
//                     match item {
//                         Ok(Some((t, next))) => {
//                             g(Some(Ok(t)), FutureStream { inner: Some(next) })
//                         }
//                         Ok(None) => {
//                             g(None, FutureStream { inner: None })
//                         }
//                         Err(e) => {
//                             g(Some(Err(e)), FutureStream { inner: None })
//                         }
//                     }
//                 })
//             }
//             None => g(None, self),
//         }
//     }
// }
//
// pub struct Map<S, F> {
//     stream: S,
//     f: F,
// }
//
// impl<S, F, U> Stream for Map<S, F>
//     where S: Stream,
//           F: FnMut(S::Item) -> U + Send + 'static,
//           U: Send + 'static,
// {
//     type Item = U;
//     type Error = S::Error;
//
//     fn poll(&mut self) -> Result<Self::Item, PollError<Self::Error>> {
//         self.stream.poll().map(&mut self.f)
//     }
//
//     fn schedule<G>(self, g: G)
//         where G: FnOnce(Option<Result<Self::Item, Self::Error>>, Self) +
//                     Send + 'static,
//     {
//         let Map { stream, mut f } = self;
//         stream.schedule(move |result, stream| {
//             g(result.map(|o| o.map(&mut f)), Map { stream: stream, f: f })
//         })
//     }
// }
//
// pub struct MapErr<S, F> {
//     stream: S,
//     f: F,
// }
//
// impl<S, F, U> Stream for MapErr<S, F>
//     where S: Stream,
//           F: FnMut(S::Error) -> U + Send + 'static,
//           U: Send + 'static,
// {
//     type Item = S::Item;
//     type Error = U;
//
//     fn poll(&mut self) -> Result<Self::Item, PollError<Self::Error>> {
//         self.stream.poll().map_err(|e| {
//             e.map(&mut self.f)
//         })
//     }
//
//     fn schedule<G>(self, g: G)
//         where G: FnOnce(Option<Result<Self::Item, Self::Error>>, Self) +
//                     Send + 'static,
//     {
//         let MapErr { stream, mut f } = self;
//         stream.schedule(move |result, s| {
//             g(result.map(|r| r.map_err(&mut f)), MapErr { stream: s, f: f })
//         })
//     }
// }
//
// pub struct Filter<S, F> {
//     stream: S,
//     f: F,
// }
//
// impl<S, F> Stream for Filter<S, F>
//     where S: Stream,
//           F: FnMut(&S::Item) -> bool + Send + 'static,
// {
//     type Item = S::Item;
//     type Error = S::Error;
//
//     fn poll(&mut self) -> Result<Self::Item, PollError<Self::Error>> {
//         loop {
//             match self.stream.poll() {
//                 Ok(item) => {
//                     if (self.f)(&item) {
//                         return Ok(item)
//                     }
//                 }
//                 Err(e) => return Err(e),
//             }
//         }
//     }
//
//     fn schedule<G>(self, g: G)
//         where G: FnOnce(Option<Result<Self::Item, Self::Error>>, Self) +
//                     Send + 'static,
//     {
//         let Filter { stream, f } = self;
//         stream.schedule(move |result, s| {
//             let mut me = Filter { stream: s, f: f };
//             if let Some(Ok(ref item)) = result {
//                 if !(me.f)(item) {
//                     return me.schedule(g)
//                 }
//             }
//             g(result, me)
//         })
//     }
// }
//
// pub struct AndThen<S, F, U> where U: IntoFuture {
//     stream: S,
//     f: F,
//     future: Option<U::Future>,
// }
//
// impl<S, F, U> Stream for AndThen<S, F, U>
//     where S: Stream,
//           F: FnMut(S::Item) -> U + Send + 'static,
//           U: IntoFuture<Error=S::Error>,
// {
//     type Item = U::Item;
//     type Error = S::Error;
//
//     fn poll(&mut self) -> Result<Self::Item, PollError<Self::Error>> {
//         let future = match self.future.take() {
//             Some(f) => f,
//             None => (self.f)(try!(self.stream.poll())).into_future(),
//         };
//
//         match future.poll() {
//             Ok(Ok(item)) => Ok(item),
//             Ok(Err(e)) => Err(PollError::Other(e)),
//             Err(f) => {
//                 self.future = Some(f);
//                 Err(PollError::NotReady)
//             }
//         }
//     }
//
//     fn schedule<G>(mut self, g: G)
//         where G: FnOnce(Option<Result<Self::Item, Self::Error>>, Self) +
//                     Send + 'static,
//     {
//         match self.future.take() {
//             Some(future) => future.schedule(|result| g(Some(result), self)),
//             None => {
//                 let AndThen { stream, f, future: _ } = self;
//                 stream.schedule(move |result, s| {
//                     let mut me = AndThen { stream: s, f: f, future: None };
//                     match result {
//                         Some(Ok(item)) => {
//                             me.future = Some((me.f)(item).into_future());
//                             me.schedule(g)
//                         }
//                         Some(Err(e)) => g(Some(Err(e)), me),
//                         None => g(None, me),
//                     }
//                 })
//             }
//         }
//     }
// }
//
// pub struct OrElse<S, F, U> where U: IntoFuture {
//     stream: S,
//     f: F,
//     future: Option<U::Future>,
// }
//
// impl<S, F, U> Stream for OrElse<S, F, U>
//     where S: Stream,
//           F: FnMut(S::Error) -> U + Send + 'static,
//           U: IntoFuture<Item=S::Item>,
// {
//     type Item = S::Item;
//     type Error = U::Error;
//
//     fn poll(&mut self) -> Result<Self::Item, PollError<Self::Error>> {
//         let future = match self.future.take() {
//             Some(f) => f,
//             None => {
//                 match self.stream.poll() {
//                     Ok(item) => return Ok(item),
//                     Err(PollError::Empty) => return Err(PollError::Empty),
//                     Err(PollError::NotReady) => return Err(PollError::NotReady),
//                     Err(PollError::Other(e)) => (self.f)(e).into_future(),
//                 }
//             }
//         };
//
//         match future.poll() {
//             Ok(Ok(item)) => Ok(item),
//             Ok(Err(e)) => Err(PollError::Other(e)),
//             Err(f) => {
//                 self.future = Some(f);
//                 Err(PollError::NotReady)
//             }
//         }
//     }
//
//     fn schedule<G>(mut self, g: G)
//         where G: FnOnce(Option<Result<Self::Item, Self::Error>>, Self) +
//                     Send + 'static,
//     {
//         match self.future.take() {
//             Some(future) => future.schedule(|result| g(Some(result), self)),
//             None => {
//                 let OrElse { stream, f, future: _ } = self;
//                 stream.schedule(move |result, s| {
//                     let mut me = OrElse { stream: s, f: f, future: None };
//                     match result {
//                         Some(Ok(item)) => g(Some(Ok(item)), me),
//                         Some(Err(e)) => {
//                             me.future = Some((me.f)(e).into_future());
//                             me.schedule(g)
//                         }
//                         None => g(None, me),
//                     }
//                 })
//             }
//         }
//     }
// }
//
// pub struct Collect<S> where S: Stream {
//     stream: S,
//     elems: Vec<S::Item>,
// }
//
// impl<S: Stream> Future for Collect<S> {
//     type Item = Vec<S::Item>;
//     type Error = S::Error;
//
//     fn poll(mut self) -> Result<Result<Self::Item, Self::Error>, Self> {
//         loop {
//             match self.stream.poll() {
//                 Ok(t) => self.elems.push(t),
//                 Err(PollError::Empty) => return Ok(Ok(self.elems)),
//                 Err(PollError::NotReady) => return Err(self),
//                 Err(PollError::Other(e)) => return Ok(Err(e)),
//             }
//         }
//     }
//
//     fn schedule<F>(self, f: F)
//         where F: FnOnce(Result<Self::Item, Self::Error>) + Send + 'static
//     {
//         let Collect { stream, mut elems } = self;
//         stream.schedule(move |result, stream| {
//             match result {
//                 Some(Ok(item)) => {
//                     elems.push(item);
//                     Collect { stream: stream, elems: elems }.schedule(f)
//                 }
//                 Some(Err(e)) => f(Err(e)),
//                 None => f(Ok(elems)),
//             }
//         })
//     }
// }
//
// pub struct Fold<S, F, T> {
//     stream: S,
//     f: F,
//     val: Option<T>,
// }
//
// impl<S, F, T> Fold<S, F, T>
//     where S: Stream,
//           F: FnMut(T, S::Item) -> T + Send + 'static,
//           T: Send + 'static,
// {
//     fn process(&mut self, item: S::Item) {
//         self.val = Some((self.f)(self.val.take().unwrap(), item));
//     }
// }
//
// impl<S, F, T> Future for Fold<S, F, T>
//     where S: Stream,
//           F: FnMut(T, S::Item) -> T + Send + 'static,
//           T: Send + 'static,
// {
//     type Item = T;
//     type Error = S::Error;
//
//     fn poll(mut self) -> Result<Result<Self::Item, Self::Error>, Self> {
//         loop {
//             match self.stream.poll() {
//                 Ok(item) => self.process(item),
//                 Err(PollError::Empty) => return Ok(Ok(self.val.take().unwrap())),
//                 Err(PollError::NotReady) => return Err(self),
//                 Err(PollError::Other(e)) => return Ok(Err(e)),
//             }
//         }
//     }
//
//     fn schedule<G>(self, g: G)
//         where G: FnOnce(Result<Self::Item, Self::Error>) + Send + 'static
//     {
//         let Fold { stream, val, f } = self;
//         stream.schedule(|result, stream| {
//             let mut me = Fold { stream: stream, val: val, f: f };
//             match result {
//                 Some(Ok(item)) => {
//                     me.process(item);
//                     me.schedule(g);
//                 }
//                 Some(Err(e)) => g(Err(e)),
//                 None => g(Ok(me.val.take().unwrap())),
//             }
//         })
//     }
// }
//
// pub struct Flatten<S> where S: Stream, S::Item: IntoFuture {
//     stream: S,
//     future: Option<<S::Item as IntoFuture>::Future>,
// }
//
// impl<S> Stream for Flatten<S>
//     where S: Stream,
//           S::Item: IntoFuture,
//           <<S as Stream>::Item as IntoFuture>::Error: From<<S as Stream>::Error>
// {
//     type Item = <S::Item as IntoFuture>::Item;
//     type Error = <S::Item as IntoFuture>::Error;
//
//     fn poll(&mut self) -> Result<Self::Item, PollError<Self::Error>> {
//         let future = match self.future.take() {
//             Some(f) => f,
//             None => {
//                 try!(self.stream.poll().map_err(|e| {
//                     e.map(From::from)
//                 })).into_future()
//             }
//         };
//         match future.poll() {
//             Ok(Ok(item)) => Ok(item),
//             Ok(Err(e)) => Err(PollError::Other(e)),
//             Err(f) => {
//                 self.future = Some(f);
//                 Err(PollError::NotReady)
//             }
//         }
//     }
//
//     fn schedule<G>(mut self, g: G)
//         where G: FnOnce(Option<Result<Self::Item, Self::Error>>, Self) +
//                     Send + 'static,
//     {
//         match self.future.take() {
//             Some(f) => f.schedule(|result| g(Some(result), self)),
//             None => {
//                 self.stream.schedule(|result, stream| {
//                     let mut me = Flatten { stream: stream, future: None };
//                     match result {
//                         Some(Ok(f)) => {
//                             me.future = Some(f.into_future());
//                             me.schedule(g)
//                         }
//                         Some(Err(e)) => g(Some(Err(From::from(e))), me),
//                         None => g(None, me),
//                     }
//                 })
//             }
//         }
//     }
// }
//
// pub struct FlatMap<S> where S: Stream, S::Item: Stream {
//     outer: S,
//     inner: Option<S::Item>,
// }
//
// impl<S> Stream for FlatMap<S>
//     where S: Stream,
//           S::Item: Stream,
//           <<S as Stream>::Item as Stream>::Error: From<<S as Stream>::Error>
// {
//     type Item = <S::Item as Stream>::Item;
//     type Error = <S::Item as Stream>::Error;
//
//     fn poll(&mut self) -> Result<Self::Item, PollError<Self::Error>> {
//         loop {
//             match self.inner.as_mut().map(|s| s.poll()) {
//                 Some(Err(PollError::Empty)) => self.inner = None,
//                 Some(other) => return other,
//                 None => {}
//             }
//
//             match self.outer.poll() {
//                 Ok(inner) => self.inner = Some(inner),
//                 Err(e) => return Err(e.map(From::from)),
//             }
//         }
//     }
//
//     fn schedule<G>(mut self, g: G)
//         where G: FnOnce(Option<Result<Self::Item, Self::Error>>, Self) +
//                     Send + 'static,
//     {
//         match self.inner.take() {
//             Some(inner) => {
//                 inner.schedule(|result, inner| {
//                     match result {
//                         Some(item) => {
//                             self.inner = Some(inner);
//                             g(Some(item), self);
//                         }
//                         None => self.schedule(g),
//                     }
//                 })
//             }
//             None => {
//                 self.outer.schedule(|result, outer| {
//                     let mut me = FlatMap { inner: None, outer: outer };
//                     match result {
//                         Some(Ok(inner)) => {
//                             me.inner = Some(inner);
//                             me.schedule(g);
//                         }
//                         Some(Err(e)) => g(Some(Err(From::from(e))), me),
//                         None => g(None, me),
//                     }
//                 })
//             }
//         }
//     }
// }

// TODO: IntoStream?
//
// impl<I> Stream for I
//     where I: Iterator,
//           I: Send + 'static,
//           I::Item: Send + 'static,
// {
//     type Item = I::Item;
//     type Error = ();
//
//     fn poll(&mut self) -> Result<Self::Item, PollError<Self::Error>> {
//         match self.next() {
//             Some(item) => Ok(item),
//             None => Err(PollError::Empty),
//         }
//     }
//
//     fn schedule<G>(mut self, g: G)
//         where G: FnOnce(Option<Result<Self::Item, Self::Error>>, Self) +
//                     Send + 'static,
//     {
//         g(self.next().map(Ok), self)
//     }
// }
