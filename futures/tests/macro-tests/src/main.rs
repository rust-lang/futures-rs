// Check that it works even if proc-macros are reexported.

fn main() {
    use futures03::executor::block_on;

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

    // TODO: add select! and select_biased!

}
