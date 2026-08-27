public class TryWithResourcesBodyContinue {
    static String out = "";

    static class Resource implements AutoCloseable {
        public void close() {
            out = out + "r";
        }
    }

    public static String run() {
        out = "";
        for (int i = 0; i < 3; i++) {
            out = out + i;
            try (Resource resource = new Resource()) {
                out = out + "t";
                continue;
            } catch (RuntimeException ex) {
                out = out + "c";
            }
            out = out + "x";
        }
        return out;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
