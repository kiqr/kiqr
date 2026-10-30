// The platform this binary is built for, so the updater picks the matching release archive.
fn main() {
    println!("cargo:rustc-env=KIQR_TARGET={}", std::env::var("TARGET").unwrap());
}
