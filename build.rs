extern crate regex;

use std::env;
use std::str;
use std::process;

#[allow(dead_code)]
struct RustVersion {
    major: u32,
    minor: u32,
    patch: u32,
    nightly: bool,
}

fn rust_version() -> RustVersion {
    let rustc = env::var("RUSTC").expect("RUSTC variable is unset");

    let command = process::Command::new(rustc)
        .args(&["--version"])
        .stdin(process::Stdio::null())
        .stderr(process::Stdio::inherit())
        .stdout(process::Stdio::piped())
        .spawn()
        .expect("spawn rustc");

    let wait = command.wait_with_output().expect("wait for rust");
    if !wait.status.success() {
        panic!("rustc --version exited with non-zero code");
    }

    let stdout = str::from_utf8(&wait.stdout).expect("stdout is not UTF-8");

    let re = regex::Regex::new(r"^rustc (\d+)\.(\d+)\.(\d+)(-nightly)?").expect("compile regex");
    let captures = re.captures(stdout)
        .expect(&format!("regex cannot match `rustc --version` output: {:?}", stdout));

    let major: u32 = captures.get(1).expect("major").as_str().parse().unwrap();
    let minor: u32 = captures.get(2).expect("minor").as_str().parse().unwrap();
    let patch: u32 = captures.get(3).expect("patch").as_str().parse().unwrap();
    let nightly: bool = captures.get(4).is_some();

    RustVersion {
        major: major,
        minor: minor,
        patch: patch,
        nightly: nightly,
    }
}

fn main() {
    let version = rust_version();

    if version.nightly {
        println!("cargo:rustc-cfg=rust_nightly");
    }
}
