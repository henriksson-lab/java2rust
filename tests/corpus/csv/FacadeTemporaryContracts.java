public class FacadeTemporaryContracts {
    static int calls;
    static FacadeTemporaryContracts held;
    static FacadeTemporaryContracts retainOwner(FacadeTemporaryContracts value) { held = value; return value; }
    String text;
    int count;
    String read() { count++; return text; }
    int accept(CharSequence sequence, int marker) { return consume(sequence, marker); }
    synchronized int fromField() {
        if (text != null) return consume(text, later());
        return 0;
    }
    String saved;
    void save(String value) { saved = value; }
    int fromPlatform() { save(System.lineSeparator()); return saved.length(); }
    static String produce(boolean present) {
        calls++;
        return present ? "\ud800" : null;
    }
    static int later() { return calls * 10; }
    private static int consume(CharSequence sequence, int marker) {
        if (sequence == null) return marker;
        return sequence.length() + marker;
    }
    static int run() {
        if (consume(produce(true), later()) != 11) return 1;
        if (consume(produce(false), later()) != 20) return 2;
        if (calls != 2) return 3;
        FacadeTemporaryContracts owner = retainOwner(new FacadeTemporaryContracts());
        owner.text = "\ud800";
        if (owner.accept(owner.read(), owner.count * 10) != 11) return 4;
        if (owner.count != 1) return 5;
        owner.text = null;
        if (owner.accept(owner.read(), owner.count * 10) != 20) return 6;
        if (owner.count != 2) return 7;
        if (owner.fromField() != 0) return 8;
        owner.text = "\ud800";
        if (owner.fromField() != 21) return 9;
        if (owner.fromPlatform() < 1) return 10;
        if (System.lineSeparator() != System.lineSeparator()) return 11;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
