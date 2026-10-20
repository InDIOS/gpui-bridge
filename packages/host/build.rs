// Rust keeps the platform default for the stack of a binary's main thread: 1 MB on Windows, 8 MB
// elsewhere. The tree walk in `render` recurses once per level of nesting, and in a debug build each
// level costs tens of KB, so a deep tree overflowed the 1 MB of Windows (STATUS_STACK_OVERFLOW,
// exit code 3221225725) before drawing its first frame. Ask for the same 8 MB on Windows.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
            println!("cargo:rustc-link-arg-bins=/STACK:8388608");
        } else {
            println!("cargo:rustc-link-arg-bins=-Wl,--stack,8388608");
        }
    }
}
