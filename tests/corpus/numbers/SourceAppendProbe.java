package appendshadow;

class Value {
    public String toString() { throw new IllegalStateException("must not format"); }
}
class StringBuilder {
    int calls;
    StringBuilder append(Object value) { calls++; return this; }
}
public class SourceAppendProbe {
    static int run() {
        StringBuilder builder = new StringBuilder();
        Value value = new Value();
        if (builder.append(value) != builder || builder.calls != 1) return 1;
        try { new java.lang.StringBuilder().append(value); return 2; }
        catch (IllegalStateException error) {
            if (!error.getMessage().equals("must not format")) return 3;
        }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
