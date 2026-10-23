interface Counter<T> { void increment(); int count(); T itself(); }
class Cell implements Counter<Cell> {
    int value;
    public void increment() { value++; }
    public int count() { return value; }
    public Cell itself() { return this; }
}
class Probe {
    static int evaluations;
    static Cell observed(boolean fail) {
        evaluations++;
        if (fail) throw new IllegalStateException("identity operand");
        return new Cell();
    }
    static int eagerIdentity() {
        Cell absent = null;
        if (absent == observed(false)) return 10;
        if (observed(false) == absent) return 11;
        try { if (absent == observed(true)) return 12; return 13; }
        catch (IllegalStateException expected) { }
        try { if (observed(true) == absent) return 14; return 15; }
        catch (IllegalStateException expected) { }
        return evaluations == 4 ? 0 : 16;
    }
    static int check(boolean present) {
        Cell original = present ? new Cell() : null;
        Counter<Cell> alias = original;
        if (!present) return alias == null ? 0 : 1;
        alias.increment();
        if (original.count() != 1) return 2;
        original.increment();
        if (alias.count() != 2 || alias.itself() != original) return 3;
        return 0;
    }
    static int run() {
        int missing = check(false);
        if (missing != 0) return missing;
        int present = check(true);
        return present == 0 ? eagerIdentity() : present;
    }
    public static void main(String[] args) {
        int result = run();
        if (result != 0) throw new AssertionError(result);
        System.out.println(result);
    }
}
