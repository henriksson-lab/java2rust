class ConstructorSuperBase<T> {
    T stored;
    void set(T value) { stored = value; }
    T get() { return stored; }
    int initial() { return 3; }
}
class ConstructorSuperChild<U> extends ConstructorSuperBase<U> {
    int initialized = super.initial();
    ConstructorSuperChild(U value) { super.set(value); }
    @Override void set(U value) { throw new IllegalStateException("override"); }
    @Override int initial() { return 99; }
}
class ConstructorLexicalSuper {
    static int run() {
        ConstructorSuperChild<String> child = new ConstructorSuperChild<>("kept");
        if (!child.get().equals("kept") || child.initialized != 3) return 1;
        try { child.set("lost"); return 2; }
        catch (IllegalStateException expected) { }
        if (!child.get().equals("kept")) return 3;
        ConstructorSuperChild<String> nullable = new ConstructorSuperChild<>(null);
        if (nullable.get() != null || nullable.initialized != 3) return 4;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
