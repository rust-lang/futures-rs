// Check that it works even if proc-macros are reexported.

fn main() {
    use futures03::{executor::block_on, future};

    // join! macro
    let _ = block_on(async {
        let _ = futures03::join!(async {}, async {});
        let _ = macro_reexport::join!(async {}, async {});
        let _ = macro_reexport::join2!(async {}, async {});
    });

    // try_join! macro
    let _ = block_on(async {
        let _ = futures03::try_join!(async { Ok::<(), ()>(()) }, async { Ok::<(), ()>(()) });
        let _ = macro_reexport::try_join!(async { Ok::<(), ()>(()) }, async { Ok::<(), ()>(()) });
        let _ = macro_reexport::try_join2!(async { Ok::<(), ()>(()) }, async { Ok::<(), ()>(()) });
        Ok::<(), ()>(())
    });

    // select! macro
    let _ = block_on(async {
        let mut a = future::ready(());
        let mut b = future::pending::<()>();
        let _ = futures03::select! {
            _ = a => {},
            _ = b => unreachable!(),
        };

        let mut a = future::ready(());
        let mut b = future::pending::<()>();
        let _ = macro_reexport::select! {
            _ = a => {},
            _ = b => unreachable!(),
        };

        let mut a = future::ready(());
        let mut b = future::pending::<()>();
        let _ = macro_reexport::select2! {
            _ = a => {},
            _ = b => unreachable!(),
        };
    });

    // select_biased! macro
    let _ = block_on(async {
        let mut a = future::ready(());
        let mut b = future::pending::<()>();
        let _ = futures03::select_biased! {
            _ = a => {},
            _ = b => unreachable!(),
        };

        let mut a = future::ready(());
        let mut b = future::pending::<()>();
        let _ = macro_reexport::select_biased! {
            _ = a => {},
            _ = b => unreachable!(),
        };

        let mut a = future::ready(());
        let mut b = future::pending::<()>();
        let _ = macro_reexport::select_biased2! {
            _ = a => {},
            _ = b => unreachable!(),
        };
    });
}
