import java.util.Random;

public class ErasedRandomDisplay {
    public static String run() {
        Object value = new Random(1);
        String text = "x" + value;
        return text.startsWith("xjava.util.Random@") ? "ok" : text;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
