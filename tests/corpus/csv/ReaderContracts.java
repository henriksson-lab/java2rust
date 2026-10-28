import java.io.*;

class CountingReader extends BufferedReader {
    int calls;
    char[] retained;
    CountingReader(Reader input) { super(input); }
    @Override public int read() throws IOException { calls++; return super.read(); }
    int next() throws IOException { return read(); }
    int fill(char[] buffer, int offset, int length) throws IOException {
        retained = buffer;
        return super.read(buffer, offset, length);
    }
    String line() throws IOException {
        StringBuilder text = new StringBuilder();
        int ch;
        while ((ch = read()) >= 0 && ch != '\n') text.append((char)ch);
        return text.toString();
    }
}

public class ReaderContracts {
    static int copy(Reader input, Writer output, char[] buffer) throws IOException {
        int count = 0;
        int size;
        while ((size = input.read(buffer)) != -1) {
            output.write(buffer, 0, size);
            count += size;
        }
        return count;
    }
    static int run() throws Exception {
        Reader source = new StringReader("A\ud800\udc00Z\nb");
        CountingReader reader = new CountingReader(source);
        char[] buffer = new char[5];
        if (reader.fill(buffer, 1, 3) != 3) return 1;
        if (buffer[0] != 0 || buffer[1] != 'A' || buffer[2] != '\ud800' || buffer[3] != '\udc00' || buffer[4] != 0) return 2;
        if (reader.retained != buffer) return 3;
        if (reader.next() != 'Z' || reader.calls != 1) return 4;
        reader.mark(2);
        if (!reader.line().equals("") || reader.calls != 2) return 5;
        if (reader.next() != 'b' || reader.calls != 3) return 6;
        reader.reset();
        if (!reader.line().equals("") || reader.calls != 4) return 7;
        if (reader.fill(buffer, 0, 5) != 1 || buffer[0] != 'b') return 8;
        if (reader.fill(buffer, 0, 5) != -1) return 9;
        if (reader.fill(buffer, 0, 0) != 0) return 10;
        try { reader.fill(null, 0, 1); return 11; }
        catch (NullPointerException expected) { }
        try { reader.fill(buffer, -1, 1); return 12; }
        catch (IndexOutOfBoundsException expected) { }
        reader.close();
        try { reader.fill(buffer, 0, 0); return 13; }
        catch (IOException expected) { }
        Reader raw = new StringReader("\ud800q");
        char[] units = new char[2];
        if (raw.read(units) != 2 || units[0] != '\ud800' || units[1] != 'q') return 14;
        try { new CountingReader(null); return 15; }
        catch (NullPointerException expected) { }
        StringWriter output = new StringWriter();
        if (copy(new StringReader("\ud800x\udc00"), output, new char[2]) != 3) return 16;
        if (!output.toString().equals("\ud800x\udc00")) return 17;
        return 0;
    }
    public static void main(String[] args) throws Exception { System.out.println(run()); }
}
