class SharedCallBase {
    int value;
    class View {
        int read() { return value; }
    }
    View view() { return new View(); }
    int read() { return value; }
    int through() { return view().read(); }
    int through(int extra) { return view().read() + extra; }
    int through(boolean doubled) { return doubled ? 2 * view().read() : view().read(); }
    int checked(boolean fail) {
        if (fail) throw new IllegalStateException("inherited");
        return through();
    }
    void set(int next) { value = next; }
}
class SharedCallChild extends SharedCallBase {
    int evaluate() { return through(); }
    int explicit() { return this.through(); }
    int selected() { return through(3) + through(true); }
    int checkedCall(boolean fail) { return checked(fail); }
}
class InheritedSelfCalls {
    static int run() {
        SharedCallChild child = new SharedCallChild();
        SharedCallChild alias = child;
        child.set(7);
        if (child.evaluate() != 7 || alias.explicit() != 7) return 1;
        SharedCallBase.View view = child.view();
        alias.set(12);
        if (view.read() != 12 || child.evaluate() != 12) return 2;
        if (child.selected() != 39 || child.checkedCall(false) != 12) return 3;
        try { child.checkedCall(true); return 4; }
        catch (IllegalStateException expected) { }
        if (child.checked(false) != 12) return 5;
        try { alias.checked(true); return 6; }
        catch (IllegalStateException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
