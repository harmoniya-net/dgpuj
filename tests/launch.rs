//! dgpuj against a real JDK: the launches a `java` would make, and the ones
//! that went wrong under dgpuj and not under `java`.
//!
//! The JDK is `DGPUJ_TEST_JAVA_HOME`, or `JAVA_HOME`. Without one the tests
//! pass having run nothing, except under CI, where that would be a check that
//! checks nothing.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

const DGPUJ: &str = env!("CARGO_BIN_EXE_dgpuj");

const PROBE: &str = r#"
import java.net.InetAddress;

public class Probe {
    static int depth(int n) {
        return n == 0 ? 0 : 1 + depth(n - 1);
    }

    // Code points, so that what is compared never passes through stdout's encoding.
    static String hex(String s) {
        StringBuilder out = new StringBuilder();
        s.codePoints().forEach(c -> out.append(Integer.toHexString(c)).append(' '));
        return out.toString().trim();
    }

    public static void main(String[] args) throws Exception {
        switch (args[0]) {
            case "echo":
                for (int i = 1; i < args.length; i++) System.out.println(hex(args[i]));
                break;
            case "exit":
                System.exit(Integer.parseInt(args[1]));
                break;
            case "throw":
                throw new IllegalStateException("boom");
            case "lookup":
                System.out.println(InetAddress.getByName("localhost").isLoopbackAddress());
                break;
            case "prop":
                System.out.println(hex(String.valueOf(System.getProperty(args[1]))));
                break;
            case "recurse":
                System.out.println(depth(Integer.parseInt(args[1])));
                break;
            case "first-thread":
                long pid = ProcessHandle.current().pid();
                System.out.println(System.getenv("JAVA_STARTED_ON_FIRST_THREAD_" + pid));
                break;
            case "thread":
                System.out.println(Thread.currentThread().getName());
                break;
        }
    }
}
"#;

const AGENT: &str = r#"
import java.lang.instrument.Instrumentation;

public class Agent {
    public static void premain(String args, Instrumentation inst) {
        System.setProperty("dgpuj.agent", "ran");
    }
}
"#;

struct Fixture {
    java_home: PathBuf,
    /// `Probe.class`, in a directory with a letter outside ASCII in its name.
    classes: PathBuf,
    agent: PathBuf,
}

fn hex(s: &str) -> String {
    s.chars()
        .map(|c| format!("{:x}", c as u32))
        .collect::<Vec<_>>()
        .join(" ")
}

fn tool(java_home: &Path, name: &str) -> PathBuf {
    java_home
        .join("bin")
        .join(format!("{name}{}", env::consts::EXE_SUFFIX))
}

fn run_tool(java_home: &Path, name: &str, dir: &Path, args: &[&str]) {
    let status = Command::new(tool(java_home, name))
        .current_dir(dir)
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("running {name}: {e}"));
    assert!(status.success(), "{name} {args:?} failed");
}

fn build_fixture() -> Option<Fixture> {
    let java_home = env::var_os("DGPUJ_TEST_JAVA_HOME")
        .or_else(|| env::var_os("JAVA_HOME"))
        .map(PathBuf::from)
        .filter(|home| tool(home, "javac").is_file());
    let Some(java_home) = java_home else {
        assert!(
            env::var_os("CI").is_none(),
            "no JDK: set DGPUJ_TEST_JAVA_HOME"
        );
        eprintln!("skipped: no JDK in DGPUJ_TEST_JAVA_HOME or JAVA_HOME");
        return None;
    };

    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("launch");
    let _ = fs::remove_dir_all(&root);
    // é is in cp1252, the code page of a CI runner, as well as in UTF-8.
    let classes = root.join("caf\u{e9} classes");
    fs::create_dir_all(&classes).unwrap();
    fs::write(root.join("Probe.java"), PROBE).unwrap();
    fs::write(root.join("Agent.java"), AGENT).unwrap();
    fs::write(root.join("agent.mf"), "Premain-Class: Agent\n").unwrap();
    run_tool(&java_home, "javac", &root, &["Probe.java", "Agent.java"]);
    run_tool(
        &java_home,
        "jar",
        &root,
        &["cfm", "agent.jar", "agent.mf", "Agent.class"],
    );
    fs::rename(root.join("Probe.class"), classes.join("Probe.class")).unwrap();

    Some(Fixture {
        java_home,
        classes,
        agent: root.join("agent.jar"),
    })
}

fn fixture() -> Option<&'static Fixture> {
    static FIXTURE: OnceLock<Option<Fixture>> = OnceLock::new();
    FIXTURE.get_or_init(build_fixture).as_ref()
}

fn dgpuj(f: &Fixture) -> Command {
    let mut command = Command::new(DGPUJ);
    command.arg("--dgpuj-home").arg(&f.java_home);
    command
}

/// `dgpuj [vm options] -cp <classes> Probe [args]`.
fn probe(f: &Fixture, vm: &[&str], args: &[&str]) -> Command {
    let mut command = dgpuj(f);
    command.args(vm).arg("-cp").arg(&f.classes).arg("Probe");
    command.args(args);
    command
}

fn stdout(mut command: Command) -> String {
    let out = command.output().unwrap();
    assert!(out.status.success(), "{}", describe(&out));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn describe(out: &Output) -> String {
    format!(
        "{}\n--- stdout\n{}\n--- stderr\n{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn hands_main_its_arguments() {
    let Some(f) = fixture() else { return };
    let out = stdout(probe(f, &[], &["echo", "two words", "caf\u{e9}", "--flag"]));
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines, [hex("two words"), hex("caf\u{e9}"), hex("--flag")]);
}

#[test]
fn exits_as_the_program_does() {
    let Some(f) = fixture() else { return };
    let status = probe(f, &[], &["exit", "7"]).status().unwrap();
    assert_eq!(status.code(), Some(7));

    let out = probe(f, &[], &["throw"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", describe(&out));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("boom"),
        "{}",
        describe(&out)
    );
}

#[test]
fn passes_a_property_outside_ascii() {
    let Some(f) = fixture() else { return };
    let out = stdout(probe(
        f,
        &["-Ddgpuj.test=caf\u{e9}"],
        &["prop", "dgpuj.test"],
    ));
    assert_eq!(out, hex("caf\u{e9}"));
}

#[test]
fn says_what_was_launched() {
    let Some(f) = fixture() else { return };
    let out = stdout(probe(f, &[], &["prop", "sun.java.command"]));
    assert_eq!(out, hex("Probe prop sun.java.command"));
}

#[test]
fn takes_module_options_as_java_does() {
    let Some(f) = fixture() else { return };
    let vm = [
        "--add-opens",
        "java.base/java.lang=ALL-UNNAMED",
        "--add-modules",
        "java.logging",
    ];
    assert_eq!(stdout(probe(f, &vm, &["echo", "x"])), hex("x"));
}

/// A `-javaagent` makes the VM load `instrument`, which wants `java` and
/// `jli` by bare name, before it has loaded its own. With another JDK on
/// `PATH`, Windows gave it that one's, and the first name lookup then failed
/// with `platform encoding not initialized`.
#[test]
fn resolves_a_name_under_an_agent_with_another_java_on_the_path() {
    let Some(f) = fixture() else { return };
    let agent = format!("-javaagent:{}", f.agent.display());

    let mut command = probe(f, &[&agent], &["lookup"]);
    if let Some(other) = env::var_os("DGPUJ_TEST_OTHER_JAVA_HOME") {
        let mut path = OsString::from(Path::new(&other).join("bin"));
        if let Some(rest) = env::var_os("PATH") {
            path.push(if cfg!(windows) { ";" } else { ":" });
            path.push(rest);
        }
        command.env("PATH", path);
    } else {
        assert!(
            env::var_os("CI").is_none(),
            "no second JDK: set DGPUJ_TEST_OTHER_JAVA_HOME"
        );
    }
    assert_eq!(stdout(command), "true");

    let out = stdout(probe(f, &[&agent], &["prop", "dgpuj.agent"]));
    assert_eq!(out, hex("ran"));
}

/// The first thread's stack is the executable's and cannot be asked for, so
/// this holds only where the VM has a thread of its own.
#[cfg(not(target_os = "macos"))]
#[test]
fn gives_main_the_stack_xss_asks_for() {
    let Some(f) = fixture() else { return };
    assert_eq!(
        stdout(probe(f, &["-Xss128m"], &["recurse", "200000"])),
        "200000"
    );
    assert_eq!(stdout(probe(f, &[], &["thread"])), "main");
}

/// `java` takes `-XstartOnFirstThread` itself and the VM refuses it, so a
/// launch line written for `java` stopped here. dgpuj is always on the first
/// thread, and says so the way LWJGL checks for it.
#[cfg(target_os = "macos")]
#[test]
fn starts_on_the_first_thread_and_says_so() {
    let Some(f) = fixture() else { return };
    let out = stdout(probe(f, &["-XstartOnFirstThread"], &["first-thread"]));
    assert_eq!(out, "1");
    assert_eq!(stdout(probe(f, &[], &["first-thread"])), "1");
}

/// The two things the Windows binary is for and must not need: the symbols a
/// GPU driver looks up in the executable, and no C runtime beside it.
#[cfg(windows)]
#[test]
fn exports_the_gpu_symbols_and_carries_its_runtime() {
    let exe = fs::read(DGPUJ).unwrap();
    let has = |needle: &str| {
        let needle = needle.to_ascii_lowercase();
        exe.windows(needle.len())
            .any(|w| w.to_ascii_lowercase() == needle.as_bytes())
    };
    assert!(has("NvOptimusEnablement"));
    assert!(has("AmdPowerXpressRequestHighPerformance"));
    assert!(!has("vcruntime140"));
}
