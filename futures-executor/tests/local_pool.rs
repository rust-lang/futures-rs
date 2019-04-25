use futures::channel::oneshot;
use futures::executor::LocalPool;
use futures::future::{Future, lazy, poll_fn};
use futures::task::{Context, Poll, Spawn, LocalSpawn, Waker};
use std::cell::{Cell, RefCell};
use std::pin::Pin;
use std::rc::Rc;

struct Pending(Rc<()>);

impl Future for Pending {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        Poll::Pending
    }
}

fn pending() -> Pending {
    Pending(Rc::new(()))
}

#[test]
fn run_until_single_future() {
    let mut cnt = 0;

    {
        let mut pool = LocalPool::new();
        let fut = lazy(|_| {
            cnt += 1;
            ()
        });
        assert_eq!(pool.run_until(fut), ());
    }

    assert_eq!(cnt, 1);
}

#[test]
fn run_until_ignores_spawned() {
    let mut pool = LocalPool::new();
    let mut spawn = pool.spawner();
    spawn.spawn_local_obj(Box::pin(pending()).into()).unwrap();
    assert_eq!(pool.run_until(lazy(|_| ())), ());
}

#[test]
fn run_until_executes_spawned() {
    let (tx, rx) = oneshot::channel();
    let mut pool = LocalPool::new();
    let mut spawn = pool.spawner();
    spawn.spawn_local_obj(Box::pin(lazy(move |_| {
        tx.send(()).unwrap();
        ()
    })).into()).unwrap();
    pool.run_until(rx).unwrap();
}

#[test]
fn run_returns_if_empty() {
    let mut pool = LocalPool::new();
    pool.run();
    pool.run();
}

#[test]
fn run_executes_spawned() {
    let cnt = Rc::new(Cell::new(0));
    let cnt2 = cnt.clone();

    let mut pool = LocalPool::new();
    let mut spawn = pool.spawner();
    let mut spawn2 = pool.spawner();

    spawn.spawn_local_obj(Box::pin(lazy(move |_| {
        spawn2.spawn_local_obj(Box::pin(lazy(move |_| {
            cnt2.set(cnt2.get() + 1);
            ()
        })).into()).unwrap();
        ()
    })).into()).unwrap();

    pool.run();

    assert_eq!(cnt.get(), 1);
}


#[test]
fn run_spawn_many() {
    const ITER: usize = 200;

    let cnt = Rc::new(Cell::new(0));

    let mut pool = LocalPool::new();
    let mut spawn = pool.spawner();

    for _ in 0..ITER {
        let cnt = cnt.clone();
        spawn.spawn_local_obj(Box::pin(lazy(move |_| {
            cnt.set(cnt.get() + 1);
            ()
        })).into()).unwrap();
    }

    pool.run();

    assert_eq!(cnt.get(), ITER);
}

#[test]
fn try_run_one_returns_if_empty() {
    let mut pool = LocalPool::new();
    assert!(pool.try_run_one() == false);
}

#[test]
fn try_run_one_executes_one_ready() {
    const ITER: usize = 200;

    let cnt = Rc::new(Cell::new(0));

    let mut pool = LocalPool::new();
    let mut spawn = pool.spawner();

    for _ in 0..ITER {
        spawn.spawn_local_obj(Box::pin(pending()).into()).unwrap();

        let cnt = cnt.clone();
        spawn.spawn_local_obj(Box::pin(lazy(move |_| {
            cnt.set(cnt.get() + 1);
            ()
        })).into()).unwrap();

        spawn.spawn_local_obj(Box::pin(pending()).into()).unwrap();
    }

    for i in 0..ITER {
        assert_eq!(cnt.get(), i);
        assert!(pool.try_run_one());
        assert_eq!(cnt.get(), i + 1);
    }
    assert!(pool.try_run_one() == false);
}

#[test]
fn try_run_one_returns_on_no_progress() {
    const ITER: usize = 10;

    let cnt = Rc::new(Cell::new(0));

    let mut pool = LocalPool::new();
    let mut spawn = pool.spawner();

    let waker: Rc<Cell<Option<Waker>>> = Rc::new(Cell::new(None));
    {
        let cnt = cnt.clone();
        let waker = waker.clone();
        spawn.spawn_local_obj(Box::pin(poll_fn(move |ctx| {
            cnt.set(cnt.get() + 1);
            waker.set(Some(ctx.waker().clone()));
            if cnt.get() == ITER {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })).into()).unwrap();
    }

    for i in 0..ITER - 1 {
        assert_eq!(cnt.get(), i);
        assert!(!pool.try_run_one());
        assert_eq!(cnt.get(), i + 1);
        let w = waker.take();
        assert!(w.is_some());
        w.unwrap().wake();
    }
    assert!(pool.try_run_one());
    assert_eq!(cnt.get(), ITER);
}

#[test]
fn run_until_stalled_returns_if_empty() {
    let mut pool = LocalPool::new();
    pool.run_until_stalled();
    pool.run_until_stalled();
}

#[test]
fn run_until_stalled_returns_multiple_times() {
    let mut pool = LocalPool::new();
    let mut spawn = pool.spawner();
    let cnt = Rc::new(Cell::new(0));

    let cnt1 = cnt.clone();
    spawn.spawn_local_obj(Box::pin(lazy(move |_|{ cnt1.set(cnt1.get() + 1) })).into()).unwrap();
    pool.run_until_stalled();
    assert_eq!(cnt.get(), 1);

    let cnt2 = cnt.clone();
    spawn.spawn_local_obj(Box::pin(lazy(move |_|{ cnt2.set(cnt2.get() + 1) })).into()).unwrap();
    pool.run_until_stalled();
    assert_eq!(cnt.get(), 2);
}

#[test]
fn run_until_stalled_executes_all_ready() {
    const ITER: usize = 200;
    const PER_ITER: usize = 3;

    let cnt = Rc::new(Cell::new(0));

    let mut pool = LocalPool::new();
    let mut spawn = pool.spawner();

    for i in 0..ITER {
        for _ in 0..PER_ITER {
            spawn.spawn_local_obj(Box::pin(pending()).into()).unwrap();

            let cnt = cnt.clone();
            spawn.spawn_local_obj(Box::pin(lazy(move |_| {
                cnt.set(cnt.get() + 1);
                ()
            })).into()).unwrap();

            // also add some pending tasks to test if they are ignored
            spawn.spawn_local_obj(Box::pin(pending()).into()).unwrap();
        }
        assert_eq!(cnt.get(), i * PER_ITER);
        pool.run_until_stalled();
        assert_eq!(cnt.get(), (i + 1) * PER_ITER);
    }
}

#[test]
#[should_panic]
fn nesting_run() {
    let mut pool = LocalPool::new();
    let mut spawn = pool.spawner();

    spawn.spawn_obj(Box::pin(lazy(|_| {
        let mut pool = LocalPool::new();
        pool.run();
    })).into()).unwrap();

    pool.run();
}

#[test]
#[should_panic]
fn nesting_run_run_until_stalled() {
    let mut pool = LocalPool::new();
    let mut spawn = pool.spawner();

    spawn.spawn_obj(Box::pin(lazy(|_| {
        let mut pool = LocalPool::new();
        pool.run_until_stalled();
    })).into()).unwrap();

    pool.run();
}

#[test]
fn tasks_are_scheduled_fairly() {
    let state = Rc::new(RefCell::new([0, 0]));

    struct Spin {
        state: Rc<RefCell<[i32; 2]>>,
        idx: usize,
    }

    impl Future for Spin {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            let mut state = self.state.borrow_mut();

            if self.idx == 0 {
                let diff = state[0] - state[1];

                assert!(diff.abs() <= 1);

                if state[0] >= 50 {
                    return Poll::Ready(());
                }
            }

            state[self.idx] += 1;

            if state[self.idx] >= 100 {
                return Poll::Ready(());
            }

            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }

    let mut pool = LocalPool::new();
    let mut spawn = pool.spawner();

    spawn.spawn_local_obj(Box::pin(Spin {
        state: state.clone(),
        idx: 0,
    }).into()).unwrap();

    spawn.spawn_local_obj(Box::pin(Spin {
        state: state,
        idx: 1,
    }).into()).unwrap();

    pool.run();
}
