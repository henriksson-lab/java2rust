import java.util.Arrays;

public class ErasedArraysAsListNullDisplay {
    public static String run() {
        Object value = Arrays.asList("a", null, "c");
        return "x" + value + ":" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
