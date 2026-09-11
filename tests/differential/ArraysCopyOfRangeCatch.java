import java.util.Arrays;

public class ArraysCopyOfRangeCatch {
    static String order(byte[] values) {
        try {
            byte[] out = Arrays.copyOfRange(values, 2, 1);
            return "miss" + out.length;
        } catch (IllegalArgumentException ex) {
            return "order";
        }
    }

    static String bounds(byte[] values) {
        try {
            byte[] out = Arrays.copyOfRange(values, -1, 1);
            return "miss" + out.length;
        } catch (ArrayIndexOutOfBoundsException ex) {
            return "bounds";
        }
    }

    static String ok(byte[] values) {
        byte[] out = Arrays.copyOfRange(values, 1, 4);
        return "ok:" + out.length + ":" + out[0] + ":" + out[2];
    }

    public static String run() {
        byte[] values = new byte[] { 1, 2, 3 };
        return order(values) + "," + bounds(values) + "," + ok(values);
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
