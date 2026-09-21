public class TryCatchFinallyTryReturnCatchFallthrough {
    public static String run() {
        return runOne("1") + "," + runOne("x") + "," + runAlwaysThrow();
    }

    private static String runOne(String input) {
        String out = "";
        try {
            Integer.parseInt(input);
            return "ok";
        } catch (NumberFormatException ex) {
            out = "caught";
        } finally {
            out += ":finally";
        }
        return out;
    }

    private static String runAlwaysThrow() {
        String out = "";
        try {
            throw new NumberFormatException("bad");
        } catch (NumberFormatException ex) {
            out = "caught";
        } finally {
            out += ":finally";
        }
        return out;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
