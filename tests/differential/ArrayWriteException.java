public class ArrayWriteException {
    static String high() {
        int[] values = new int[] { 1, 2 };
        try {
            values[3] = 9;
            return "miss";
        } catch (ArrayIndexOutOfBoundsException ex) {
            return values[0] + ":high";
        }
    }

    static String negative() {
        int[] values = new int[] { 1, 2 };
        try {
            values[-1] = 9;
            return "miss";
        } catch (ArrayIndexOutOfBoundsException ex) {
            return values[1] + ":negative";
        }
    }

    public static String run() {
        return high() + "," + negative();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
