# dgpuj

`java`, on the machine's **discrete GPU**. One C file, one launch line, on
Windows / Linux / macOS.

```
dgpuj <java> <whatever java takes>
```

`<java>` is the `java` to be: a path to one, or the bare word for the one on
`PATH`. dgpuj goes in front of a launch line the way `nice` does.

```
dgpuj C:\jdk-17\bin\java.exe -Xmx4G -cp client.jar;lib\* net.minecraft.client.main.Main
dgpuj java -jar server.jar
```

## Why this exists

On a hybrid laptop, Java apps default to the **integrated** GPU. Which GPU a
program gets is decided **for the process that creates the GL/D3D context**,
and on Windows by looking at that process's executable. So starting `java.exe`
from something else changes nothing: the game is drawn by `java.exe`, which
carries no hint. On Windows `dgpuj` is `java.exe` with the hint.

| OS | What dgpuj does | Notes |
|----|-----------------|-------|
| **Windows** | Exports `NvOptimusEnablement` / `AmdPowerXpressRequestHighPerformance` from the `.exe`, then calls `JLI_Launch` | Must be the exe, not a DLL — so stock `java.exe`/`javaw.exe` (every vendor) can't carry it. Inert without a hybrid GPU. |
| **Linux** | Sets the NVIDIA PRIME render-offload variables, then runs the rest of the line | Only when the proprietary NVIDIA driver is present (`/proc/driver/nvidia`), and never over variables you already set. |
| **macOS** | Runs the rest of the line | Nothing to force: Apple Silicon has one GPU, and the Intel lever was an `.app` `Info.plist` key. |

`java.exe` is itself one call, `JLI_Launch`, into its JDK's `jli.dll`, which
reads the arguments, loads the VM and runs `main`. dgpuj makes the same call
into the same library. What a launch line means is therefore the JDK's to say:
`-jar`, `-cp lib\*`, `--add-opens x`, `-Xss`, `@argfiles`, agents and whatever a
later Java adds all behave as they do under `java`, because it is the same
code.

It was not always so. Up to 0.4 dgpuj was a Rust program that hosted the VM
itself, through `JNI_CreateJavaVM`, and had to do again everything `java` does
around it. Each thing it missed was a launch that worked under `java` and not
under dgpuj.

## Which Java

Any, from 8 on, JDK or JRE: all dgpuj needs is in every Java's `bin`.

- **A path** names the `java` of a JDK, and that JDK is the one launched.
  Either slash, with `.exe` or without.
- **The bare word** is the one on `PATH`. On Windows that is asking for
  `jli.dll` by name, which looks beside `dgpuj.exe` first and on `PATH` after:
  so it needs a real Java `bin` there, and not only the `java.exe` shim an
  installer may have put on `PATH` in its place.
- **Java 8 on Windows** finds its own files from where its launcher is, and
  cannot be pointed at: install `dgpuj.exe` into its `bin`, beside `java.exe`,
  and give it the bare word. Java 9 and later work from anywhere.

## The Windows binary

It is 1.4 KB, small enough to fit in one TCP packet, which nothing needed. It
got there by asking `jli.dll` for everything it would otherwise have brought:
memory, the splitting of the command line, the launch. It has no C runtime,
so it needs no `VCRUNTIME140.dll`, which a clean Windows does not have.

What it does besides the one call:

- **The JDK's `bin` is put on the DLL search path.** The JDK's DLLs import each
  other by bare name, and Windows looks a bare name up beside the *executable*.
  Beside `java.exe` that is the JDK; beside `dgpuj.exe` it is not, so the search
  went on to `PATH`. With a `-javaagent`, a second Java installed on the
  machine lent its `java.dll`, and the first DNS lookup died with
  `InternalError: platform encoding not initialized`.
- **`<java>` is cut out of the command line** before `jli.dll` splits it.
  Shown a first word with no dash, it takes it for the main class and
  miscounts everything after.

What it gave up for the size is being told what went wrong: with no Java where
`<java>` says, it exits with 1 and prints nothing.

## Build and test

```
make                    # ./dgpuj or ./dgpuj.exe, for this machine
make TARGET=<triple>    # for another, named as the release archives are
make test               # launches $JAVA_HOME through it, every way the tests know
```

A Windows target builds anywhere there is clang, lld-link and llvm-dlltool, and
needs nothing of Windows, not even its headers. The tests are a Java program,
since a JDK is what they need anyway, and CI runs them on Java 8, 17 and 25 on
all three systems.

Prebuilt archives for Windows (x64/arm64), Linux (x64, static) and macOS
(arm64/x64) are attached to each
[GitHub release](https://github.com/harmoniya-net/dgpuj/releases) — `.zip` on
Windows, `.tar.gz` elsewhere, with the `dgpuj`/`dgpuj.exe` binary at the archive
root (the tarball preserves the executable bit).

## How it's wired into opys

[opys](https://github.com/harmoniya-net/opys) uses the **same** `command` on
every platform: its `dgpuj()` plugin ships the released binary, `command`
points at it, and the arguments are the `java` to run and then what it would
get. dgpuj does the right per-OS thing internally, so a manifest needs no
per-OS branching.

## License

MIT
