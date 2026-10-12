//! dgpuj — `java`, on the machine's better graphics card.
//!
//! Which GPU a program gets is decided for the process that creates the GL or
//! D3D context, and on Windows by looking at that process's executable. So a
//! wrapper that starts `java.exe` changes nothing there: the game is drawn by
//! `java.exe`, which carries no hint. dgpuj is `java.exe` with the hint.
//!
//! That is nearly all it is. `java.exe` itself is one call, `JLI_Launch`, into
//! the JDK's `jli.dll`, which reads the arguments, loads the VM and runs
//! `main`. dgpuj makes the same call into the same library, so what a launch
//! line means is the JDK's to say and not ours.
//!
//! - Windows: export `NvOptimusEnablement` / `AmdPowerXpressRequestHighPerformance`
//!   from this exe (see build.rs), then `JLI_Launch`.
//! - Linux: the hint is environment, which a child keeps. Set the NVIDIA PRIME
//!   render-offload variables, where that driver is loaded and they are not
//!   already set, and `exec` the JDK's own `java`.
//! - macOS: nothing to force (Apple Silicon has one GPU; the Intel lever was an
//!   `.app`'s `Info.plist`). `exec` the JDK's own `java`.
//!
//! dgpuj hosted the VM itself once, through `JNI_CreateJavaVM`, and so had to
//! do again everything `java` does around it: the DLL search path, the options'
//! encoding, a thread for `main`, the module options, `-XstartOnFirstThread`.
//! Each was a launch that worked under `java` and not under dgpuj until it was
//! found.
//!
//!   dgpuj [--dgpuj-home DIR] <whatever java takes>
//!
//! The JDK is `--dgpuj-home`, else the one dgpuj is installed in the `bin` of,
//! else `$JAVA_HOME`.

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod cli;

// Per-OS specifics — the discrete-GPU hint and how `java` is become — live in
// one file per platform; the cfg selects exactly one.
#[cfg(windows)]
#[path = "windows.rs"]
mod platform;
#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod platform;
#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod platform;
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
compile_error!("dgpuj supports only Windows, Linux, and macOS");

#[cfg(unix)]
mod exec;

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(e) => {
            eprintln!("dgpuj: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<i32, String> {
    let inv = cli::parse(env::args_os().skip(1))?;
    let home = inv
        .java_home
        .or_else(own_home)
        .or_else(|| env::var_os("JAVA_HOME").map(PathBuf::from))
        .ok_or("no JDK: pass --dgpuj-home or set JAVA_HOME")?;
    platform::launch(&home, &inv.java_args)
}

/// The JDK dgpuj is installed in, if it sits in one's `bin` as `java` does.
fn own_home() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let home = exe.parent()?.parent()?;
    java(home).is_file().then(|| home.to_path_buf())
}

/// A JDK's own launcher.
fn java(home: &Path) -> PathBuf {
    home.join("bin")
        .join(format!("java{}", env::consts::EXE_SUFFIX))
}
