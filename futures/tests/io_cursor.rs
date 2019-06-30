use assert_matches::assert_matches;
use futures::Poll;
use futures::future::lazy;
use futures::io::AsyncWrite;
use std::io::Cursor;
use std::pin::Pin;

#[test]
fn cursor_asyncwrite_asmut() {
    let mut cursor = Cursor::new([0; 5]);
    futures::executor::block_on(lazy(|cx| {
        assert_matches!(Pin::new(&mut cursor).poll_write(cx, &[1, 2]), Poll::Ready(Ok(2)));
        assert_matches!(Pin::new(&mut cursor).poll_write(cx, &[3, 4]), Poll::Ready(Ok(2)));
        assert_matches!(Pin::new(&mut cursor).poll_write(cx, &[5, 6]), Poll::Ready(Ok(1)));
        assert_matches!(Pin::new(&mut cursor).poll_write(cx, &[6, 7]), Poll::Ready(Ok(0)));
    }));
    assert_eq!(cursor.into_inner(), [1, 2, 3, 4, 5]);
}
