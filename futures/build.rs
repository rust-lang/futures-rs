fn main() {
    println!("cargo:rustc-check-cfg=cfg(futures_sanitizer)");
}
