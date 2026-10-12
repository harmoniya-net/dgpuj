//! Unix: become the JDK's own `java`.
//!
//! Nothing here needs dgpuj to stay the process, as Windows does: whatever
//! hint there is, is environment, and survives the `exec`.

use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

/// Replace this process with `<home>/bin/java <args>`. Returns only on failure.
pub fn java(home: &Path, args: &[OsString]) -> Result<i32, String> {
    let java = crate::java(home);
    let err = Command::new(&java).args(args).exec();
    Err(format!("starting {}: {err}", java.display()))
}
