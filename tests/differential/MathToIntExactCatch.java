public class MathToIntExactCatch {
    static int exact(long value) {
        try {
            return Math.toIntExact(value);
        } catch (ArithmeticException ex) {
            return -7;
        }
    }

    public static String run() {
        return exact(12L) + "," + exact(2147483648L) + "," + exact(-3L);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
