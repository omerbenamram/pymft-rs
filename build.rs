//! Fails the build when PyO3 resolved a different Python ABI than CI asked for.
//!
//! Cross builds (zig targets, the x86_64 half of universal2) cannot run the target interpreter,
//! so PyO3 takes the version and free-threading flag from `PYO3_CROSS_PYTHON_VERSION`. A wheel
//! tag only reflects the host interpreter, so a misread flag would otherwise ship a GIL-ABI
//! binary in a `cp314t` wheel. CI sets `MFT_EXPECT_PYTHON` (e.g. `3.10` or `3.14t`) per leg.

fn main() {
    println!("cargo:rerun-if-env-changed=MFT_EXPECT_PYTHON");
    let Ok(expected) = std::env::var("MFT_EXPECT_PYTHON") else {
        return;
    };
    let expect_free_threaded = expected.ends_with('t');
    let config = pyo3_build_config::get();
    let target = std::env::var("TARGET").unwrap_or_default();
    assert_eq!(
        config.is_free_threaded(),
        expect_free_threaded,
        "MFT_EXPECT_PYTHON={expected}, but PyO3 configured {} {:?} for {target}",
        if config.is_free_threaded() {
            "a free-threaded"
        } else {
            "a GIL-enabled"
        },
        config.version(),
    );
}
