public class CatchFinallyLabeledContinue {
    static void fail() {
        throw new RuntimeException("x");
    }

    public static String run() {
        String out = "";
        outer:
        for (int i = 0; i < 3; i++) {
            for (int j = 0; j < 3; j++) {
                out += i + "" + j;
                try {
                    fail();
                } catch (RuntimeException ex) {
                    out += "c";
                    continue outer;
                } finally {
                    out += "f";
                }
                out += "x";
            }
        }
        return out;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
