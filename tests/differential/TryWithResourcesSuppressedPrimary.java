public class TryWithResourcesSuppressedPrimary {
    static class Resource implements AutoCloseable {
        public void close() {
            throw new RuntimeException("close");
        }
    }

    static void fail() {
        throw new RuntimeException("body");
    }

    public static String run() {
        try (Resource r = new Resource()) {
            fail();
        } catch (RuntimeException e) {
            Throwable[] suppressed = e.getSuppressed();
            return e.getMessage() + ":" + suppressed.length + ":" + suppressed[0].getMessage();
        }
        return "ok";
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
