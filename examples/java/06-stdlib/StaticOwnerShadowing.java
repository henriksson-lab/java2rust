package examples.stdlib;

public class StaticOwnerShadowing {
    static class Objects {
        static boolean equals(String left, String right) {
            return false;
        }
    }

    static class Collections {
        static int emptyList() {
            return 42;
        }
    }

    static class Integer {
        static int max(int left, int right) {
            return left - right;
        }
    }

    public boolean same(String left, String right) {
        return Objects.equals(left, right);
    }

    public int count() {
        return Collections.emptyList();
    }

    public int max(int left, int right) {
        return Integer.max(left, right);
    }
}

