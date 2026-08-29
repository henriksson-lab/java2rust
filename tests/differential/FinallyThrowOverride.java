public class FinallyThrowOverride {
    static String value() {
        try {
            return "try";
        } finally {
            throw new RuntimeException("finally");
        }
    }

    public static String run() {
        try {
            return value();
        } catch (RuntimeException ex) {
            return ex.getMessage();
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
