public class FinallyBreakOverridesReturn {
    static String f() {
        for (int i = 0; i < 1; i++) {
            try {
                return "try";
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
