public class ErasedInputStreamDisplay {
    public static String run() {
        Object value = System.in;
        String text = "x" + value;
        String direct = value.toString();
        return text.startsWith("xjava.io.") && direct.startsWith("java.io.") ? "ok" : text + "|" + direct;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
