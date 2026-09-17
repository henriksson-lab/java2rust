import java.util.StringTokenizer;

public class ErasedStringTokenizerDisplay {
    public static String run() {
        Object value = new StringTokenizer("a b");
        String text = "x" + value;
        String direct = value.toString();
        return text.startsWith("xjava.util.StringTokenizer@") && direct.startsWith("java.util.StringTokenizer@") ? "ok" : text + "|" + direct;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
