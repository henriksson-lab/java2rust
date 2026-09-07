import java.util.Random;

public class ErasedRandomDisplay {
    public static String run() {
        Object value = new Random(1);
        String text = "x" + value;
        String direct = value.toString();
        return text.startsWith("xjava.util.Random@") && direct.startsWith("java.util.Random@") ? "ok" : text + "|" + direct;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
