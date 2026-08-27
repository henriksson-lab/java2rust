import java.util.Arrays;

public class ArraysCopyOfCatch {
    public static String run() {
        byte[] values = new byte[] {1, 2};
        byte[] padded = Arrays.copyOf(values, 3);
        String normal = padded.length + ":" + padded[0] + ":" + padded[2];
        try {
            byte[] bad = Arrays.copyOf(values, -1);
            return "miss" + bad.length;
        } catch (NegativeArraySizeException ex) {
            return normal + ":caught";
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
