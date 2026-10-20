class GenericCallBase<T> {
    int value;
    class View {
        int read() { return value; }
    }
    View view() { return new View(); }
    int through(T tag, boolean fail) {
        if (fail) throw new IllegalStateException("generic inherited");
        return view().read();
    }
    void set(int next) { value = next; }
}
class GenericCallChild<U> extends GenericCallBase<U> {
    int evaluate(U tag, boolean fail) { return through(tag, fail); }
}
class InheritedGenericSelfCalls {
    static int run() {
        GenericCallChild<String> child = new GenericCallChild<String>();
        GenericCallChild<String> alias = child;
        child.set(7);
        if (child.evaluate("tag", false) != 7) return 1;
        alias.set(12);
        if (child.through("tag", false) != 12) return 2;
        try { child.evaluate("tag", true); return 3; }
        catch (IllegalStateException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
