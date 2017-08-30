extern crate compiletest_rs as compiletest;

use std::env;

fn run_mode(mode: &'static str) {
    let mut config = compiletest::Config::default();
    config.mode = mode.parse().expect("invalid mode");
    let mut me = env::current_exe().unwrap();
    me.pop();
    config.target_rustcflags = Some(format!("-L {}", me.display()));
    config.src_base = format!("tests/{}", mode).into();
    compiletest::run_tests(&config);
}

fn main() {
    run_mode("compile-fail");
}
