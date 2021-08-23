use futures::executor::block_on;

use futures::io::{AsyncWriteExt, LineWriter};

#[test]
fn line_writer() {
    let mut writer = LineWriter::new(Vec::new());

    block_on(writer.write(&[0])).unwrap();
    assert_eq!(*writer.get_ref(), []);

    block_on(writer.write(&[1])).unwrap();
    assert_eq!(*writer.get_ref(), []);

    block_on(writer.flush()).unwrap();
    assert_eq!(*writer.get_ref(), [0, 1]);

    block_on(writer.write(&[0, b'\n', 1, b'\n', 2])).unwrap();
    assert_eq!(*writer.get_ref(), [0, 1, 0, b'\n', 1, b'\n']);

    block_on(writer.flush()).unwrap();
    assert_eq!(*writer.get_ref(), [0, 1, 0, b'\n', 1, b'\n', 2]);

    block_on(writer.write(&[3, b'\n'])).unwrap();
    assert_eq!(*writer.get_ref(), [0, 1, 0, b'\n', 1, b'\n', 2, 3, b'\n']);
}
