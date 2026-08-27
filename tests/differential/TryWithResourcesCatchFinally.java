public class TryWithResourcesCatchFinally {
    static String out = "";

    static class Resource implements AutoCloseable {
        public void close() {
            out = out + "r";
        }
    }

    static void fail() {
        throw new RuntimeException("x");
    }

    public static String run() {
        out = "";
        try (Resource resource = new Resource()) {
            out = out + "t";
            fail();
        } catch (RuntimeException ex) {
            out = out + "c";
        } finally {
            out = out + "f";
        }
        return out + "d";
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
