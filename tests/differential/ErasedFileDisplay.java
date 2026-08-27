import java.io.File;

public class ErasedFileDisplay {
    public static String run() {
        Object o = new File("abc.txt");
        return "o=" + o;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
