public class TryWithResourcesCatchFinallyBreak {
    static String out = "";

    static class Resource implements AutoCloseable {
        public void close() {
            out = out + "r";
        }
    }

    static void fail() {
        throw new RuntimeException("x");
    }

    static String run() {
        out = "";
        for (int i = 0; i < 3; i++) {
            try (Resource resource = new Resource()) {
                out = out + "t";
                fail();
            } catch (RuntimeException ex) {
                out = out + "c";
                break;
            } finally {
                out = out + "f";
            }
            out = out + "x";
        }
        return out + "d";
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
