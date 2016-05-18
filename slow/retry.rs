extern crate futures;

use futures::*;

type MyFuture<T> = Future<Item=T,Error=()>;

fn fetch_item() -> Box<MyFuture<Option<Foo>>> {
    return fetch_item2(fetch_replica_a(), fetch_replica_b());
}

fn fetch_item2(a: Box<MyFuture<Foo>>, b: Box<MyFuture<Foo>>) -> Box<MyFuture<Option<Foo>>> {
    let a = a.then(|a| disambiguate(a, 0));
    let b = b.then(|b| disambiguate(b, 1));
    a.select(b).map(Some).select(timeout()).then(|res| {
        let (to_restart, other) = match res {
            // Something finished successfully, see if it's a valid response
            Ok((Some(((val, which), next)), _next_timeout)) => {
                if val.is_valid() {
                    return finished(Some(val)).boxed()
                }
                (which, next)
            }
            // timeout, we're done
            Ok((None, _)) => return finished(None).boxed(),

            Err(((((), which), next), _next_timeout)) => (which, next),
        };
        let other = other.then(reambiguate);
        if to_restart == 0 {
            fetch_item2(fetch_replica_a(), other.boxed())
        } else {
            fetch_item2(other.boxed(), fetch_replica_b())
        }
    }).boxed()
}

fn disambiguate<T, E, U>(r: Result<T, E>, u: U) -> Result<(T, U), (E, U)> {
    match r {
        Ok(t) => Ok((t, u)),
        Err(e) => Err((e, u)),
    }
}

fn reambiguate<T, E, U>(r: Result<(T, U), (E, U)>) -> Result<T, E> {
    match r {
        Ok((t, _u)) => Ok(t),
        Err((e, _u)) => Err(e),
    }
}

struct Foo;

impl Foo {
    fn is_valid(&self) -> bool { false }
}

fn fetch_replica_a() -> Box<MyFuture<Foo>> { loop {} }
fn fetch_replica_b() -> Box<MyFuture<Foo>> { loop {} }
fn timeout<T, E>() -> Box<Future<Item=Option<T>,Error=E>> { loop {} }

fn main() {
    fetch_item();
}
