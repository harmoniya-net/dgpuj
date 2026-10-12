//! macOS: there is no discrete-GPU lever reachable from a bare CLI binary.
//!
//! Apple Silicon has a single unified GPU, and the legacy Intel dual-GPU switch
//! was driven by an `.app` bundle's `Info.plist`
//! (`NSSupportsAutomaticGraphicsSwitching`), which a standalone executable can't
//! carry. So dgpuj is just an in-process JVM launcher here, one that keeps
//! `main` on the first thread for LWJGL/GLFW.

use std::ffi::CString;
use std::path::Path;

/// HotSpot ships `libjvm.dylib` under `<home>/lib/server/`.
pub const JVM_LIB_REL: &str = "lib/server/libjvm.dylib";

/// The VM stays on the thread dgpuj started on. AppKit takes events on the
/// process's first thread alone, so that is where `main` has to be for a GLFW
/// window to open: what `java -XstartOnFirstThread` arranges.
pub const JVM_ON_FIRST_THREAD: bool = true;

/// No discrete-GPU hint applies — the unified/bundle model leaves nothing to set.
pub fn force_gpu() {}

/// `-XstartOnFirstThread` is `java`'s own, and the VM refuses it as an option
/// it does not know. dgpuj is always what it asks for, so it is dropped here.
pub const LAUNCHER_OPTIONS: &[&str] = &["-XstartOnFirstThread"];

/// Say that `main` is on the first thread, the way `java -XstartOnFirstThread`
/// says it: LWJGL and AWT both look for this variable, named after the
/// process, before they will open a window. Nothing else needs preparing, the
/// JDK's libraries finding each other by `@loader_path`.
pub fn prepare_jvm(_jvm: &Path) -> Result<(), String> {
    let key = format!("JAVA_STARTED_ON_FIRST_THREAD_{}", std::process::id());
    std::env::set_var(key, "1");
    Ok(())
}

/// A VM option as the VM reads it: its bytes, which is what `java` hands on.
pub fn vm_option(option: &str) -> Result<CString, String> {
    CString::new(option).map_err(|_| format!("NUL byte in option: {option}"))
}
