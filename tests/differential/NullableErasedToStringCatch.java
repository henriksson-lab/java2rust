public class NullableErasedToStringCatch {
    private static String direct() {
        Object value = null;
        try {
            return value.toString();
        } catch (NullPointerException ex) {
            return "npe";
        }
    }

    private static String nested() {
        Object value = null;
        try {
            return "x" + value.toString();
        } catch (NullPointerException ex) {
            return "npe";
        }
    }

    public static String run() {
        Object value = null;
        return direct() + "," + nested() + "," + "x" + value;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
