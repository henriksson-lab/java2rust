import java.util.Arrays;

public class ArraysBinarySearchRangeCatch {
    static String order(int[] values) {
        try {
            int found = Arrays.binarySearch(values, 3, 1, 4);
            return "miss" + found;
        } catch (IllegalArgumentException ex) {
            return "order";
        }
    }

    static String bounds(int[] values) {
        try {
            int found = Arrays.binarySearch(values, -1, 1, 4);
            return "miss" + found;
        } catch (ArrayIndexOutOfBoundsException ex) {
            return "bounds";
        }
    }

    static String found(int[] values) {
        try {
            return String.valueOf(Arrays.binarySearch(values, 1, 4, 5));
        } catch (RuntimeException ex) {
            return "caught";
        }
    }

    public static String run() {
        int[] values = new int[] { 1, 3, 5, 7 };
        return order(values) + "," + bounds(values) + "," + found(values);
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
