// Check that it works even if proc-macros are reexported.

fn main() {
    use futures04::{executor::block_on, future};

    // join! macro
    block_on(async {
        let _ = futures04::join!(async {}, async {});
        let _ = macro_reexport::join!(async {}, async {});
        let _ = macro_reexport::join2!(async {}, async {});
    });

    // try_join! macro
    let _ = block_on(async {
        let _ = futures04::try_join!(async { Ok::<(), ()>(()) }, async { Ok::<(), ()>(()) });
        let _ = macro_reexport::try_join!(async { Ok::<(), ()>(()) }, async { Ok::<(), ()>(()) });
        let _ = macro_reexport::try_join2!(async { Ok::<(), ()>(()) }, async { Ok::<(), ()>(()) });
        Ok::<(), ()>(())
    });

    // select! macro
    block_on(async {
        let mut a = future::ready(());
        let mut b = future::pending::<()>();
        futures04::select! {
            _ = a => {},
            _ = b => unreachable!(),
        };

        let mut a = future::ready(());
        let mut b = future::pending::<()>();
        macro_reexport::select! {
            _ = a => {},
            _ = b => unreachable!(),
        };

        let mut a = future::ready(());
        let mut b = future::pending::<()>();
        macro_reexport::select2! {
            _ = a => {},
            _ = b => unreachable!(),
        };
    });

    // select_biased! macro
    block_on(async {
        let mut a = future::ready(());
        let mut b = future::pending::<()>();
        futures04::select_biased! {
            _ = a => {},
            _ = b => unreachable!(),
        };

        let mut a = future::ready(());
        let mut b = future::pending::<()>();
        macro_reexport::select_biased! {
            _ = a => {},
            _ = b => unreachable!(),
        };

        let mut a = future::ready(());
        let mut b = future::pending::<()>();
        macro_reexport::select_biased2! {
            _ = a => {},
            _ = b => unreachable!(),
        };
    });
}
