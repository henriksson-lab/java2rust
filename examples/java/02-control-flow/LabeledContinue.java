package examples.control;

public class LabeledContinue {
    public int firstShared(int[] left, int[] right) {
        outer:
        for (int i = 0; i < left.length; i++) {
            for (int j = 0; j < right.length; j++) {
                if (left[i] != right[j]) {
                    continue outer;
                }
                return left[i];
            }
        }
        return -1;
    }
}

