public class TryWithResourcesCatchBreak {
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
            try (Resource resource = new Resource()) {
                fail();
            } catch (RuntimeException ex) {
                out += "c";
                break;
            }
            out += "x";
        }
        return out + "d";
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
