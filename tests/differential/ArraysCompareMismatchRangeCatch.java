import java.util.Arrays;

public class ArraysCompareMismatchRangeCatch {
    static String equalsOrder(int[] left, int[] right) {
        try {
            boolean out = Arrays.equals(left, 3, 1, right, 0, 1);
            return "miss" + out;
        } catch (IllegalArgumentException ex) {
            return "equals-order";
        }
    }

    static String equalsBounds(int[] left, int[] right) {
        try {
            boolean out = Arrays.equals(left, 0, 1, right, -1, 1);
            return "miss" + out;
        } catch (ArrayIndexOutOfBoundsException ex) {
            return "equals-bounds";
        }
    }

    static String compareOrder(int[] left, int[] right) {
        try {
            int out = Arrays.compare(left, 3, 1, right, 0, 1);
            return "miss" + out;
        } catch (IllegalArgumentException ex) {
            return "compare-order";
        }
    }

    static String compareBounds(int[] left, int[] right) {
        try {
            int out = Arrays.compare(left, 0, 1, right, -1, 1);
            return "miss" + out;
        } catch (ArrayIndexOutOfBoundsException ex) {
            return "compare-bounds";
        }
    }

    static String mismatchOrder(int[] left, int[] right) {
        try {
            int out = Arrays.mismatch(left, 3, 1, right, 0, 1);
            return "miss" + out;
        } catch (IllegalArgumentException ex) {
            return "mismatch-order";
        }
    }

    static String mismatchBounds(int[] left, int[] right) {
        try {
            int out = Arrays.mismatch(left, 0, 1, right, -1, 1);
            return "miss" + out;
        } catch (ArrayIndexOutOfBoundsException ex) {
            return "mismatch-bounds";
        }
    }

    static String ok(int[] left, int[] right) {
        try {
            return Arrays.equals(left, 1, 3, right, 1, 3) + ":"
                + Arrays.compare(left, 1, 3, right, 1, 3) + ":"
                + Arrays.mismatch(left, 1, 3, right, 1, 3);
        } catch (RuntimeException ex) {
            return "caught";
        }
    }

    public static String run() {
        int[] left = new int[] { 9, 1, 2, 8 };
        int[] right = new int[] { 7, 1, 3, 6 };
        return equalsOrder(left, right) + ","
            + equalsBounds(left, right) + ","
            + compareOrder(left, right) + ","
            + compareBounds(left, right) + ","
            + mismatchOrder(left, right) + ","
            + mismatchBounds(left, right) + ","
            + ok(left, right);
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
