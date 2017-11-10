extern crate futures;

use futures::prelude::*;
use futures::future::{blocking, ok, err};

#[test]
fn smoke() {
    let mut counter = 0;

    {
        let work = ok::<u32, u32>(40).inspect(|val| { counter += *val; });
        assert_eq!(blocking(work).wait(), Ok(40));
    }

    assert_eq!(counter, 40);

    {
        let work = err::<u32, u32>(4).inspect(|val| { counter += *val; });
        assert_eq!(blocking(work).wait(), Err(4));
    }

    assert_eq!(counter, 40);
}
