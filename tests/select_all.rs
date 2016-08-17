extern crate futures;

use futures::*;
use futures::task::Task;

#[test]
fn smoke() {
    let v = vec![
        finished(1).boxed(),
        failed(2).boxed(),
        finished(3).boxed(),
    ];
    Task::new().enter(|| {
        let (i, idx, v) = select_all(v).poll().unwrap().ok().unwrap();
        assert_eq!(i, 1);
        assert_eq!(idx, 0);

        let (i, idx, v) = select_all(v).poll().unwrap().err().unwrap();
        assert_eq!(i, 2);
        assert_eq!(idx, 0);

        let (i, idx, v) = select_all(v).poll().unwrap().ok().unwrap();
        assert_eq!(i, 3);
        assert_eq!(idx, 0);

        assert!(v.len() == 0);
    })
}
