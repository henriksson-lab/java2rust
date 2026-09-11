import java.util.Arrays;

public class ArraysStreamRangeCatch {
    static int sum(int[] values, int from, int to) {
        try {
            return Arrays.stream(values, from, to).sum();
        } catch (IllegalArgumentException ex) {
            return -1;
        } catch (ArrayIndexOutOfBoundsException ex) {
            return -2;
        }
    }

    public static String run() {
        int[] values = new int[] { 1, 2, 3, 4 };
        return sum(values, 1, 3) + ","
            + sum(values, 3, 1) + ","
            + sum(values, -1, 2) + ","
            + sum(values, 1, 5);
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
