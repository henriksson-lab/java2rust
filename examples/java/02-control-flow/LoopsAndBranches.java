package examples.control;

public class LoopsAndBranches {
    public int sumOddUntil(int[] values, int limit) {
        int total = 0;
        for (int i = 0; i < values.length; i++) {
            if (values[i] >= limit) {
                break;
            }
            if (values[i] % 2 == 0) {
                continue;
            }
            total += values[i];
        }
        return total;
    }
}

