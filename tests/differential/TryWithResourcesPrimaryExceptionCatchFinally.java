public class TryWithResourcesPrimaryExceptionCatchFinally {
    static String out = "";

    static class Resource implements AutoCloseable {
        public void close() {
            out = out + "r";
            throw new RuntimeException("close");
        }
    }

    static void fail() {
        out = out + "t";
        throw new RuntimeException("body");
    }

    public static String run() {
        out = "";
        try (Resource resource = new Resource()) {
            fail();
        } catch (RuntimeException ex) {
            out = out + "c:" + ex.getMessage();
        } finally {
            out = out + ":f";
        }
        return out;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
