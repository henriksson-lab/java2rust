import java.io.IOException;

class StagedConditionalPayload {
    int value;
    StagedConditionalPayload(int value) { this.value = value; }
}
public class FallibleStagedConditional {
    static int trace;
    StagedConditionalPayload payload;
    Object saved;
    FallibleStagedConditional(StagedConditionalPayload payload) { this.payload = payload; }
    StagedConditionalPayload read(boolean fail) throws IOException {
        trace = trace * 10 + 1;
        if (fail) throw new IOException("read");
        return payload;
    }
    static int after() { trace = trace * 10 + 2; return 7; }
    void accept(Object value, int marker) {
        trace = trace * 10 + 3;
        saved = value;
        if (marker != 7) throw new IllegalStateException();
    }
    void call(boolean present, boolean fail) throws IOException {
        accept(present ? read(fail) : null, after());
    }
    void inspect(Object value, int marker) {
        trace = trace * 10 + 3;
        if (value != null && value != payload) throw new IllegalStateException("identity");
        if (marker != 7) throw new IllegalStateException("marker");
    }
    void loan(boolean present, boolean fail) throws IOException {
        inspect(present ? read(fail) : null, after());
    }
    static int run() throws IOException {
        StagedConditionalPayload payload = new StagedConditionalPayload(4);
        FallibleStagedConditional target = new FallibleStagedConditional(payload);
        trace = 0;
        target.call(true, false);
        if (trace != 123 || target.saved != payload) return 1;
        trace = 0;
        target.call(false, true);
        if (trace != 23 || target.saved != null) return 2;
        trace = 0;
        try { target.call(true, true); return 3; } catch (IOException expected) {}
        if (trace != 1 || target.saved != null) return 4;
        trace = 0;
        target.loan(true, false);
        if (trace != 123) return 5;
        trace = 0;
        target.loan(false, true);
        if (trace != 23) return 6;
        trace = 0;
        try { target.loan(true, true); return 7; } catch (IOException expected) {}
        if (trace != 1) return 8;
        return 0;
    }
    public static void main(String[] args) throws IOException { System.out.println(run()); }
}
