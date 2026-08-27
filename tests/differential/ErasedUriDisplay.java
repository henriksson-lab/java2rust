import java.net.URI;

public class ErasedUriDisplay {
    public static String run() throws Exception {
        Object o = new URI("abc/def");
        return "u=" + o + ",direct=" + o.toString();
    }

    public static void main(String[] args) throws Exception {
        System.out.println(run());
    }
}
