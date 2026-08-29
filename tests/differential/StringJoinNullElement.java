import java.util.Arrays;

public class StringJoinNullElement {
    public static String run() {
        return String.join(",", Arrays.asList("a", null, "b"));
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
