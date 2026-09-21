import java.util.Locale;

public class StringFormatLocale {
    static String dynamic(String template, int value) {
        return String.format(Locale.US, template, value);
    }

    public static String run() {
        return String.format(Locale.US, "%s:%d", "x", 2) + ","
            + dynamic("n=%d", 3);
    }

    public static void main(String[] args) { System.out.println(run()); }
}
