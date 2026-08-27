public class CatchFinallyReturnOverride {
    static void fail() {
        throw new RuntimeException("x");
    }

    static String f(boolean throwFirst) {
        try {
            if (throwFirst) {
                fail();
            }
            return "try";
        } catch (RuntimeException e) {
            return "catch";
        } finally {
            return "finally";
        }
    }

    public static String run() {
        return f(false) + "," + f(true);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
