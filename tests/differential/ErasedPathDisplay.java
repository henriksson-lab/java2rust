import java.io.File;

public class ErasedPathDisplay {
    public static String run() {
        Object value = new File("abc").toPath();
        return "x" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
