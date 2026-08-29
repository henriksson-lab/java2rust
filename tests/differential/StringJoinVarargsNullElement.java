public class StringJoinVarargsNullElement {
    public static String run() {
        return String.join(",", "a", null, "b");
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
