use futures::executor::block_on;
use futures::future::{ScopedBoxFuture, ScopedFutureExt};

struct Db {
    count: u8,
}

impl Db {
    async fn transaction<'a, T: 'a, E: 'a, F: 'a>(&mut self, callback: F) -> Result<T, E>
    where
        F: for<'b> FnOnce(&'b mut Self) -> ScopedBoxFuture<'a, 'b, Result<T, E>> + Send,
    {
        callback(self).await
    }
}

async fn test_transaction<'a, 'b>(
    db: &mut Db,
    ok: &'a str,
    err: &'b str,
    is_ok: bool,
) -> Result<&'a str, &'b str> {
    db.transaction(|db| async move {
        db.count += 1;
        if is_ok {
            Ok(ok)
        } else {
            Err(err)
        }
    }.scope_boxed()).await?;

    // note that `async` is used instead of `async move`
    // since the callback parameter is unused
    db.transaction(|_| async {
        if is_ok {
            Ok(ok)
        } else {
            Err(err)
        }
    }.scope_boxed()).await
}

#[test]
fn test_transaction_works() {
    block_on(async {
        let mut db = Db { count: 0 };
        let ok = String::from("ok");
        let err = String::from("err");
        let result = test_transaction(&mut db, &ok, &err, true).await;
        assert_eq!(Ok(&*ok), result);
        assert_eq!(1, db.count);
    })
}
