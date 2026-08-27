public class TryWithResourcesFinally {
    static String out = "";

    static class Resource implements AutoCloseable {
        public void close() {
            out = out + "r";
        }
    }

    public static String run() {
        out = "";
        try (Resource resource = new Resource()) {
            out = out + "t";
        } finally {
            out = out + "f";
        }
        return out + "d";
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
