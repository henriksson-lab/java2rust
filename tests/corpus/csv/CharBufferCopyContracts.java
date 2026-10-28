import java.io.Reader;
import java.io.StringReader;
import java.nio.CharBuffer;

public class CharBufferCopyContracts {
    static int copy(Reader input, Appendable output, CharBuffer buffer) throws Exception {
        int count = 0;
        int n;
        while ((n = input.read(buffer)) != -1) {
            buffer.flip();
            output.append(buffer, 0, n);
            count += n;
        }
        return count;
    }
    static int run() throws Exception {
        try { CharBuffer.allocate(-1); return 1; }
        catch (IllegalArgumentException expected) { }
        CharBuffer buffer = CharBuffer.allocate(2);
        CharBuffer alias = buffer;
        Reader input = new StringReader("\ud800x\udc00\ud83d\ude00z");
        StringBuilder output = new StringBuilder();
        if (copy(input, output, buffer) != 6) return 2;
        if (!output.toString().equals("\ud800x\udc00\ud83d\ude00z")) return 3;
        if (alias.remaining() != 2) return 4;
        alias.flip();
        if (buffer.remaining() != 0) return 5;
        CharBuffer empty = CharBuffer.allocate(0);
        Reader closed = new StringReader("");
        closed.close();
        try { closed.read(empty); return 6; }
        catch (java.io.IOException expected) { }
        return 0;
    }
    public static void main(String[] args) throws Exception { System.out.println(run()); }
}
