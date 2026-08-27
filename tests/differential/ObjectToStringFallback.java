public class ObjectToStringFallback {
    public static String run() {
        Object value = new Object();
        return String.valueOf(value.toString().startsWith("java.lang.Object@"));
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
