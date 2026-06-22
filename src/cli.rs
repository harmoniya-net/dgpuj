//! Command-line parsing: split argv into the JVM location, VM options, main
//! class, and program args — mirroring the `java` launcher closely enough for a
//! classpath + main-class launch. Pure (no I/O), so it's unit-tested below.

use std::path::PathBuf;

/// A fully-parsed launch request.
pub struct Invocation {
    /// Explicit path to the JVM shared library (`--dgpuj-jvm`).
    pub jvm_dll: Option<PathBuf>,
    /// JDK/JRE home to derive the JVM library from (`--dgpuj-home`).
    pub java_home: Option<PathBuf>,
    /// JVM options, in order, as `JNI_CreateJavaVM` expects them.
    pub vm_opts: Vec<String>,
    /// Fully-qualified main class (dotted form, as written on the CLI).
    pub main_class: String,
    /// Arguments passed to `main(String[])`.
    pub prog_args: Vec<String>,
}

/// Parse the program arguments (excluding `argv[0]`).
pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Invocation, String> {
    let mut args = args.into_iter().peekable();

    // 1. Consume wrapper-specific leading flags.
    let mut jvm_dll: Option<PathBuf> = None;
    let mut java_home: Option<PathBuf> = None;
    while let Some(a) = args.peek() {
        match a.as_str() {
            "--dgpuj-jvm" => {
                args.next();
                jvm_dll = Some(args.next().ok_or("--dgpuj-jvm needs a path")?.into());
            }
            "--dgpuj-home" => {
                args.next();
                java_home = Some(args.next().ok_or("--dgpuj-home needs a directory")?.into());
            }
            _ => break,
        }
    }

    // 2. Split the rest java-style: [vm options] <main class> [program args].
    let mut vm_opts: Vec<String> = Vec::new();
    let mut main_class: Option<String> = None;
    let mut prog_args: Vec<String> = Vec::new();
    while let Some(a) = args.next() {
        if main_class.is_some() {
            prog_args.push(a);
            continue;
        }
        match a.as_str() {
            // JNI only honours -Djava.class.path; the -cp family is launcher sugar.
            "-cp" | "-classpath" | "--class-path" => {
                let cp = args.next().ok_or(format!("{a} needs a value"))?;
                vm_opts.push(format!("-Djava.class.path={cp}"));
            }
            _ if a.starts_with('-') => vm_opts.push(a),
            _ => main_class = Some(a),
        }
    }

    Ok(Invocation {
        jvm_dll,
        java_home,
        vm_opts,
        main_class: main_class.ok_or("no main class given")?,
        prog_args,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn parse_ok(a: &[&str]) -> Invocation {
        parse(a.iter().map(|s| s.to_string())).unwrap()
    }

    #[test]
    fn splits_vm_opts_main_and_program_args() {
        let inv = parse_ok(&["-Xmx2G", "-Dfoo=bar", "net.Main", "--user", "x"]);
        assert_eq!(inv.vm_opts, ["-Xmx2G", "-Dfoo=bar"]);
        assert_eq!(inv.main_class, "net.Main");
        assert_eq!(inv.prog_args, ["--user", "x"]);
        assert!(inv.jvm_dll.is_none() && inv.java_home.is_none());
    }

    #[test]
    fn translates_every_classpath_flag() {
        for flag in ["-cp", "-classpath", "--class-path"] {
            let inv = parse_ok(&[flag, "a.jar:b.jar", "Main"]);
            assert_eq!(inv.vm_opts, ["-Djava.class.path=a.jar:b.jar"]);
            assert_eq!(inv.main_class, "Main");
        }
    }

    #[test]
    fn consumes_leading_wrapper_flags() {
        let inv = parse_ok(&["--dgpuj-home", "/opt/jdk", "Main"]);
        assert_eq!(inv.java_home.as_deref(), Some(Path::new("/opt/jdk")));

        let inv = parse_ok(&["--dgpuj-jvm", "/x/libjvm.so", "Main"]);
        assert_eq!(inv.jvm_dll.as_deref(), Some(Path::new("/x/libjvm.so")));
    }

    #[test]
    fn program_args_after_main_are_passed_through_verbatim() {
        // Tokens that look like flags/classpath are program args once past main.
        let inv = parse_ok(&["Main", "-cp", "ignored", "-X"]);
        assert_eq!(inv.main_class, "Main");
        assert_eq!(inv.prog_args, ["-cp", "ignored", "-X"]);
        assert!(inv.vm_opts.is_empty());
    }

    #[test]
    fn errors_without_a_main_class() {
        assert!(parse(["-Xmx1G".to_string()]).is_err());
    }

    #[test]
    fn errors_on_dangling_value_flags() {
        assert!(parse(["-cp".to_string()]).is_err());
        assert!(parse(["--dgpuj-home".to_string()]).is_err());
    }
}
