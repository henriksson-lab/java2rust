public class TextAssemblyValues {
    public static String text() { return "\ud800A\udc00"; }
    public static String copied(String value) { return new String(value); }
    public static String formatted(String value) { return String.format("pre:%s:post", value); }
}
