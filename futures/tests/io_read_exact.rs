use futures::executor::block_on;
use futures::io::AsyncReadExt;

#[test]
fn read_exact() {
    let mut reader: &[u8] = &[1, 2, 3, 4, 5];
    let mut out = [0u8; 3];

    let res = block_on(reader.read_exact(&mut out)); // read 3 bytes out
    res.unwrap();
    assert_eq!(out, [1, 2, 3]);
    assert_eq!(reader.len(), 2);

    let res = block_on(reader.read_exact(&mut out)); // read another 3 bytes, but only 2 bytes left
    res.unwrap_err();
    assert_eq!(reader.len(), 0);
}
