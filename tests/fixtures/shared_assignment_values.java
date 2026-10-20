class SharedAssignmentBox {
    boolean flag = true;
    int number;
    SharedAssignmentBox child = null;
    boolean change(boolean next) { return flag = next; }
    int changeNumber(int next) { return number = next; }
    int nested(int next) { return number = (number = next) + 1; }
    static int observe(SharedAssignmentBox box) { return box.number; }
    static int sum(int a, int b) { return a + b; }
    int callback() { return number = observe(this) + 1; }
    int argumentOrder(int next) { return sum(number = next, observe(this)); }
    int checked(int next) {
        if (next < 0) throw new IllegalStateException("assignment");
        return next;
    }
    int assignChecked(int next) { return number = checked(next); }
    boolean bind(SharedAssignmentBox other) { return (child = other) != null; }
    boolean clear() { return (child = null) == null; }
    class Reader {
        int calls;
        boolean read() { calls++; return flag; }
        int value() { calls++; return number; }
    }
    Reader reader() { return new Reader(); }
}
class SharedAssignmentRoot {
    static SharedAssignmentBox box;
}
class SharedAssignmentValues {
    static int run() {
        SharedAssignmentBox box = new SharedAssignmentBox();
        SharedAssignmentBox alias = box;
        if (box.change(false) || alias.flag) return 1;
        if (box.changeNumber(4) != 4 || alias.number != 4) return 2;
        if (box.nested(8) != 9 || alias.number != 9) return 3;
        if (box.callback() != 10 || alias.number != 10) return 4;
        if (box.argumentOrder(12) != 24 || alias.number != 12) return 5;
        if (!box.bind(alias) || box.child != alias) return 6;
        if (!box.clear() || box.child != null) return 7;
        SharedAssignmentBox.Reader reader = box.reader();
        SharedAssignmentBox.Reader readerAlias = reader;
        if (reader != readerAlias || reader.read() || reader.value() != 12) return 8;
        try { box.assignChecked(-1); return 9; }
        catch (IllegalStateException expected) { }
        if (alias.number != 12 || box.assignChecked(17) != 17 || alias.number != 17) return 10;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
