public class FinallyReturnOverride {
    static String f() {
        try {
            return "try";
        } finally {
            return "finally";
        }
    }

    public static String run() {
        return f();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
