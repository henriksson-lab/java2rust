public class ArrayNegativeSizeCatch {
    static String runCase(int n) {
        try {
            int[] values = new int[n];
            return "len:" + values.length;
        } catch (NegativeArraySizeException ex) {
            return "caught";
        }
    }

    public static String run() {
        return runCase(-1) + "," + runCase(2);
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
