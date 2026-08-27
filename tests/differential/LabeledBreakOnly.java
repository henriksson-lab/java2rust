public class LabeledBreakOnly {
    public static String run() {
        int total = 0;
        outer:
        for (int i = 0; i < 3; i++) {
            for (int j = 0; j < 3; j++) {
                if (j == 2) break outer;
                total += 1;
            }
        }
        return "" + total;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
