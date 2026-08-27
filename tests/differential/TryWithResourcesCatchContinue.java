public class TryWithResourcesCatchContinue {
    static String out = "";

    static class Resource implements AutoCloseable {
        public void close() {
            out += "r";
        }
    }

    static void fail() {
        throw new RuntimeException("x");
    }

    public static String run() {
        out = "";
        for (int i = 0; i < 3; i++) {
            out += i;
            try (Resource resource = new Resource()) {
                fail();
            } catch (RuntimeException ex) {
                out += "c";
                continue;
            }
            out += "x";
        }
        return out;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
