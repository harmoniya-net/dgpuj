import java.lang.management.ManagementFactory;
import java.net.InetAddress;

/** What the tests launch through dgpuj: it says what it was given and where it is. */
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
            case "env":
                System.out.println(hex(String.valueOf(System.getenv(args[1]))));
                break;
            case "recurse":
                System.out.println(depth(Integer.parseInt(args[1])));
                break;
            case "first-thread":
                String self = ManagementFactory.getRuntimeMXBean().getName();
                String pid = self.substring(0, self.indexOf('@'));
                System.out.println(System.getenv("JAVA_STARTED_ON_FIRST_THREAD_" + pid));
                break;
            case "thread":
                System.out.println(Thread.currentThread().getName());
                break;
            default:
                throw new IllegalArgumentException(args[0]);
        }
    }
}
