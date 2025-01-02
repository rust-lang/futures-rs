use std::{env, process::Command};

fn main() {
    println!("cargo:rustc-check-cfg=cfg(nightly)");
    if is_nightly() {
        println!("cargo:rustc-cfg=nightly");
    }
}

fn is_nightly() -> bool {
    env::var_os("RUSTC")
        .and_then(|rustc| Command::new(rustc).arg("--version").output().ok())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|version| version.contains("nightly") || version.contains("dev"))
}
