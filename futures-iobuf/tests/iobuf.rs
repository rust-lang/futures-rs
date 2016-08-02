extern crate futures_iobuf;

use futures_iobuf::IoBuf;

#[test]
fn simple() {
    let mut buf = IoBuf::new();
    assert_eq!(buf.as_slice(), &[]);
    assert_eq!(buf.len(), 0);

    buf.get_mut().extend(b"12345");
    assert_eq!(buf.len(), 5);
    assert_eq!(buf.as_slice(), b"12345");
    let mut other = buf.split_off(1);
    assert_eq!(buf.as_slice(), b"1");
    assert_eq!(other.as_slice(), b"2345");
    let mut other2 = other.drain_to(2);
    assert_eq!(buf.as_slice(), b"1");
    assert_eq!(other2.as_slice(), b"23");
    assert_eq!(other.as_slice(), b"45");

    buf.get_mut().extend(b"567");
    assert_eq!(buf.as_slice(), b"1567");
    assert_eq!(other2.as_slice(), b"23");
    assert_eq!(other.as_slice(), b"45");

    other2.get_mut().extend(b"890");
    assert_eq!(buf.as_slice(), b"1567");
    assert_eq!(other2.as_slice(), b"23890");
    assert_eq!(other.as_slice(), b"45");

    other.get_mut().extend(b"abc");
    assert_eq!(buf.as_slice(), b"1567");
    assert_eq!(other2.as_slice(), b"23890");
    assert_eq!(other.as_slice(), b"45abc");
}
