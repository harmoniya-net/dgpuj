//! Linux: force the NVIDIA discrete GPU via PRIME render offload.
//!
//! Unlike Windows, GPU selection here is environment-driven, and the env is read
//! by the GL/Vulkan loader when the context is created — so setting these before
//! the in-process JVM spins up its GL stack is enough.

use std::env;
use std::path::Path;

/// HotSpot ships `libjvm.so` under `<home>/lib/server/`.
pub const JVM_LIB_REL: &str = "lib/server/libjvm.so";

/// Opt into NVIDIA PRIME render offload — but only when the proprietary NVIDIA
/// driver is actually loaded, and never clobbering values the caller already
/// set. On a non-NVIDIA box `__GLX_VENDOR_LIBRARY_NAME=nvidia` would make the
/// GLVND loader fail to find a vendor library and break GL, so the
/// `/proc/driver/nvidia` gate keeps this safe-by-default — the same spirit as
/// the Windows export being inert without a hybrid GPU.
pub fn force_gpu() {
    if !Path::new("/proc/driver/nvidia/version").exists() {
        return;
    }
    set_if_unset("__NV_PRIME_RENDER_OFFLOAD", "1");
    set_if_unset("__GLX_VENDOR_LIBRARY_NAME", "nvidia");
    set_if_unset("__VK_LAYER_NV_optimus", "NVIDIA_only");
}

fn set_if_unset(key: &str, val: &str) {
    if env::var_os(key).is_none() {
        env::set_var(key, val);
    }
}
