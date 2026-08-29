public class TryWithResourcesMultipleCloseSuppressed {
    static class Resource implements AutoCloseable {
        final String name;

        Resource(String name) {
            this.name = name;
        }

        public void close() {
            throw new RuntimeException(name);
        }
    }

    public static String run() {
        try (Resource a = new Resource("a"); Resource b = new Resource("b")) {
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
