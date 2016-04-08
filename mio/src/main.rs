extern crate futuremio;
extern crate futures;

// use futures::Future;

fn main() {
    // let l = futuremio::Loop::new().unwrap();

    //
    // let l1 = l.tcp_listen(&"127.0.0.1:0".parse().unwrap()).unwrap();
    // let l2 = l.tcp_listen(&"127.0.0.1:0".parse().unwrap()).unwrap();
    // let a1 = l1.local_addr().unwrap();
    // let a2 = l2.local_addr().unwrap();
    // let a = std::thread::spawn(move || {
    //     let ((mut c1, a1, l1), (c2, a2, l2)) = l1.join(l2).await().unwrap();
    //     println!("c1: {:?}", a1);
    //     println!("c2: {:?}", a2);
    //     let ((c3, a3, l1), (c4, a4, l2)) = l1.join(l2).await().unwrap();
    //     println!("c3: {:?}", a3);
    //     println!("c4: {:?}", a4);
    //     drop((c2, c3, c4, l1, l2));
    //
    //     let mut buf = Vec::with_capacity(100);
    //     loop {
    //         let read = c1.read(buf).unwrap();
    //         let echo = read.and_then(|(buf, amt, c1)| {
    //             println!("srv - read: {:?}", &buf[..amt]);
    //             c1.write(buf).unwrap()
    //         });
    //         let (c, b) = echo.map(|(buf, amt, c1)| {
    //             println!("srv - write: {:?}", amt);
    //             (c1, buf)
    //         }).await().unwrap();
    //         c1 = c;
    //         buf = b;
    //     }
    // });
    //
    // let s1 = l.tcp_connect(&a1).unwrap();
    // let s2 = l.tcp_connect(&a2).unwrap();
    // let s3 = l.tcp_connect(&a1).unwrap();
    // let s4 = l.tcp_connect(&a2).unwrap();
    // let (((c1, c2), c3), c4) = s1.join(s2).join(s3).join(s4).await().unwrap();
    //
    // println!("c1-2: {:?}", c1.local_addr().unwrap());
    // println!("c2-2: {:?}", c2.local_addr().unwrap());
    // println!("c3-2: {:?}", c3.local_addr().unwrap());
    // println!("c4-2: {:?}", c4.local_addr().unwrap());
    //
    // drop((c2, c3, c4));
    //
    // let buf = vec![1; 20];
    // let write = c1.write(buf).unwrap().map(|(buf, amt, c1)| {
    //     assert_eq!(amt, 20);
    //     (buf, c1)
    // });
    // let read = write.and_then(|(buf, c1)| {
    //     c1.read(buf).unwrap()
    // }).map(|(buf, amt, c1)| {
    //     println!("read: {:?}", &buf[..amt]);
    //     drop((buf, c1));
    // });
    //
    // read.await().unwrap();
    //
    // a.join().unwrap();
}
