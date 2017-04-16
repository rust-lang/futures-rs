use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;

const MAX_SIZE_ENV : Option<&'static str> = option_env!("FUTURES_MAX_UNPARK_BYTES");

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let max_size = match MAX_SIZE_ENV.and_then(|s| usize::from_str(&s).ok()) {
        Some(x) if x > 0 => x,
        _ => 64 // Default value.
    };
    let dest_path = Path::new(&out_dir).join("max_unpark_bytes.txt");
    let mut f = File::create(&dest_path).unwrap();
    f.write_all(max_size.to_string().as_bytes()).unwrap();
}
