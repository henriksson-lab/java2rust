package examples.objects;

public class FieldWriteReceiverMutation {
    private int total = 0;

    public int addAll(int[] values) {
        for (int i = 0; i < values.length; i++) {
            total += values[i];
        }
        return total;
    }
}
