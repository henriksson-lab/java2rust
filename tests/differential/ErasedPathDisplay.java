import java.io.File;

public class ErasedPathDisplay {
    public static String run() {
        Object value = new File("abc").toPath();
        return "x" + value;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
