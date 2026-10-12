import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.jar.Attributes;
import java.util.jar.JarEntry;
import java.util.jar.JarOutputStream;
import java.util.jar.Manifest;

/**
 * dgpuj against a real JDK: the launches a `java` would make, and the ones
 * that went wrong under a dgpuj and not under `java`.
 *
 * <pre>java Tests &lt;dgpuj&gt; &lt;directory of the compiled tests&gt;</pre>
 *
 * The JDK launched is the one this runs on. `OTHER_JAVA_HOME` is a second
 * one, a stranger to put on PATH. Written for Java 8, the oldest it tests.
 */
public class Tests {
    static final String OS = System.getProperty("os.name").toLowerCase();
    static final boolean WINDOWS = OS.startsWith("windows");
    static final boolean MAC = OS.startsWith("mac");
    static final boolean LINUX = OS.startsWith("linux");
    static final boolean CI = System.getenv("CI") != null;
    static final String E_ACUTE = "café";

    /** `java.home` is a Java 8 JDK's `jre`; the JDK itself is what holds `bin/javac`. */
    static final File JAVA_HOME = jdk();
    static final boolean JAVA_8 = System.getProperty("java.version").startsWith("1.");
    /** Java 8 on Windows finds itself from its launcher's place, so dgpuj is put there. */
    static final boolean BESIDE = WINDOWS && JAVA_8;

    static File dgpuj;
    /** The word dgpuj is given for `java`: a path, or the bare name where it is beside one. */
    static String java;
    /** `Probe.class`, in a directory with a letter outside ASCII in its name. */
    static File classes;
    static File agentJar;
    /** `Probe`, as a jar that names it as its main class, alone in a directory. */
    static File probeJar;

    interface Test {
        void run() throws Exception;
    }

    static int failed;

    static void test(String name, boolean applies, Test test) {
        if (!applies) {
            System.out.println("skip  " + name);
            return;
        }
        try {
            test.run();
            System.out.println("ok    " + name);
        } catch (Throwable e) {
            failed++;
            System.out.println("FAIL  " + name);
            System.out.println("      " + String.valueOf(e.getMessage()).replace("\n", "\n      "));
        }
    }

    public static void main(String[] args) throws Exception {
        File built = new File(args[1]).getAbsoluteFile();
        setUp(new File(args[0]).getAbsoluteFile(), built);

        test("hands main its arguments", true, () -> {
            String out = probe(none(), "echo", "two words", E_ACUTE, "--flag").stdout();
            same(hex("two words") + "\n" + hex(E_ACUTE) + "\n" + hex("--flag"), out);
        });

        test("exits as the program does", true, () -> {
            same(7, probe(none(), "exit", "7").run().code);
            Result thrown = probe(none(), "throw").run();
            same(1, thrown.code);
            check(thrown.err.contains("boom"), thrown.toString());
        });

        test("passes a property outside ASCII", true, () -> {
            String out = probe(list("-Ddgpuj.test=" + E_ACUTE), "prop", "dgpuj.test").stdout();
            same(hex(E_ACUTE), out);
        });

        test("says what was launched", true, () -> {
            String out = probe(none(), "prop", "sun.java.command").stdout();
            same(hex("Probe prop sun.java.command"), out);
        });

        // Modules are Java 9's, and so are their options.
        test("takes module options as two words", !JAVA_8, () -> {
            List<String> vm = list(
                "--add-opens", "java.base/java.lang=ALL-UNNAMED", "--add-modules", "java.logging");
            same(hex("x"), probe(vm, "echo", "x").stdout());
        });

        // A `-javaagent` makes the VM load `instrument`, which wants `java` and
        // `jli` by bare name, before it has loaded its own. With another JDK
        // on PATH, Windows gave it that one's, and the first name lookup then
        // failed with "platform encoding not initialized".
        test("resolves a name under an agent with another java on PATH", true, () -> {
            String other = System.getenv("OTHER_JAVA_HOME");
            check(other != null || !CI, "no second JDK: set OTHER_JAVA_HOME");
            List<String> vm = list("-javaagent:" + agentJar);
            Launch lookup = probe(vm, "lookup");
            if (other != null) {
                lookup.pathFirst(new File(other, "bin"));
            }
            same("true", lookup.stdout());
            same(hex("ran"), probe(vm, "prop", "dgpuj.agent").stdout());
        });

        test("gives main the stack -Xss asks for", true, () -> {
            same("200000", probe(list("-Xss128m"), "recurse", "200000").stdout());
            same("main", probe(none(), "thread").stdout());
        });

        test("launches a jar and a wildcard class path", true, () -> {
            same(hex("x"), dgpuj(java, "-jar", probeJar.getPath(), "echo", "x").stdout());
            String jars = new File(probeJar.getParentFile(), "*").getPath();
            same(hex("y"), dgpuj(java, "-cp", jars, "Probe", "echo", "y").stdout());
        });

        // A launch line for macOS says `-XstartOnFirstThread`, which is the
        // launcher's own: a VM handed it refuses it as an option it does not know.
        test("starts on the first thread when asked", MAC, () -> {
            same("1", probe(list("-XstartOnFirstThread"), "first-thread").stdout());
        });

        // `dgpuj java ...`: the bare word is the `java` on PATH, as it is to a
        // shell. Not for Java 8 on Windows, which PATH cannot find.
        test("is the java on PATH when given the bare word", !BESIDE, () -> {
            Launch launch = dgpuj("java", "-cp", classes.getPath(), "Probe", "prop", "java.home");
            launch.pathFirst(new File(JAVA_HOME, "bin"));
            samePath(hex(System.getProperty("java.home")), launch.stdout());
        });

        // Installed in a JDK's `bin`, dgpuj is that JDK's, as `java.exe` is,
        // whatever PATH would have said.
        test("is the java it is installed beside", WINDOWS, () -> {
            File copy = new File(new File(JAVA_HOME, "bin"), "dgpuj-installed.exe");
            Files.copy(dgpuj.toPath(), copy.toPath(), StandardCopyOption.REPLACE_EXISTING);
            try {
                Launch launch = new Launch(list(
                    copy.getPath(), "java", "-cp", classes.getPath(), "Probe", "prop", "java.home"));
                String other = System.getenv("OTHER_JAVA_HOME");
                if (other != null) {
                    launch.pathFirst(new File(other, "bin"));
                }
                samePath(hex(System.getProperty("java.home")), launch.stdout());
            } finally {
                copy.delete();
            }
        });

        test("takes a path to java however it is written", WINDOWS && !BESIDE, () -> {
            String slashed = JAVA_HOME.getPath().replace('\\', '/') + "/bin/java";
            same(hex("w"), dgpuj(slashed, "-cp", classes.getPath(), "Probe", "echo", "w").stdout());
        });

        // What the Windows binary is for and must not need: the symbols a GPU
        // driver looks up in the executable, and no C runtime beside it.
        test("exports the GPU symbols and needs no runtime", WINDOWS, () -> {
            String exe = new String(Files.readAllBytes(dgpuj.toPath()), StandardCharsets.ISO_8859_1);
            check(exe.contains("NvOptimusEnablement"), "no NvOptimusEnablement");
            check(exe.contains("AmdPowerXpressRequestHighPerformance"), "no Amd symbol");
            check(!exe.toLowerCase().contains("vcruntime"), "needs a C runtime");
        });

        // One TCP segment over Ethernet: 1500, less 20 of IP and 20 of TCP.
        // On ARM an image has to carry its relocations, and does not fit.
        boolean x64 = System.getProperty("os.arch").equals("amd64");
        test("fits in one TCP packet", WINDOWS && x64, () -> {
            check(dgpuj.length() <= 1460, dgpuj.length() + " bytes");
        });

        // The PRIME variables are set only where the NVIDIA driver is loaded,
        // and never over what the caller chose.
        test("leaves a GPU variable the caller set", LINUX, () -> {
            Launch launch = probe(none(), "env", "__NV_PRIME_RENDER_OFFLOAD");
            launch.env("__NV_PRIME_RENDER_OFFLOAD", "0");
            same(hex("0"), launch.stdout());
            boolean nvidia = new File("/proc/driver/nvidia/version").exists();
            String set = probe(none(), "env", "__GLX_VENDOR_LIBRARY_NAME").stdout();
            same(hex(nvidia ? "nvidia" : "null"), set);
        });

        test("says how it is used when given nothing", !WINDOWS, () -> {
            Result nothing = new Launch(list(dgpuj.getPath())).run();
            check(nothing.code != 0 && nothing.err.contains("usage"), nothing.toString());
        });

        if (failed > 0) {
            System.out.println(failed + " failed");
            System.exit(1);
        }
    }

    static File jdk() {
        File home = new File(System.getProperty("java.home"));
        return new File(home, "bin/javac").isFile() || new File(home, "bin/javac.exe").isFile()
            ? home
            : home.getParentFile();
    }

    static void setUp(File binary, File built) throws IOException {
        File root = new File(built, "launch");
        classes = new File(root, E_ACUTE + " classes");
        File jars = new File(root, "jars");
        check(classes.mkdirs() && jars.mkdirs(), "cannot make " + root);
        Files.copy(new File(built, "Probe.class").toPath(), new File(classes, "Probe.class").toPath());

        agentJar = new File(root, "agent.jar");
        jar(agentJar, new Attributes.Name("Premain-Class"), "Agent", new File(built, "Agent.class"));
        probeJar = new File(jars, "probe.jar");
        jar(probeJar, Attributes.Name.MAIN_CLASS, "Probe", new File(built, "Probe.class"));

        String exe = WINDOWS ? ".exe" : "";
        if (BESIDE) {
            dgpuj = new File(new File(JAVA_HOME, "bin"), "dgpuj-under-test.exe");
            Files.copy(binary.toPath(), dgpuj.toPath(), StandardCopyOption.REPLACE_EXISTING);
            dgpuj.deleteOnExit();
            java = "java";
        } else {
            dgpuj = binary;
            java = new File(JAVA_HOME, "bin/java" + exe).getPath();
        }
    }

    static void jar(File to, Attributes.Name key, String value, File classFile) throws IOException {
        Manifest manifest = new Manifest();
        manifest.getMainAttributes().put(Attributes.Name.MANIFEST_VERSION, "1.0");
        manifest.getMainAttributes().put(key, value);
        try (JarOutputStream out = new JarOutputStream(new FileOutputStream(to), manifest)) {
            out.putNextEntry(new JarEntry(classFile.getName()));
            out.write(Files.readAllBytes(classFile.toPath()));
            out.closeEntry();
        }
    }

    /** `dgpuj <java> [vm options] -cp <classes> Probe [args]`. */
    static Launch probe(List<String> vm, String... args) {
        List<String> command = list(dgpuj.getPath(), java);
        command.addAll(vm);
        command.addAll(list("-cp", classes.getPath(), "Probe"));
        command.addAll(Arrays.asList(args));
        return new Launch(command);
    }

    static Launch dgpuj(String... args) {
        List<String> command = list(dgpuj.getPath());
        command.addAll(Arrays.asList(args));
        return new Launch(command);
    }

    static class Launch {
        final ProcessBuilder builder;

        Launch(List<String> command) {
            builder = new ProcessBuilder(command);
            // Not a JDK to fall back on: a launch finds it as the test says, or fails.
            builder.environment().remove("JAVA_HOME");
        }

        void env(String key, String value) {
            builder.environment().put(key, value);
        }

        /** Put a directory ahead of everything on PATH, whatever case Windows spells it in. */
        void pathFirst(File dir) {
            Map<String, String> env = builder.environment();
            String key = "PATH";
            for (String name : env.keySet()) {
                if (name.equalsIgnoreCase("PATH")) {
                    key = name;
                }
            }
            String rest = env.get(key);
            env.put(key, dir.getPath() + (rest == null ? "" : File.pathSeparator + rest));
        }

        Result run() throws Exception {
            // Into files, so that neither stream can fill a pipe nobody is reading.
            File out = File.createTempFile("dgpuj", ".out");
            File err = File.createTempFile("dgpuj", ".err");
            try {
                builder.redirectOutput(out).redirectError(err);
                int code = builder.start().waitFor();
                return new Result(code, read(out), read(err), builder.command());
            } finally {
                out.delete();
                err.delete();
            }
        }

        /** What it printed, having succeeded. */
        String stdout() throws Exception {
            Result result = run();
            check(result.code == 0, result.toString());
            return result.out.trim().replace("\r\n", "\n");
        }
    }

    static class Result {
        final int code;
        final String out;
        final String err;
        final List<String> command;

        Result(int code, String out, String err, List<String> command) {
            this.code = code;
            this.out = out;
            this.err = err;
            this.command = command;
        }

        @Override
        public String toString() {
            return command + "\nexit code " + code + " (0x" + Integer.toHexString(code) + ")"
                + "\n--- stdout\n" + out + "\n--- stderr\n" + err;
        }
    }

    static String read(File file) throws IOException {
        return new String(Files.readAllBytes(file.toPath()), StandardCharsets.ISO_8859_1);
    }

    static String hex(String s) {
        StringBuilder out = new StringBuilder();
        s.codePoints().forEach(c -> out.append(Integer.toHexString(c)).append(' '));
        return out.toString().trim();
    }

    static List<String> list(String... items) {
        return new ArrayList<>(Arrays.asList(items));
    }

    static List<String> none() {
        return new ArrayList<>();
    }

    static void check(boolean holds, String otherwise) {
        if (!holds) {
            throw new AssertionError(otherwise);
        }
    }

    static void same(Object expected, Object actual) {
        check(expected.equals(actual), "expected " + expected + "\n but got " + actual);
    }

    /** As `same`, for paths spelled in hex: Windows does not mind the case of a letter. */
    static void samePath(String expected, String actual) {
        if (WINDOWS) {
            same(unhex(expected).toLowerCase(), unhex(actual).toLowerCase());
        } else {
            same(expected, actual);
        }
    }

    static String unhex(String hex) {
        StringBuilder out = new StringBuilder();
        for (String point : hex.split(" ")) {
            out.appendCodePoint(Integer.parseInt(point, 16));
        }
        return out.toString();
    }
}
