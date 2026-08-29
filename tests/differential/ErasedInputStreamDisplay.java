public class ErasedInputStreamDisplay {
    public static String run() {
        Object value = System.in;
        String text = "x" + value;
        return text.startsWith("xjava.io.") ? "ok" : text;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
