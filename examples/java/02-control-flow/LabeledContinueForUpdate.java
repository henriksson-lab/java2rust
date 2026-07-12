package examples.control;

public class LabeledContinueForUpdate {
    public int scan(int[] left, int[] right) {
        int hits = 0;
        outer:
        for (int i = 0; i < left.length; i++) {
            for (int j = 0; j < right.length; j++) {
                if (left[i] < right[j]) {
                    continue outer;
                }
                hits++;
            }
        }
        return hits;
    }
}

