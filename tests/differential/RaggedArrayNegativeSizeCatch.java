public class RaggedArrayNegativeSizeCatch {
    static String runCase(int n) {
        try {
            int[][] values = new int[n][];
            values[0] = new int[] { 7, 8 };
            return "rows:" + values.length + ",cols:" + values[0].length;
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
