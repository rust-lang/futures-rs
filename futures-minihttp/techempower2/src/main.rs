extern crate env_logger;
extern crate futures;
extern crate futures_cpupool;
extern crate futures_minihttp;
extern crate postgres;
extern crate r2d2;
extern crate r2d2_postgres;
extern crate rand;
extern crate rustc_serialize;

use std::env;
use std::io;
use std::net::SocketAddr;

use futures::*;
use futures_minihttp::{Request, Response, Server};
use futures_cpupool::CpuPool;
use r2d2_postgres::{SslMode, PostgresConnectionManager};
use rand::Rng;

#[derive(RustcEncodable)]
#[allow(bad_style)]
struct Row {
    id: i32,
    randomNumber: i32,
}

fn main() {
    env_logger::init().unwrap();
    let addr = env::args().nth(1).unwrap_or("127.0.0.1:8080".to_string());
    let addr = addr.parse::<SocketAddr>().unwrap();

    let config = r2d2::Config::builder()
                              .pool_size(80)
                              .build();
    let url = "postgres://benchmarkdbuser:benchmarkdbpass@\
                          localhost:5432/\
                          hello_world";
    let manager = PostgresConnectionManager::new(url, SslMode::None).unwrap();

    let cpupool = CpuPool::new(10);
    let r2d2pool = r2d2::Pool::new(config, manager).unwrap();

    Server::new(&addr).workers(8).serve(move |r| {
        json(r, &r2d2pool, &cpupool)
    }).unwrap()
}

trait Thunk: Send + 'static {
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce() + Send + 'static> Thunk for F {
    fn call_box(self: Box<Self>) {
        (*self)()
    }
}

fn json(r: Request,
        r2d2: &r2d2::Pool<PostgresConnectionManager>,
        pool: &CpuPool)
        -> Box<Future<Item=Response, Error=io::Error>> {
    assert_eq!(r.path(), "/db");
    let id = rand::thread_rng().gen_range(0, 10_000) + 1;
    let r2d2 = r2d2.clone();

    pool.execute(move || {
        let conn = r2d2.get().unwrap();
        let query = "SELECT id, randomNumber FROM World WHERE id = $1";
        let stmt = conn.prepare_cached(query).unwrap();
        let rows = stmt.query(&[&id]).unwrap();
        let row = rows.get(0);
        Row {
            id: row.get(0),
            randomNumber: row.get(1),
        }
    }).then(|res| {
        match res {
            Ok(row) => {
                let mut r = Response::new();
                r.header("Content-Type", "application/json")
                 .body(&rustc_serialize::json::encode(&row).unwrap());
                Ok(r)
            }
            Err(_err) => Err(io::Error::new(io::ErrorKind::Other,
                                            "thread panicked")),
        }
    }).boxed()
}
