public class LabeledContinue {
    public int run(int n) {
        int total = 0;
        outer:
        for (int i = 0; i < n; i++) {
            for (int j = 0; j < n; j++) {
                if (j == 1) continue outer;
                total += 1;
            }
        }
        return total;
    }
}
