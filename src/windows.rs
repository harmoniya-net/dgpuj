//! Windows: be `java.exe`, with the driver-selection symbols exported.
//!
//! The Optimus / PowerXpress driver reads the DWORD value of `NvOptimusEnablement`
//! / `AmdPowerXpressRequestHighPerformance` from the export table of the process
//! that creates the GL/D3D context (value `0x1` == "use High Performance
//! Graphics"). `#[used]` keeps the compiler from dropping the statics; build.rs
//! adds the `/EXPORT:` linker args that place them in the PE export directory.
//! Harmless on machines with no hybrid NVIDIA/AMD GPU.
//!
//! The rest is `java.exe`'s own `main`: one call to `JLI_Launch`.

#![allow(non_upper_case_globals)]

use std::ffi::{c_char, c_void, CString, OsStr, OsString};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

#[used]
#[no_mangle]
pub static NvOptimusEnablement: u32 = 0x0000_0001;
#[used]
#[no_mangle]
pub static AmdPowerXpressRequestHighPerformance: u32 = 0x0000_0001;

const CP_ACP: u32 = 0;

#[link(name = "kernel32")]
extern "system" {
    fn SetDllDirectoryW(path: *const u16) -> i32;
    fn LoadLibraryW(path: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    fn WideCharToMultiByte(
        code_page: u32,
        flags: u32,
        wide: *const u16,
        wide_len: i32,
        out: *mut u8,
        out_len: i32,
        default_char: *const u8,
        used_default_char: *mut i32,
    ) -> i32;
}

/// `JLI_Launch`, as `jli.dll` has exported it since Java 6 (`java.h`).
type JliLaunch = unsafe extern "system" fn(
    argc: i32,
    argv: *mut *mut c_char,
    jargc: i32,
    jargv: *const *const c_char,
    appclassc: i32,
    appclassv: *const *const c_char,
    fullversion: *const c_char,
    dotversion: *const c_char,
    pname: *const c_char,
    lname: *const c_char,
    javaargs: u8,
    cpwildcard: u8,
    javaw: u8,
    ergo: i32,
) -> i32;

/// Run `java <args>` in this process, through the JDK's own launcher library.
pub fn launch(home: &Path, args: &[OsString]) -> Result<i32, String> {
    let bin = jli_dir(home)?;
    check_placement(home)?;

    // The JDK's DLLs import each other by bare name, and Windows looks a bare
    // name up beside the *executable*, which under `java.exe` is this
    // directory. Without it the search goes on to `PATH`: with a `-javaagent`
    // another Java installed on the machine lent its `java.dll`, and the
    // first DNS lookup died with "platform encoding not initialized".
    let bin_wide = wide(bin.as_os_str());
    // SAFETY: `bin_wide` is a NUL-terminated UTF-16 string that outlives the call.
    if unsafe { SetDllDirectoryW(bin_wide.as_ptr()) } == 0 {
        return Err(os_error(format!(
            "adding {} to the DLL search path",
            bin.display()
        )));
    }

    let jli = bin.join("jli.dll");
    let jli_wide = wide(jli.as_os_str());
    // SAFETY: a NUL-terminated path; the library is never unloaded, and the
    // symbol is `JLI_Launch`, whose signature `JliLaunch` spells out.
    let jli_launch: JliLaunch = unsafe {
        let module = LoadLibraryW(jli_wide.as_ptr());
        if module.is_null() {
            return Err(os_error(format!("loading {}", jli.display())));
        }
        let symbol = GetProcAddress(module, c"JLI_Launch".as_ptr());
        if symbol.is_null() {
            return Err(format!("no JLI_Launch in {}", jli.display()));
        }
        std::mem::transmute::<*mut c_void, JliLaunch>(symbol)
    };

    // `argv`, as `java.exe` would have had it: the program, then the arguments,
    // then a null. Handed over for good, since the launcher keeps pointers.
    let mut argv: Vec<*mut c_char> = std::iter::once(OsStr::new("java"))
        .chain(args.iter().map(OsString::as_os_str))
        .map(|a| ansi(a).map(CString::into_raw))
        .collect::<Result<_, _>>()?;
    let argc = argv.len() as i32;
    argv.push(std::ptr::null_mut());

    // The constants `java.exe` is built with (`main.c`, `defines.h`): no
    // arguments of its own, wildcards in the class path expanded, a console.
    // SAFETY: `argv` holds `argc` C strings and a null, all alive for the call.
    Ok(unsafe {
        jli_launch(
            argc,
            argv.as_mut_ptr(),
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
            c"dgpuj".as_ptr(),
            c"0.0".as_ptr(),
            c"java".as_ptr(),
            c"openjdk".as_ptr(),
            0,
            1,
            0,
            0,
        )
    })
}

/// The directory `jli.dll` is in: `bin`, or in a Java 8 JDK its `jre\bin`.
fn jli_dir(home: &Path) -> Result<PathBuf, String> {
    [home.join("bin"), home.join("jre").join("bin")]
        .into_iter()
        .find(|dir| dir.join("jli.dll").is_file())
        .ok_or_else(|| format!("no JDK at {}: no bin\\jli.dll", home.display()))
}

/// Java 8 finds its own files from where its launcher is, and nothing else:
/// `jli.dll` learned to look beside itself in Java 9. So for Java 8 dgpuj has
/// to be where `java.exe` is, and saying so beats the launcher's "could not
/// find java.dll".
fn check_placement(home: &Path) -> Result<(), String> {
    if home.join("lib").join("modules").is_file() {
        return Ok(());
    }
    let bin = home.join("bin");
    let beside_java = std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.canonicalize().ok()? == bin.canonicalize().ok()?))
        .unwrap_or(false);
    if beside_java {
        Ok(())
    } else {
        Err(format!(
            "Java 8 finds itself from where its launcher is: install dgpuj.exe into {}",
            bin.display()
        ))
    }
}

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(Some(0)).collect()
}

fn os_error(doing: String) -> String {
    format!("{doing}: {}", std::io::Error::last_os_error())
}

/// An argument in the system's ANSI code page, which is what `java.exe` is
/// given and what the launcher reads. Ours is Unicode, and UTF-8 bytes read as
/// cp1251 turn a class path under `C:\Users\Олексій` into a directory that
/// does not exist. A character the code page lacks becomes its best fit or
/// `?`, exactly what `java.exe` would have been handed.
fn ansi(arg: &OsStr) -> Result<CString, String> {
    let wide: Vec<u16> = arg.encode_wide().collect();
    let mut bytes = Vec::new();
    if !wide.is_empty() {
        let len = wide.len() as i32;
        let convert = |out: *mut u8, out_len: i32| {
            // SAFETY: `wide` holds `len` units, and `out` has room for
            // `out_len` bytes, none being asked for when it is null.
            unsafe {
                WideCharToMultiByte(
                    CP_ACP,
                    0,
                    wide.as_ptr(),
                    len,
                    out,
                    out_len,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                )
            }
        };
        let size = convert(std::ptr::null_mut(), 0);
        if size <= 0 {
            return Err(os_error(format!("encoding {}", arg.to_string_lossy())));
        }
        bytes.resize(size as usize, 0u8);
        let written = convert(bytes.as_mut_ptr(), size);
        bytes.truncate(written.max(0) as usize);
    }
    CString::new(bytes).map_err(|_| format!("NUL byte in {}", arg.to_string_lossy()))
}
