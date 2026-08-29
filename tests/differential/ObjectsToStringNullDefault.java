import java.util.Objects;

public class ObjectsToStringNullDefault {
    static int count = 0;

    static String fallback() {
        count++;
        return "d" + count;
    }

    public static String run() {
        String present = "x";
        String missing = null;
        return Objects.toString(present, fallback())
            + ","
            + count
            + ","
            + Objects.toString(missing, fallback())
            + ","
            + count
            + ","
            + Objects.toString(missing);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
