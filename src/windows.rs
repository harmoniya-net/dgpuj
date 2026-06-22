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

/// HotSpot ships `jvm.dll` under `<home>\bin\server\`.
pub const JVM_LIB_REL: &str = r"bin\server\jvm.dll";

#[used]
#[no_mangle]
pub static NvOptimusEnablement: u32 = 0x0000_0001;
#[used]
#[no_mangle]
pub static AmdPowerXpressRequestHighPerformance: u32 = 0x0000_0001;

/// Nothing to do at runtime — the link-time exports above are the mechanism.
pub fn force_gpu() {}
