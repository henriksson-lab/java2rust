package examples.types;

public class ArraysAndVarMutation {
    public void fill(int[] values, int start) {
        int current = start;
        for (int i = 0; i < values.length; i++) {
            values[i] = current;
            current++;
        }
    }
}

