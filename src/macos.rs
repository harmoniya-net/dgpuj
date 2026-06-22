//! macOS: there is no discrete-GPU lever reachable from a bare CLI binary.
//!
//! Apple Silicon has a single unified GPU, and the legacy Intel dual-GPU switch
//! was driven by an `.app` bundle's `Info.plist`
//! (`NSSupportsAutomaticGraphicsSwitching`), which a standalone executable can't
//! carry. So dgpuj is just an in-process JVM launcher here — remember
//! `-XstartOnFirstThread` for any LWJGL/GLFW app.

/// HotSpot ships `libjvm.dylib` under `<home>/lib/server/`.
pub const JVM_LIB_REL: &str = "lib/server/libjvm.dylib";

/// No discrete-GPU hint applies — the unified/bundle model leaves nothing to set.
pub fn force_gpu() {}
