# dgpu-jvm

A tiny launcher that forces the **discrete GPU** on Windows hybrid-graphics
laptops (NVIDIA Optimus / AMD PowerXpress), then runs the JVM **in-process** so
the choice actually applies to your Java app (e.g. Minecraft).

## Why this exists

On a hybrid laptop, Java apps default to the **integrated** GPU. The known fix
is to export the magic data symbol `NvOptimusEnablement = 1` (or
`AmdPowerXpressRequestHighPerformance = 1`) from the executable — but two facts
make that hard for Java:

1. **The symbol must live in the `.exe`, not a DLL.** So it can't go in
   `jvm.dll`, in LWJGL, or in any library the JVM loads.
2. **The driver checks the process that creates the GL/D3D context, per
   process, with no inheritance.** So a wrapper that *spawns* `javaw.exe` as a
   child does nothing — the child's main module is `javaw.exe`, which has no
   export.

Stock `java.exe` / `javaw.exe` from every vendor (Temurin, Zulu, Corretto,
Oracle, Microsoft, …) is the same generic launcher with no export, which is why
vanilla Minecraft lands on the iGPU unless you set an NVIDIA Control Panel
profile or a Windows `UserGpuPreferences` entry by hand.

`dgpu-jvm` solves it the only clean way: it **is** the executable that exports
the symbols, and it hosts the JVM with `JNI_CreateJavaVM` so *this* process owns
the GL context. No registry writes, no driver profiles, no per-version path
fixups.

## Usage

A near drop-in for `java`:

```
dgpu-jvm [--dgpu-java-home DIR | --dgpu-jvm-dll PATH] \
         [VM options...] <main.Class> [program args...]
```

- The JVM library is located from `--dgpu-jvm-dll`, then `--dgpu-java-home`,
  then `$JAVA_HOME` (expects `<home>\bin\server\jvm.dll`).
- Everything after the optional `--dgpu-*` flags is parsed like the `java`
  launcher: `-…` tokens are JVM options, the first bare token is the main class,
  the rest go to `main(String[])`.
- `-cp` / `-classpath` / `--class-path` are translated to
  `-Djava.class.path=` (the only form `JNI_CreateJavaVM` understands).

Example:

```
set JAVA_HOME=C:\jdk-17
dgpu-jvm -Xmx4G -cp lib\* net.minecraft.client.main.Main --username Dev
```

**Not supported:** `-jar`, `@argfiles`, module-path launches with a manifest
main class. Pass an explicit main class + classpath.

## Build

Windows MSVC toolchain only (the GPU exports use MSVC `/EXPORT:` linker syntax):

```
cargo build --release --target x86_64-pc-windows-msvc
```

Prebuilt x64 and arm64 binaries are attached to each
[GitHub release](https://github.com/harmoniya-net/dgpu-jvm/releases).

## How it's wired into opys

[opys](https://github.com/harmoniya-net/opys) can adopt this as a Windows-only
`command` override: ship the released `dgpu-jvm.exe` as an artifact, point
`command` at it (rule-tagged to Windows), set `JAVA_HOME`, and forward the same
args `java` would get. On Linux, prefer the PRIME env vars
(`__NV_PRIME_RENDER_OFFLOAD=1`, `__GLX_VENDOR_LIBRARY_NAME=nvidia`) instead.

## License

MIT
