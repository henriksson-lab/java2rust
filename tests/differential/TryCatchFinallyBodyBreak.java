public class TryCatchFinallyBodyBreak {
    public static String run() {
        String out = "";
        for (int i = 0; i < 3; i++) {
            try {
                out = out + "t";
                break;
            } catch (RuntimeException ex) {
                out = out + "c";
            } finally {
                out = out + "f";
            }
            out = out + "x";
        }
        return out + "d";
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
