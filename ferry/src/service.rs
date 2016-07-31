use std::sync::Mutex;
use std::marker::PhantomData;

use futures::{Future, IntoFuture};

/// An RPC service which can take requests and asynchronously produce responses.
pub trait Service: Send + Sync + 'static {
    /// Requests for the service.
    type Req: Send + 'static;

    /// Responses for the service.
    type Resp: Send + 'static;

    /// Errors produced during normal (non-panicking) execution of the service.
    type Error: Send + 'static;

    /// A single instance of the service, which may contain per-instance state.
    type Instance: Send + 'static;

    /// The concrete future representing an asynchronous computation of the
    /// reponse to a request.
    type Fut: Future<Item = Self::Resp, Error = Self::Error>;

    /// Create a new instance of the service.
    // TODO: provide connection information
    fn new_instance(&self) -> Self::Instance;

    /// Process a single request on the given instance.
    fn process_req(&self, inst: &mut Self::Instance, req: Self::Req) -> Self::Fut;
}

/// A service wrapping a single request to response closure.
// Req is required due to not being an associated type for F
pub struct SimpleService<F, Req> {
    process: F,
    _ty: PhantomData<Mutex<Req>>, // use Mutex to avoid imposing Sync on Req
}

impl<F, Req, Resp, Error, Fut> Service for SimpleService<F, Req>
    where F: Fn(Req) -> Fut + Send + Sync + 'static,
          Fut: IntoFuture<Item = Resp, Error = Error>,
          Req: Send + 'static,
          Resp: Send + 'static,
          Error: Send + 'static
{
    type Req = Req;
    type Resp = Resp;
    type Error = Error;
    type Instance = ();
    type Fut = Fut::Future;

    fn new_instance(&self) -> () {
        ()
    }
    fn process_req(&self, _: &mut (), req: Req) -> Fut::Future {
        (self.process)(req).into_future()
    }
}

/// Create a service directly from a request to response closure.
pub fn simple_service<F, Req, Resp, Error, Fut>(f: F) -> SimpleService<F, Req>
    where F: Fn(Req) -> Fut + Send + Sync + 'static,
          Fut: IntoFuture<Item = Resp, Error = Error>,
          Req: Send + 'static,
          Resp: Send + 'static,
          Error: Send + 'static
{
    SimpleService {
        process: f,
        _ty: PhantomData,
    }
}
