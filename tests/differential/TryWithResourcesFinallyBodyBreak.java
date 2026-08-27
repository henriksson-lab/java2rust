public class TryWithResourcesFinallyBodyBreak {
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
            try (Resource a = new Resource("a");
                 Resource b = new Resource("b")) {
                out = out + "t";
                if (i == 0) {
                    break;
                }
            } finally {
                out = out + "f";
            }
            out = out + "x";
        }
        return out + "d";
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
