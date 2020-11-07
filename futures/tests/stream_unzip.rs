use futures::stream::{self, StreamExt};
use futures::executor::{block_on_stream};

fn fail_on_thread_panic() {
    std::panic::set_hook(Box::new(move |panic_info: &std::panic::PanicInfo| {
        println!("{}", panic_info.to_string());
        std::process::exit(1);
    }));
}

fn sample_stream(start: usize, end: usize) -> futures_util::stream::Iter<std::vec::IntoIter<(usize, usize)>> {
    let list_iter = (start..end)
        .filter(|&x| x % 2 == 1)
        .map(|x| (x, x + 1));
    
    return stream::iter(list_iter.collect::<Vec<_>>());
}

#[test]
fn left_dropped_before_first_poll() {
    let (_, mut s2) = {
        let (s1, s2) = sample_stream(1, 2).unzip();
        (block_on_stream(s1), block_on_stream(s2))
    };

    assert_eq!(s2.next(), Some(2));
    assert_eq!(s2.next(), None);
}

#[test]
fn left_dropped_after_polled() {
    fail_on_thread_panic();

    let (mut s1, mut s2) = {
        let (s1, s2) = sample_stream(1, 4).unzip();
        (block_on_stream(s1), block_on_stream(s2))
    };
    

    let t1 = std::thread::spawn(move || {
        assert_eq!(s1.next(), Some(1));
        drop(s1);
    });

    let t2 = std::thread::spawn(move || {
        assert_eq!(s2.next(), Some(2));
        assert_eq!(s2.next(), Some(4));
        assert_eq!(s2.next(), None);
    });

    let _ = t1.join();
    let _ = t2.join();
}

#[test]
fn right_dropped_before_first_poll() {
    let (mut s1, _) = {
        let (s1, s2) = sample_stream(1, 2).unzip();
        (block_on_stream(s1), block_on_stream(s2))
    };

    assert_eq!(s1.next(), Some(1));
    assert_eq!(s1.next(), None);
}

#[test]
fn right_dropped_after_polled() {
    let (mut s1, mut s2) = {
        let (s1, s2) = sample_stream(1, 4).unzip();
        (block_on_stream(s1), block_on_stream(s2))
    };

    assert_eq!(s1.next(), Some(1));
    assert_eq!(s2.next(), Some(2));
    drop(s2);
    assert_eq!(s1.next(), Some(3));
    assert_eq!(s1.next(), None);
}