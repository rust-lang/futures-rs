export PATH=$HOME/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/bin:$PATH
export PATH=$HOME/code/rust2/build/x86_64-unknown-linux-gnu/stage2/bin:$PATH

# which rustc
# rustc -vV
# which cargo
# cargo -vV
cargo "$@"
