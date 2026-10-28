import java.io.BufferedReader;
import java.io.IOException;
import java.io.Reader;
import java.io.StringReader;

class SuperResultReader extends BufferedReader {
    int completed;
    SuperResultReader(Reader input) { super(input); }
    void markDiscarded(int limit) throws IOException { super.mark(limit); completed++; }
    void readDiscarded(char[] buffer) throws IOException { super.read(buffer, 0, buffer.length); completed++; }
    void resetDiscarded() throws IOException { super.reset(); completed++; }
    void closeDiscarded() throws IOException { super.close(); completed++; }
    boolean supported() { return super.markSupported(); }
}

class PlainResultBase {
    int calls;
    int value() { calls++; return calls; }
    void signal() { calls++; }
}
class PlainResultChild extends PlainResultBase {
    int exercise() { super.value(); super.signal(); return calls; }
}

public class DiscardedSuperResultContracts {
    static int run() throws IOException {
        SuperResultReader reader = new SuperResultReader(new StringReader("ab"));
        SuperResultReader shared = reader;
        if (!shared.supported()) return 1;
        try { shared.resetDiscarded(); return 2; } catch (IOException expected) {}
        if (shared.completed != 0) return 3;
        shared.markDiscarded(2);
        char[] buffer = new char[1];
        shared.readDiscarded(buffer);
        if (buffer[0] != 'a' || shared.completed != 2) return 4;
        shared.resetDiscarded();
        shared.readDiscarded(buffer);
        if (buffer[0] != 'a' || shared.completed != 4) return 5;
        shared.closeDiscarded();
        try { shared.markDiscarded(1); return 6; } catch (IOException expected) {}
        try { shared.readDiscarded(buffer); return 7; } catch (IOException expected) {}
        try { shared.resetDiscarded(); return 8; } catch (IOException expected) {}
        if (reader.completed != 5 || shared.completed != 5) return 9;
        if (new PlainResultChild().exercise() != 2) return 10;
        if (!shared.supported()) return 11;
        try { shared.markDiscarded(-1); return 12; } catch (IllegalArgumentException expected) {}
        if (shared.completed != 5) return 13;
        Reader raw = new StringReader("r");
        if (raw.read() != 'r') return 14;
        raw.reset();
        if (raw.read() != 'r') return 15;
        return 0;
    }
    public static void main(String[] args) throws Exception { System.out.println(run()); }
}
