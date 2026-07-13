package examples.control;

public class LabeledBreak {
    public int firstLimit(int[] values, int limit) {
        int found = -1;
        outer:
        for (int i = 0; i < values.length; i++) {
            if (values[i] > limit) {
                found = values[i];
                break outer;
            }
        }
        return found;
    }
}
