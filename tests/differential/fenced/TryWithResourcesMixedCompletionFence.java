public class TryWithResourcesMixedCompletionFence {
    static class Resource implements AutoCloseable {
        public void close() {}
    }

    static String runOne(boolean fail) {
        String out = "";
        try (Resource resource = new Resource()) {
            if (fail) {
                throw new RuntimeException("bad");
            }
            return "ok";
        } catch (RuntimeException ex) {
            out = "caught";
        } finally {
            out += ":finally";
        }
        return out;
    }

    public static String run() {
        return runOne(false) + "," + runOne(true);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
