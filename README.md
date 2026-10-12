# dgpuj

`java`, on the machine's **discrete GPU**. One binary and one launch line, on
Windows / Linux / macOS.

## Why this exists

On a hybrid laptop, Java apps default to the **integrated** GPU. Which GPU a
program gets is decided **for the process that creates the GL/D3D context**,
and on Windows by looking at that process's executable. So a wrapper that
merely *starts* `java.exe` changes nothing: the game is drawn by `java.exe`,
which carries no hint. `dgpuj` is `java.exe` with the hint.

| OS | What dgpuj does | Notes |
|----|-----------------|-------|
| **Windows** | Exports `NvOptimusEnablement` / `AmdPowerXpressRequestHighPerformance` from the `.exe`, then calls `JLI_Launch` | Must be the exe, not a DLL — so stock `java.exe`/`javaw.exe` (every vendor) can't carry it. Inert without a hybrid GPU. |
| **Linux** | Sets the NVIDIA PRIME render-offload variables, then `exec`s the JDK's `java` | Only when the proprietary NVIDIA driver is present (`/proc/driver/nvidia`), and never clobbering variables you already set. |
| **macOS** | `exec`s the JDK's `java` | Nothing to force: Apple Silicon has one GPU, and the Intel lever was an `.app` `Info.plist` key. |

`java.exe` is itself one call, `JLI_Launch`, into the JDK's `jli.dll`, which
reads the arguments, loads the VM and runs `main`. dgpuj makes the same call
into the same library. What a launch line means is therefore the JDK's to say:
`-jar`, `-cp lib\*`, `--add-opens x`, `-Xss`, agents and whatever a later Java
adds all behave as they do under `java`, because it is the same code.

It was not always so. Up to 0.4 dgpuj hosted the VM itself, through
`JNI_CreateJavaVM`, and had to do again everything `java` does around it. Each
thing it missed was a launch that worked under `java` and not under dgpuj.

## Usage

```
dgpuj [--dgpuj-home DIR] <whatever java takes>
```

The JDK is `--dgpuj-home`, else the one dgpuj is installed in the `bin` of,
else `$JAVA_HOME`.

```
dgpuj --dgpuj-home C:\jdk-17 -Xmx4G -cp client.jar;lib\* net.minecraft.client.main.Main --username Dev
```

**Java 8 on Windows** finds its own files from where its launcher is, and
cannot be pointed at: install `dgpuj.exe` into that JDK's `bin`, beside
`java.exe`, and leave `--dgpuj-home` out. Java 9 and later work from anywhere.
dgpuj says so itself when asked to launch a Java 8 it is not beside.

**Not supported:** `@argfiles` and `JDK_JAVA_OPTIONS`, which `java.exe` reads
before it calls its library. (On Linux and macOS they work, `java` being what
runs.)

### On Windows, beyond the one call

- **The JDK's `bin` is put on the DLL search path.** The JDK's DLLs import each
  other by bare name, and Windows looks a bare name up beside the *executable*.
  Beside `java.exe` that is the JDK; beside `dgpuj.exe` it is not, so the search
  went on to `PATH`. With a `-javaagent`, a second Java installed on the
  machine lent its `java.dll`, and the first DNS lookup died with
  `InternalError: platform encoding not initialized`.
- **Arguments are handed over in the ANSI code page**, as `java.exe` receives
  them. As UTF-8, a class path under `C:\Users\Олексій` named a directory that
  does not exist.
- **The C runtime is linked in.** `dgpuj.exe` does not need `VCRUNTIME140.dll`,
  which a clean Windows does not have.

Each is held by a test that launches a real JDK (`tests/launch.rs`), on Java
8, 17 and 25 on all three systems.

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

[opys](https://github.com/harmoniya-net/opys) uses the **same** `command` on
every platform: its `dgpuj()` plugin ships the released binary, `command`
points at it, and the arguments are `--dgpuj-home` and then what `java` would
get. dgpuj does the right per-OS thing internally, so a manifest needs no
per-OS branching.

## License

MIT
