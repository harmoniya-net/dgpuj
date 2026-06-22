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

use std::env;
use std::ffi::{c_void, CString};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use jni::objects::{JObject, JValue};
use jni::sys::{
    jint, JNIEnv as SysJNIEnv, JNIInvokeInterface_, JavaVMInitArgs, JavaVMOption, JNI_FALSE,
    JNI_OK, JNI_VERSION_1_8,
};
use jni::JNIEnv;

// Per-OS specifics — the discrete-GPU hint plus the HotSpot library path — live
// in one module per platform; the cfg selects exactly one. macOS / other Unix
// have nothing to force, so they fall back to a tiny inline default.
#[cfg(windows)]
#[path = "windows.rs"]
mod platform;
#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod platform;
#[cfg(not(any(windows, target_os = "linux")))]
mod platform {
    #[cfg(target_os = "macos")]
    pub const JVM_LIB_REL: &str = "lib/server/libjvm.dylib";
    #[cfg(not(target_os = "macos"))]
    pub const JVM_LIB_REL: &str = "lib/server/libjvm.so";
    /// No discrete-GPU hint applies here (e.g. macOS unified GPU).
    pub fn force_gpu() {}
}

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

    let mut args = env::args().skip(1).peekable();

    // 1. Consume wrapper-specific leading flags.
    let mut jvm_dll: Option<PathBuf> = None;
    let mut java_home: Option<PathBuf> = None;
    while let Some(a) = args.peek() {
        match a.as_str() {
            "--dgpuj-jvm" => {
                args.next();
                jvm_dll = Some(args.next().ok_or("--dgpuj-jvm needs a path")?.into());
            }
            "--dgpuj-home" => {
                args.next();
                java_home = Some(args.next().ok_or("--dgpuj-home needs a directory")?.into());
            }
            _ => break,
        }
    }
    let jvm_path = resolve_jvm(jvm_dll, java_home)?;

    // 2. Split the rest java-style: [vm options] <main class> [program args].
    let mut vm_opts: Vec<String> = Vec::new();
    let mut main_class: Option<String> = None;
    let mut prog_args: Vec<String> = Vec::new();
    while let Some(a) = args.next() {
        if main_class.is_some() {
            prog_args.push(a);
            continue;
        }
        match a.as_str() {
            // JNI only honours -Djava.class.path; the -cp family is launcher sugar.
            "-cp" | "-classpath" | "--class-path" => {
                let cp = args.next().ok_or(format!("{a} needs a value"))?;
                vm_opts.push(format!("-Djava.class.path={cp}"));
            }
            _ if a.starts_with('-') => vm_opts.push(a),
            _ => main_class = Some(a),
        }
    }
    let main_class = main_class.ok_or("no main class given")?;

    launch(&jvm_path, &vm_opts, &main_class, &prog_args)
}

fn resolve_jvm(dll: Option<PathBuf>, home: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(p) = dll {
        return p
            .is_file()
            .then_some(p.clone())
            .ok_or_else(|| format!("jvm library not found: {}", p.display()));
    }
    let home = home
        .or_else(|| env::var_os("JAVA_HOME").map(PathBuf::from))
        .ok_or("no JVM location: pass --dgpuj-home / --dgpuj-jvm or set JAVA_HOME")?;
    let p = home.join(platform::JVM_LIB_REL);
    p.is_file()
        .then_some(p.clone())
        .ok_or_else(|| format!("jvm library not found: {}", p.display()))
}

/// Signature of `JNI_CreateJavaVM` exported by `jvm.dll`.
/// C: `jint JNI_CreateJavaVM(JavaVM **pvm, void **penv, void *args)`.
type CreateJavaVm = unsafe extern "system" fn(
    *mut *mut *const JNIInvokeInterface_,
    *mut *mut c_void,
    *mut c_void,
) -> jint;

fn launch(
    jvm_path: &Path,
    vm_opts: &[String],
    main_class: &str,
    prog_args: &[String],
) -> Result<i32, String> {
    // CStrings must outlive the JavaVMInitArgs that points into them.
    let opt_cstrs: Vec<CString> = vm_opts
        .iter()
        .map(|s| CString::new(s.as_str()).map_err(|_| format!("NUL byte in option: {s}")))
        .collect::<Result<_, _>>()?;
    let mut options: Vec<JavaVMOption> = opt_cstrs
        .iter()
        .map(|c| JavaVMOption {
            optionString: c.as_ptr().cast_mut(),
            extraInfo: std::ptr::null_mut(),
        })
        .collect();

    let mut init_args = JavaVMInitArgs {
        version: JNI_VERSION_1_8,
        nOptions: options.len() as jint,
        options: options.as_mut_ptr(),
        // Fail loudly on a bad VM option rather than silently ignoring it.
        ignoreUnrecognized: JNI_FALSE,
    };

    unsafe {
        let lib = libloading::Library::new(jvm_path)
            .map_err(|e| format!("loading {}: {e}", jvm_path.display()))?;
        let create: libloading::Symbol<CreateJavaVm> = lib
            .get(b"JNI_CreateJavaVM\0")
            .map_err(|e| format!("JNI_CreateJavaVM not found in jvm.dll: {e}"))?;

        let mut vm: *mut *const JNIInvokeInterface_ = std::ptr::null_mut();
        let mut env_ptr: *mut c_void = std::ptr::null_mut();
        let rc = create(
            &mut vm,
            &mut env_ptr,
            &mut init_args as *mut JavaVMInitArgs as *mut c_void,
        );
        if rc != JNI_OK {
            return Err(format!("JNI_CreateJavaVM failed (code {rc})"));
        }

        // The calling thread is now attached; wrap its env for ergonomic calls.
        let mut env = JNIEnv::from_raw(env_ptr as *mut SysJNIEnv)
            .map_err(|e| format!("wrapping JNIEnv: {e}"))?;

        let result = invoke_main(&mut env, main_class, prog_args);

        // Block until all non-daemon JVM threads finish (what the `java`
        // launcher does) — otherwise we'd exit while the game is still running.
        if let Some(destroy) = (**vm).DestroyJavaVM {
            destroy(vm);
        }

        result
    }
}

fn invoke_main(env: &mut JNIEnv, main_class: &str, prog_args: &[String]) -> Result<i32, String> {
    let internal = main_class.replace('.', "/");
    let class = env
        .find_class(&internal)
        .map_err(|e| explain(env, format!("locating main class `{main_class}`"), e))?;

    let string_class = env
        .find_class("java/lang/String")
        .map_err(|e| format!("locating java.lang.String: {e}"))?;
    let argv = env
        .new_object_array(prog_args.len() as jint, &string_class, JObject::null())
        .map_err(|e| format!("allocating args array: {e}"))?;
    for (i, a) in prog_args.iter().enumerate() {
        let s = env
            .new_string(a)
            .map_err(|e| format!("encoding arg {i}: {e}"))?;
        env.set_object_array_element(&argv, i as jint, &s)
            .map_err(|e| format!("setting arg {i}: {e}"))?;
    }

    let argv: JObject = argv.into();
    env.call_static_method(
        class,
        "main",
        "([Ljava/lang/String;)V",
        &[JValue::Object(&argv)],
    )
    .map_err(|e| explain(env, "calling main(String[])".into(), e))?;

    // A pending exception turns into a non-zero exit, after printing the trace.
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
        return Ok(1);
    }
    Ok(0)
}

/// Attach a pending Java stack trace (if any) to an error message.
fn explain(env: &mut JNIEnv, ctx: String, e: jni::errors::Error) -> String {
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    format!("{ctx}: {e}")
}
