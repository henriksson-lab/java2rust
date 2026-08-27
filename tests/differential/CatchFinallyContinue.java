public class CatchFinallyContinue {
    static void fail() {
        throw new RuntimeException("x");
    }

    public static String run() {
        String out = "";
        for (int i = 0; i < 3; i++) {
            out += i;
            try {
                fail();
            } catch (RuntimeException ex) {
                out += "c";
                continue;
            } finally {
                out += "f";
            }
            out += "x";
        }
        return out;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
