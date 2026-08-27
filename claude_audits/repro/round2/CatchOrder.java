public class CatchOrder {
    static String classify(int n) {
        try {
            if (n == 0) throw new IllegalArgumentException("iae");
            if (n == 1) throw new IllegalStateException("ise");
            if (n == 2) { int[] a = new int[1]; return "" + a[5]; }
            if (n == 3) return "" + (10 / (n - 3));
            return "none";
        } catch (IllegalArgumentException e) {
            return "IAE:" + e.getMessage();
        } catch (RuntimeException e) {
            return "RTE:" + e.getMessage();
        }
    }
    public static String run() {
        return classify(0) + "," + classify(1) + "," + classify(2) + "," + classify(3) + "," + classify(9);
    }
    public static void main(String[] args) { System.out.println(run()); }
}
