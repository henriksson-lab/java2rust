import java.io.InputStream;

public class InputStreamReadBoundsCatch {
    static String probe(InputStream input, byte[] buffer, int off, int len) throws Exception {
        try {
            return Integer.toString(input.read(buffer, off, len));
        } catch (IndexOutOfBoundsException ex) {
            return "caught";
        }
    }

    public static String run() throws Exception {
        InputStream input = System.in;
        byte[] buffer = new byte[4];
        return probe(input, buffer, -1, 1)
            + ","
            + probe(input, buffer, 0, -1)
            + ","
            + probe(input, buffer, 1, 5);
    }

    public static void main(String[] args) throws Exception {
        System.out.println(run());
    }
}
