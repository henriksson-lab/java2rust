class Objects {
    static String toString(Object value, String fallback) { return "source"; }
}

public class SourceObjectsToStringShadow {
    static String sourceResult() { return Objects.toString(null, null); }

    public static String run() {
        String value = Objects.toString(null, null);
        return value + ":" + sourceResult() + ":"
            + new Objects().toString().startsWith("Objects@");
    }

    public static void main(String[] args) { System.out.println(run()); }
}
