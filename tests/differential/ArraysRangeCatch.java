import java.util.Arrays;

public class ArraysRangeCatch {
    static String sortOrder(int[] values) {
        try {
            Arrays.sort(values, 3, 1);
            return "miss";
        } catch (IllegalArgumentException ex) {
            return "sort-order";
        }
    }

    static String fillBounds(int[] values) {
        try {
            Arrays.fill(values, -1, 2, 9);
            return "miss";
        } catch (ArrayIndexOutOfBoundsException ex) {
            return "fill-bounds";
        }
    }

    public static String run() {
        return sortOrder(new int[] { 1, 2, 3 }) + ","
            + fillBounds(new int[] { 1, 2, 3 });
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
