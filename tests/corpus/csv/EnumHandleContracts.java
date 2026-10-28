public class EnumHandleContracts {
    static class Cell {
        int number;
        synchronized void set(int value) { number = value; }
        synchronized int get() { return number; }
    }
    enum Choice {
        A(new Cell()), B(null);
        final Cell value;
        int seen;
        Choice(Cell value) { this.value = value; }
        Cell get() { return value; }
        int seen() { return seen; }
        void visit() { seen++; }
    }
    static Cell valueOf(String key) { return Choice.valueOf(key).get(); }
    static int run() {
        Cell first = Choice.A.get();
        first.set(7);
        if (Choice.A.get().get() != 7) return 1;
        if (Choice.B.get() != null) return 2;
        if (Choice.valueOf("A").get() != first) return 3;
        if (valueOf("A") != first) return 6;
        Choice.A.visit();
        if (Choice.A.seen() != 1 || Choice.B.seen() != 0) return 7;
        try { Choice.valueOf("missing").get(); return 4; }
        catch (IllegalArgumentException expected) { }
        try { Choice.valueOf((String)null).get(); return 5; }
        catch (NullPointerException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
