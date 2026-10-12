import java.lang.instrument.Instrumentation;

/** A `-javaagent` that leaves a mark, for the launch that has to load one. */
public class Agent {
    public static void premain(String args, Instrumentation inst) {
        System.setProperty("dgpuj.agent", "ran");
    }
}
