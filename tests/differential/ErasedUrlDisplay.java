import java.net.URL;

public class ErasedUrlDisplay {
    public static String run() throws Exception {
        Object value = new URL("http://example.com/a");
        return "x" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) throws Exception {
        System.out.print(run());
    }
}
