//! Command-line parsing: take dgpuj's own flag off the front and leave the
//! rest for `java`, whose arguments they are. Pure (no I/O), so it's
//! unit-tested below.

use std::ffi::OsString;
use std::path::PathBuf;

/// A parsed launch request.
#[derive(Debug)]
pub struct Invocation {
    /// The JDK to launch (`--dgpuj-home`).
    pub java_home: Option<PathBuf>,
    /// Everything else, untouched: what `java` would have been given.
    pub java_args: Vec<OsString>,
}

/// Parse the program arguments (excluding `argv[0]`).
pub fn parse<I: IntoIterator<Item = OsString>>(args: I) -> Result<Invocation, String> {
    let mut args = args.into_iter().peekable();
    let mut java_home = None;
    while let Some(flag) = args.peek().and_then(|a| a.to_str()) {
        match flag {
            "--dgpuj-home" => {
                args.next();
                java_home = Some(args.next().ok_or("--dgpuj-home needs a directory")?.into());
            }
            // Named the VM's library while dgpuj loaded it. The JDK loads its
            // own now, so there is nothing for a path to one to choose.
            "--dgpuj-jvm" => {
                return Err("--dgpuj-jvm is gone: name the JDK with --dgpuj-home".into());
            }
            _ => break,
        }
    }
    Ok(Invocation {
        java_home,
        java_args: args.collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn parse_args(a: &[&str]) -> Result<Invocation, String> {
        parse(a.iter().map(OsString::from))
    }

    #[test]
    fn leaves_javas_arguments_as_they_were_written() {
        let inv = parse_args(&["-Xmx2G", "-cp", "a.jar", "net.Main", "--user", "x"]).unwrap();
        assert_eq!(
            inv.java_args,
            ["-Xmx2G", "-cp", "a.jar", "net.Main", "--user", "x"]
        );
        assert!(inv.java_home.is_none());
    }

    #[test]
    fn takes_its_own_flag_off_the_front_only() {
        let inv = parse_args(&["--dgpuj-home", "/opt/jdk", "Main", "--dgpuj-home", "x"]).unwrap();
        assert_eq!(inv.java_home.as_deref(), Some(Path::new("/opt/jdk")));
        assert_eq!(inv.java_args, ["Main", "--dgpuj-home", "x"]);
    }

    #[test]
    fn errors_on_a_home_with_no_directory() {
        assert!(parse_args(&["--dgpuj-home"]).is_err());
    }

    #[test]
    fn says_where_the_jvm_flag_went() {
        let err = parse_args(&["--dgpuj-jvm", "/x/libjvm.so", "Main"]).unwrap_err();
        assert!(err.contains("--dgpuj-home"), "{err}");
    }
}
