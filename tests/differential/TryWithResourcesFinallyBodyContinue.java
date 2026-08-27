public class TryWithResourcesFinallyBodyContinue {
    static String out = "";

    static class Resource implements AutoCloseable {
        final String name;

        Resource(String name) {
            this.name = name;
        }

        public void close() {
            out = out + name;
        }
    }

    public static String run() {
        out = "";
        for (int i = 0; i < 3; i++) {
            out = out + i;
            try (Resource a = new Resource("a");
                 Resource b = new Resource("b")) {
                out = out + "t";
                if (i >= 0) {
                    continue;
                }
            } finally {
                out = out + "f";
            }
            out = out + "x";
        }
        return out;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
