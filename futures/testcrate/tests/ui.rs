fn run_mode(mode: &'static str) {
    use std::env;
    use std::path::PathBuf;

    let mut config = compiletest::Config::default();
    config.mode = mode.parse().expect("invalid mode");
    let mut me = env::current_exe().unwrap();
    me.pop();
    config.target_rustcflags = Some(format!(
        "--edition=2018 \
         -Z unstable-options \
         --extern futures \
         -L {}",
        me.display()
    ));
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    config.src_base = src.join(mode);

    me.pop();
    me.pop();
    config.build_base = me.join("tests").join(mode);
    compiletest::run_tests(&config);
}

fn main() {
    run_mode("ui");
}
