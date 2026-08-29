public class TryWithResourcesCloseThrowFinally {
    static String out = "";

    static class Resource implements AutoCloseable {
        public void close() throws Exception {
            out = out + "c";
            throw new RuntimeException("close");
        }
    }

    public static String run() {
        out = "";
        try {
            inner();
        } catch (Exception e) {
            return out + ":close";
        }
        return out + ":done";
    }

    static void inner() throws Exception {
        try (Resource resource = new Resource()) {
            out = out + "t";
        } finally {
            out = out + "f";
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
