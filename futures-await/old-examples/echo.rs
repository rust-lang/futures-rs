//! A "souped up" echo server example.
//!
//! Very similar to the example at https://tokio.rs

#![feature(proc_macro, conservative_impl_trait, generators)]

extern crate futures_await as futures;
extern crate tokio_core;
extern crate tokio_io;

use std::io::{self, BufReader};

use futures::prelude::*;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::Core;
use tokio_io::{AsyncRead};

fn main() {
    // Create the event loop that will drive this server
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    // Bind the server's socket
    let addr = "127.0.0.1:12345".parse().unwrap();
    let tcp = TcpListener::bind(&addr, &handle).expect("failed to bind listener");
    println!("listening for connections on {}",
             tcp.local_addr().unwrap());

    let server = async_block! {
        #[async]
        for (client, _) in tcp.incoming() {
            handle.spawn(handle_client(client).then(|result| {
                match result {
                    Ok(n) => println!("wrote {} bytes", n),
                    Err(e) => println!("IO error {:?}", e),
                }
                Ok(())
            }));
        }

        Ok::<(), io::Error>(())
    };
    core.run(server).unwrap();
}

#[async_move]
fn handle_client(socket: TcpStream) -> io::Result<u64> {
    let (reader, mut writer) = socket.split();
    let input = BufReader::new(reader);

    let mut total = 0;

    #[async]
    for line in tokio_io::io::lines(input) {
        println!("got client line: {}", line);
        total += line.len() as u64;
        writer = await!(tokio_io::io::write_all(writer, line))?.0;
    }

    Ok(total)
}
