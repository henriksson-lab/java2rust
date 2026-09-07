import java.util.Locale;

public class ErasedLocaleDisplay {
    public static String run() {
        Object value = Locale.US;
        return "x" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
