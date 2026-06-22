//! dgpuj — force the discrete GPU on hybrid-graphics systems, then run the JVM
//! *in this process* so the choice actually applies.
//!
//! Why in-process (and not a wrapper that spawns `java`/`javaw`): GPU selection
//! is decided per-process at GL/D3D context-creation time, for the process that
//! creates the context, with no inheritance to children. A launcher that just
//! *spawns* the JVM is useless — the child is a different process with its own
//! environment/exe. So dgpuj applies the per-OS GPU hint to ITSELF and loads the
//! JVM via `JNI_CreateJavaVM`, making this the process that owns the context.
//!
//! Per-OS forcing (each is safe-by-default — a no-op on non-hybrid machines):
//! - Windows: export `NvOptimusEnablement` / `AmdPowerXpressRequestHighPerformance`
//!   from this exe; the driver reads them from the export table (see build.rs).
//! - Linux: set the NVIDIA PRIME render-offload env vars before launch, but only
//!   when the proprietary NVIDIA driver is present (`/proc/driver/nvidia`) and
//!   the vars aren't already set — `__GLX_VENDOR_LIBRARY_NAME=nvidia` on a
//!   non-NVIDIA box would break GL.
//! - macOS: nothing to force (Apple Silicon has one GPU; the legacy Intel lever
//!   was an `.app` `Info.plist` key, out of reach of a bare binary). Runs as a
//!   plain in-process launcher — pass `-XstartOnFirstThread` for LWJGL/GLFW.
//!
//! CLI contract — a near drop-in for `java`:
//!
//!   dgpuj [--dgpuj-home DIR | --dgpuj-jvm PATH] \
//!         [VM options...] <main.Class> [program args...]
//!
//! Everything after the optional `--dgpuj-*` flags is parsed exactly like the
//! `java` launcher: tokens starting with `-` are JVM options (with `-cp` /
//! `-classpath` / `--class-path` translated to `-Djava.class.path=`, since
//! `JNI_CreateJavaVM` only understands the latter), the first bare token is the
//! main class, and the rest are passed to `main(String[])`.
//!
//! The JVM library is located from `--dgpuj-jvm`, then `--dgpuj-home`, then
//! `$JAVA_HOME` — under `bin\server\jvm.dll` (Windows), `lib/server/libjvm.so`
//! (Linux), or `lib/server/libjvm.dylib` (macOS). `-jar`/`@argfiles` unsupported.

use std::process::ExitCode;

mod cli;
mod jvm;

// Per-OS specifics — the discrete-GPU hint plus the HotSpot library path — live
// in one file per platform; the cfg selects exactly one.
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
    // Apply the platform's discrete-GPU hint before the JVM (and its GL stack)
    // initialise. On Windows the link-time exports already did the work.
    platform::force_gpu();

    let inv = cli::parse(std::env::args().skip(1))?;
    let jvm_path = jvm::resolve(inv.jvm_dll, inv.java_home)?;
    jvm::launch(&jvm_path, &inv.vm_opts, &inv.main_class, &inv.prog_args)
}
