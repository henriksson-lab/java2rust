import java.util.Objects;

public class NullableObjectsEquals {
    public static String run() {
        String value = null;
        return "" + Objects.equals(value, "x");
    }

    public static void main(String[] args) { System.out.println(run()); }
}
