//! macOS: there is no discrete-GPU lever reachable from a bare CLI binary.
//!
//! Apple Silicon has a single unified GPU, and the legacy Intel dual-GPU switch
//! was driven by an `.app` bundle's `Info.plist`
//! (`NSSupportsAutomaticGraphicsSwitching`), which a standalone executable can't
//! carry. So dgpuj is `java` and nothing more here, there so that one launch
//! line serves every platform.

use std::ffi::OsString;
use std::path::Path;

pub fn launch(home: &Path, args: &[OsString]) -> Result<i32, String> {
    crate::exec::java(home, args)
}
