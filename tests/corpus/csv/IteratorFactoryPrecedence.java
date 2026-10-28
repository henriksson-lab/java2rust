public class IteratorFactoryPrecedence {
    static class Spliterators {
        static int spliteratorUnknownSize(int value, int extra) { return value + extra; }
    }
    static class StreamSupport {
        static int stream(int value, boolean parallel) { return parallel ? 0 : value; }
    }
    static class Facade {
        int stream(int value, boolean parallel) { return parallel ? 0 : value + 1; }
    }
    static int run() {
        if (Spliterators.spliteratorUnknownSize(2, 3) != 5) return 1;
        if (StreamSupport.stream(5, false) != 5) return 2;
        Facade StreamSupport = new Facade();
        if (StreamSupport.stream(6, false) != 7) return 3;
        if (java.util.stream.StreamSupport.stream(
                java.util.Spliterators.spliteratorUnknownSize(
                    java.util.Arrays.asList("a").iterator(), 0), false).count() != 1) return 4;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
