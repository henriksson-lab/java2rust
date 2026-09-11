public class TryWithResourcesCatchFinallyBodyBreak {
    static String out = "";

    static class Resource implements AutoCloseable {
        public void close() {
            out = out + "r";
        }
    }

    static String run() {
        out = "";
        for (int i = 0; i < 3; i++) {
            try (Resource resource = new Resource()) {
                out = out + "t";
                break;
            } catch (RuntimeException ex) {
                out = out + "c";
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
