interface StagedCounter {
    int get();
    int write(int first, int second);
}
class StagedCounterImpl implements StagedCounter {
    int value;
    StagedCounterImpl(int value) { this.value = value; }
    public int get() { return value; }
    public int write(int first, int second) {
        int old = value;
        value = first + second;
        return old * 100 + first * 10 + second;
    }
}
class StagedTrace {
    static int trace;
    static int mark(int value) { trace = trace * 10 + value; return value; }
}
class StagedOwner {
    StagedCounter current;
    int ticks;
    Integer nullable;
    int observe(int first, int second) { return first * 100 + second * 10 + ticks; }
    int mutate() { StagedTrace.mark(2); ticks++; return ticks; }
    int nestedSelf() { return observe(StagedTrace.mark(1), mutate()); }
    int argument(int digit) { StagedTrace.mark(digit); return current.get(); }
    int replace(int digit, StagedCounter next) {
        StagedTrace.mark(digit); current = next; return digit;
    }
    int fail() { StagedTrace.mark(1); throw new IllegalArgumentException("argument"); }
    StagedCounter selected() { StagedTrace.mark(5); return current; }
    int readWrite() { return current.write(current.get(), current.get()); }
    int replacement(StagedCounter next) { return current.write(replace(1, next), argument(2)); }
    int selectedReplacement(StagedCounter next) { return selected().write(replace(1, next), argument(2)); }
    int failure() { return current.write(fail(), argument(2)); }
    int unbox() { return current.write(nullable, argument(2)); }
}
class SourceCallStaging {
    static int run() {
        StagedOwner owner = new StagedOwner();
        StagedTrace.trace = 0;
        if (owner.nestedSelf() != 111 || StagedTrace.trace != 12) return 1;
        StagedCounter original = new StagedCounterImpl(3);
        StagedCounter next = new StagedCounterImpl(9);
        owner.current = original;
        if (owner.readWrite() != 333 || original.get() != 6) return 2;
        owner.current = new StagedCounterImpl(3);
        StagedCounter receiver = owner.current;
        StagedTrace.trace = 0;
        if (owner.replacement(next) != 319 || StagedTrace.trace != 12) return 3;
        if (receiver.get() != 10 || owner.current.get() != 9) return 4;
        owner.current = new StagedCounterImpl(3);
        receiver = owner.current;
        StagedTrace.trace = 0;
        if (owner.selectedReplacement(next) != 319 || StagedTrace.trace != 512) return 5;
        if (receiver.get() != 10 || owner.current.get() != 9) return 6;
        owner.current = null;
        StagedTrace.trace = 0;
        try { owner.replacement(next); return 7; }
        catch (NullPointerException expected) { }
        if (StagedTrace.trace != 12 || owner.current.get() != 9) return 8;
        owner.current = null;
        StagedTrace.trace = 0;
        try { owner.failure(); return 9; }
        catch (IllegalArgumentException expected) { }
        if (StagedTrace.trace != 1) return 10;
        owner.nullable = null;
        StagedTrace.trace = 0;
        try { owner.unbox(); return 11; }
        catch (NullPointerException expected) { }
        if (StagedTrace.trace != 0) return 12;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
