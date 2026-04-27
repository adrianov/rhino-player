//! Prefer a libmpv in `/usr/local` (e.g. VapourSynth build) without `LD_LIBRARY_PATH`.
//! The final binary gets `DT_RUNPATH` for `/usr/local/lib/<multiarch>` and `/usr/local/lib`.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").ok().as_deref() != Some("linux") {
        return;
    }
    // Prints once per `cargo` invocation; not per crate.
    println!("cargo:rerun-if-changed=build.rs");
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").ok().and_then(|a| {
        let triplet = match a.as_str() {
            "x86_64" => Some("x86_64-linux-gnu"),
            "aarch64" => Some("aarch64-linux-gnu"),
            "arm" => Some("arm-linux-gnueabihf"),
            _ => None,
        };
        triplet.map(|t| format!("/usr/local/lib/{}", t))
    });
    for dir in [arch.as_deref(), Some("/usr/local/lib")]
        .into_iter()
        .flatten()
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", dir);
    }
    println!("cargo:rustc-link-arg=-Wl,--enable-new-dtags");
}
