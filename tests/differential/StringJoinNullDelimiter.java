public class StringJoinNullDelimiter {
    public static String run() {
        try {
            return String.join(null, "a", "b");
        } catch (NullPointerException e) {
            return "npe";
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
