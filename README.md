# dgpuj

A tiny `java`-style launcher that forces the **discrete GPU** on hybrid-graphics
machines, then runs the JVM **in-process** so the choice actually applies to
your Java app (e.g. Minecraft). One binary, one CLI, on Windows / Linux / macOS.

## Why this exists

On a hybrid laptop, Java apps default to the **integrated** GPU. The universal
catch is that GPU selection is decided **per-process, for the process that
creates the GL/D3D context, with no inheritance to children** — so a wrapper
that merely *spawns* `java`/`javaw` is useless (the child is a different
process). `dgpuj` instead applies the per-OS GPU hint to **itself** and hosts
the JVM via `JNI_CreateJavaVM`, so the process that owns the GL context is the
one carrying the hint.

Each platform's hint is **safe-by-default** — a no-op on non-hybrid machines:

| OS | Mechanism | Notes |
|----|-----------|-------|
| **Windows** | Exports `NvOptimusEnablement` / `AmdPowerXpressRequestHighPerformance` from the `.exe` | Must be the exe, not a DLL — so stock `java.exe`/`javaw.exe` (every vendor) can't carry it. Inert without a hybrid GPU. |
| **Linux** | Sets NVIDIA PRIME render-offload env vars before launch | Only when the proprietary NVIDIA driver is present (`/proc/driver/nvidia`), and never clobbering vars you already set. |
| **macOS** | *(nothing to force)* | Apple Silicon has one GPU; the legacy Intel lever was an `.app` `Info.plist` key, unreachable from a bare binary. Runs as a plain in-process launcher. |

## Usage

A near drop-in for `java`:

```
dgpuj [--dgpuj-home DIR | --dgpuj-jvm PATH] \
      [VM options...] <main.Class> [program args...]
```

- The JVM library is located from `--dgpuj-jvm`, then `--dgpuj-home`, then
  `$JAVA_HOME` — under `bin\server\jvm.dll` (Windows), `lib/server/libjvm.so`
  (Linux), or `lib/server/libjvm.dylib` (macOS).
- Everything after the optional `--dgpuj-*` flags is parsed like the `java`
  launcher: `-…` tokens are JVM options, the first bare token is the main class,
  the rest go to `main(String[])`.
- `-cp` / `-classpath` / `--class-path` are translated to
  `-Djava.class.path=` (the only form `JNI_CreateJavaVM` understands).
- **macOS:** include `-XstartOnFirstThread` for any LWJGL3/GLFW app (as every
  Minecraft-on-macOS launch already does).

Example:

```
set JAVA_HOME=C:\jdk-17
dgpuj -Xmx4G -cp lib\* net.minecraft.client.main.Main --username Dev
```

**Not supported:** `-jar`, `@argfiles`, module-path launches with a manifest
main class. Pass an explicit main class + classpath.

## Build

```
cargo build --release
```

Windows uses the MSVC toolchain (the GPU exports use MSVC `/EXPORT:` linker
syntax). Prebuilt archives for Windows (x64/arm64), Linux (x64), and macOS
(arm64/x64) are attached to each
[GitHub release](https://github.com/harmoniya-net/dgpuj/releases) — `.zip` on
Windows, `.tar.gz` elsewhere, with the `dgpuj`/`dgpuj.exe` binary at the archive
root (the tarball preserves the executable bit).

## How it's wired into opys

[opys](https://github.com/harmoniya-net/opys) can use the **same** `command`
across platforms: ship the released `dgpuj` binary as an artifact, point
`command` at it, set `JAVA_HOME`, and forward the same args `java` would get.
dgpuj then does the right per-OS thing internally — so opys needs no per-OS
branching in the manifest. (Equivalently, on Linux opys could still set the
PRIME env vars natively via `envs`; on macOS no GPU hint is needed at all.)

## License

MIT
