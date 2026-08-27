public class CatchFinallyBreak {
    static void fail() {
        throw new RuntimeException("x");
    }

    public static String run() {
        String out = "";
        for (int i = 0; i < 3; i++) {
            try {
                fail();
            } catch (RuntimeException ex) {
                out += "c";
                break;
            } finally {
                out += "f";
            }
            out += "x";
        }
        return out + "d";
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
