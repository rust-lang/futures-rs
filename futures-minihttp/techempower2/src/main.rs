//! An implementation of the TechEmpower [Single query][bench] benchmark.
//!
//! This simple HTTP server generates a random number, looks up that row in a
//! database, and then renders the row as JSON.
//!
//! [bench]: https://www.techempower.com/benchmarks/#section=data-r12&hw=peak&test=db
//!
//! First you'll need to prepare a Postgres database by running the
//! [create-postgres-database.sql](https://github.com/TechEmpower/FrameworkBenchmarks/blob/master/config/create-postgres-database.sql)
//! followed by the
//! [create-postgres.sql](https://github.com/TechEmpower/FrameworkBenchmarks/blob/master/config/create-postgres.sql)
//! file.
//!
//! After, run the server
//!
//! ```
//! $ cargo run --release
//! ```
//!
//! Then, make a connection
//!
//! ```
//! $ curl https://localhost:8080/db
//! ```

extern crate env_logger;
extern crate futures;
extern crate futures_cpupool;
extern crate futures_minihttp;
extern crate rand;
extern crate rustc_serialize;

// Database management tools
extern crate postgres;
extern crate r2d2;
extern crate r2d2_postgres;

use std::env;
use std::io;
use std::net::SocketAddr;

use futures::*;
use futures_minihttp::{Request, Response, Server};
use futures_cpupool::{CpuPool, CpuFuture};
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

    // Connect to our database
    let config = r2d2::Config::builder()
                              .pool_size(80)
                              .build();
    let url = "postgres://benchmarkdbuser:benchmarkdbpass@\
                          localhost:5432/\
                          hello_world";
    let manager = PostgresConnectionManager::new(url, SslMode::None).unwrap();
    let r2d2pool = r2d2::Pool::new(config, manager).unwrap();

    // Create a worker thread pool to execute database queries on
    let cpupool = CpuPool::new(10);

    Server::new(&addr).workers(8).serve(move |r: Request| {
        assert_eq!(r.path(), "/db");
        let id = rand::thread_rng().gen_range(0, 10_000) + 1;
        get_row(&r2d2pool, id, &cpupool).then(|res| {
            // If we successfully got the row, render it to an HTTP response,
            // otherwise convert the error to an I/O error
            match res {
                Ok(row) => {
                    let mut r = Response::new();
                    r.header("Content-Type", "application/json")
                     .body(&rustc_serialize::json::encode(&row).unwrap());
                    Ok(r)
                }
                Err(_err) => Err(io::Error::new(io::ErrorKind::Other,
                                                "database panicked")),
            }
        })
    }).unwrap()
}

/// To fetch a row from the database, we execute the blocking database call on
/// the `CpuPool` provided.
fn get_row(r2d2: &r2d2::Pool<PostgresConnectionManager>,
           id: i32,
           pool: &CpuPool)
           -> CpuFuture<Row> {
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
    })
}
