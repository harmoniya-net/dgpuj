//! Force the GPU-selection data symbols into the executable's PE export table.
//!
//! The Optimus / PowerXpress driver looks up `NvOptimusEnablement` /
//! `AmdPowerXpressRequestHighPerformance` in the export table of the
//! *executable that creates the GL/D3D context*. `#[no_mangle] #[used]` keeps
//! the symbol in the object file; the linker only places it in the export
//! directory when told to — hence these `/EXPORT:` args (MSVC linker syntax;
//! the Windows targets we build are `*-pc-windows-msvc`).

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        println!("cargo:rustc-link-arg-bins=/EXPORT:NvOptimusEnablement");
        println!("cargo:rustc-link-arg-bins=/EXPORT:AmdPowerXpressRequestHighPerformance");
    }
}
