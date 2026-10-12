//! Windows: force the discrete GPU by exporting the driver-selection symbols
//! from this executable.
//!
//! The Optimus / PowerXpress driver reads the DWORD value of `NvOptimusEnablement`
//! / `AmdPowerXpressRequestHighPerformance` from the export table of the process
//! that creates the GL/D3D context (value `0x1` == "use High Performance
//! Graphics"). `#[used]` keeps the compiler from dropping the statics; build.rs
//! adds the `/EXPORT:` linker args that place them in the PE export directory.
//! Harmless on machines with no hybrid NVIDIA/AMD GPU.

#![allow(non_upper_case_globals)]

use std::ffi::CString;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

/// Where HotSpot keeps `jvm.dll` under a home: `bin\server`, and in a Java 8
/// JDK under its `jre`.
pub const JVM_LIB_RELS: &[&str] = &[r"bin\server\jvm.dll", r"jre\bin\server\jvm.dll"];

#[used]
#[no_mangle]
pub static NvOptimusEnablement: u32 = 0x0000_0001;
#[used]
#[no_mangle]
pub static AmdPowerXpressRequestHighPerformance: u32 = 0x0000_0001;

/// Nothing to do at runtime — the link-time exports above are the mechanism.
pub fn force_gpu() {}

/// The VM gets a thread of its own, the only one whose stack `-Xss` sizes.
pub const JVM_ON_FIRST_THREAD: bool = false;

/// Options that are `java`'s own and never reach the VM: none here.
pub const LAUNCHER_OPTIONS: &[&str] = &[];

const CP_ACP: u32 = 0;

#[link(name = "kernel32")]
extern "system" {
    fn SetDllDirectoryW(path: *const u16) -> i32;
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

/// A VM option as the VM reads it: in the system's ANSI code page.
///
/// HotSpot takes an option's bytes for that code page, and `java.exe` hands
/// them over so because its `argv` already is. Ours is Unicode, and UTF-8
/// bytes read as cp1251 turn a class path under `C:\Users\Олексій` into a
/// directory that does not exist. A character the code page lacks becomes
/// its best fit or `?`, exactly what `java.exe` would have been given.
pub fn vm_option(option: &str) -> Result<CString, String> {
    let wide: Vec<u16> = option.encode_utf16().collect();
    let mut bytes = Vec::new();
    if !wide.is_empty() {
        let len = wide.len() as i32;
        // SAFETY: `wide` holds `len` units; a null output asks for the size.
        let size = unsafe {
            WideCharToMultiByte(
                CP_ACP,
                0,
                wide.as_ptr(),
                len,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        };
        bytes.resize(size.max(0) as usize, 0u8);
        // SAFETY: as above, and `bytes` has room for the `size` just asked for.
        let written = unsafe {
            WideCharToMultiByte(
                CP_ACP,
                0,
                wide.as_ptr(),
                len,
                bytes.as_mut_ptr(),
                size,
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        };
        if written <= 0 {
            return Err(format!(
                "encoding option {option}: {}",
                std::io::Error::last_os_error()
            ));
        }
        bytes.truncate(written as usize);
    }
    CString::new(bytes).map_err(|_| format!("NUL byte in option: {option}"))
}

/// Put the JDK's `bin` on this process's DLL search path, ahead of `PATH`.
///
/// The JDK's own DLLs import each other by bare name (`instrument.dll` wants
/// `java.dll` and `jli.dll`), and Windows resolves a bare name from the
/// *executable's* directory, not the importing DLL's. Under `java.exe` that is
/// the JDK's `bin`; under dgpuj it is wherever dgpuj sits, so the search falls
/// through to `PATH` and finds another Java's `java.dll`, or none. With a
/// `-javaagent` that happens before the VM has loaded its own `java.dll`, and
/// the process ends up with two: the VM initialises one, `net.dll` binds to
/// the other, and the first DNS lookup dies with `InternalError: platform
/// encoding not initialized`.
pub fn prepare_jvm(jvm: &Path) -> Result<(), String> {
    // `…\bin\server\jvm.dll` -> `…\bin`, whichever layout it was found in.
    let bin = jvm
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| format!("no JDK bin directory above {}", jvm.display()))?;
    let wide: Vec<u16> = bin.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: `wide` is a NUL-terminated UTF-16 string that outlives the call.
    if unsafe { SetDllDirectoryW(wide.as_ptr()) } == 0 {
        return Err(format!(
            "adding {} to the DLL search path: {}",
            bin.display(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}
