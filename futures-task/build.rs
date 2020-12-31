use autocfg::AutoCfg;

// The rustc-cfg strings below are *not* public API. Please let us know by
// opening a GitHub issue if your build environment requires some way to enable
// these cfgs other than by executing our build script.
fn main() {
    let cfg = match AutoCfg::new() {
        Ok(cfg) => cfg,
        Err(e) => {
            println!(
                "cargo:warning={}: unable to determine rustc version: {}",
                env!("CARGO_PKG_NAME"),
                e
            );
            return;
        }
    };

    // Note that this is `no_atomic_cas`, not `has_atomic_cas`. This allows
    // treating `cfg(target_has_atomic = "ptr")` as true when the build script
    // doesn't run. This is needed for compatibility with non-cargo build
    // systems that don't run build scripts.
    if !cfg.probe_expression("core::sync::atomic::AtomicPtr::<()>::compare_exchange") {
        println!("cargo:rustc-cfg=no_atomic_cas");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
