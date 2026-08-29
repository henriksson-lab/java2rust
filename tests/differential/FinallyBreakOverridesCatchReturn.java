public class FinallyBreakOverridesCatchReturn {
    static void fail() {
        throw new IllegalArgumentException("bad");
    }

    static String f() {
        for (int i = 0; i < 1; i++) {
            try {
                fail();
            } catch (IllegalArgumentException ex) {
                return "catch";
            } finally {
                break;
            }
        }
        return "after";
    }

    public static String run() {
        return f();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
