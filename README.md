# dgpuj

`java`, on the machine's **discrete GPU**.

```
dgpuj <java> <whatever java takes>
```

`<java>` is a path to a `java`, or the bare word for the one on `PATH`. dgpuj
goes in front of a launch line the way `nice` does:

```
dgpuj C:\jdk-17\bin\java.exe -Xmx4G -cp client.jar;lib\* net.minecraft.client.main.Main
dgpuj java -jar server.jar
```

## Why

On a hybrid laptop a Java game starts on the integrated GPU. Which GPU a
program gets is decided for the process that draws, and on Windows by looking
at that process's executable, so a wrapper that merely starts `java.exe`
changes nothing. dgpuj *is* that `java`, with the hint.

| OS          | What dgpuj does                                                                                                                                           |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Windows** | Exports `NvOptimusEnablement` / `AmdPowerXpressRequestHighPerformance` from the exe, then makes `java.exe`'s one call, `JLI_Launch`, into the JDK's `jli.dll`. |
| **Linux**   | Sets the NVIDIA PRIME render-offload variables, if that driver is loaded and you have not set them, then runs the rest of the line.                        |
| **macOS**   | Runs the rest of the line. There is nothing to force.                                                                                                     |

Everything after `<java>` is read by the JDK's own launcher, so `-jar`,
`-cp lib\*`, `@argfiles`, agents and the rest behave exactly as under `java`.

## Which Java

Any from 8 on, JDK or JRE.

- **A path** is that Java. Either slash, with `.exe` or without.
- **The bare word** is the one on `PATH`. On Windows there has to be a real
  Java `bin` there, not only an installer's `java.exe` shim.
- **Java 8 on Windows** has to have `dgpuj.exe` beside its `java.exe`, in the
  JDK's `bin`, and be given the bare word. Java 8 finds itself from where its
  launcher is.

On Windows a Java that is not there is exit code 1 and no message: see below.

## It fits in one TCP packet

The Windows exe is 1,392 bytes. An Ethernet packet carries 1,460. Nothing
needed this, and CI fails if it stops being true.

It got there by having no C runtime and asking `jli.dll` for whatever it would
have brought: memory, the splitting of the command line, the launch. What it
gave up is the room to say what went wrong.

## Build

```
make                    # ./dgpuj or ./dgpuj.exe, for this machine
make TARGET=<triple>    # for another, named as the release archives are
make test               # launches $JAVA_HOME through it, on every path the tests know
```

A Windows target builds on any machine with clang, lld-link and llvm-dlltool.
The tests are a Java program and run in CI on Java 8, 17 and 25, on Windows
(x64 and ARM), Linux and macOS.

[Releases](https://github.com/harmoniya-net/dgpuj/releases) carry one archive
per target, with the binary at its root.

## License

MIT
