//! Locate the JVM shared library and run it in-process via JNI.
//!
//! Loading `jvm.dll`/`libjvm.so`/`libjvm.dylib` into *this* process (rather than
//! spawning `java`) is what makes the per-OS GPU hint apply — see the crate
//! docs. We `dlopen` the library, call `JNI_CreateJavaVM`, invoke the target
//! `main(String[])`, then block in `DestroyJavaVM` until the app's non-daemon
//! threads finish (mirroring the stock `java` launcher).

use std::env;
use std::ffi::{c_void, CString};
use std::path::{Path, PathBuf};

use jni::objects::{JObject, JValue};
use jni::sys::{
    jint, JNIEnv as SysJNIEnv, JNIInvokeInterface_, JavaVMInitArgs, JavaVMOption, JNI_FALSE,
    JNI_OK, JNI_VERSION_1_8,
};
use jni::JNIEnv;

use crate::platform;

/// Resolve the JVM library path from an explicit dll, then a home dir, then
/// `$JAVA_HOME` (joined with the platform's HotSpot-relative library path).
pub fn resolve(dll: Option<PathBuf>, home: Option<PathBuf>) -> Result<PathBuf, String> {
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

/// Signature of `JNI_CreateJavaVM` exported by the JVM library.
/// C: `jint JNI_CreateJavaVM(JavaVM **pvm, void **penv, void *args)`.
type CreateJavaVm = unsafe extern "system" fn(
    *mut *mut *const JNIInvokeInterface_,
    *mut *mut c_void,
    *mut c_void,
) -> jint;

/// Load the JVM in-process and run `main_class`'s `main(prog_args)`.
pub fn launch(
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
            .map_err(|e| format!("JNI_CreateJavaVM not found in jvm library: {e}"))?;

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
