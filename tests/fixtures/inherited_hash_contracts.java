class HashParent<T> {
    protected T value;
    HashParent(T value) { this.value = value; }
    public int hashCode() { return value == null ? 0 : value.hashCode(); }
}

class HashChild<U> extends HashParent<U> {
    HashChild(U value) { super(value); }
    public int hashCode() { return value == null ? 0 : 2 * value.hashCode(); }
}

class HashThrowingChild<U> extends HashParent<U> {
    HashThrowingChild(U value) { super(value); }
    public int hashCode() { throw new IllegalStateException("hash"); }
}

class HashUnguarded<T> {
    T value;
    HashUnguarded(T value) { this.value = value; }
    public int hashCode() { return value.hashCode(); }
}

class InheritedHashContracts {
    static <T> int guarded(T value) { return value == null ? 0 : value.hashCode(); }
    static int run() {
        HashParent<String> parent = new HashParent<>("a");
        HashParent<String> child = new HashChild<>("a");
        if (parent.hashCode() != 97 || child.hashCode() != 194) return 1;
        HashChild<String> empty = new HashChild<>(null);
        if (empty.hashCode() != 0 || guarded((String)null) != 0 || guarded("a") != 97) return 2;
        HashUnguarded<String> missing = new HashUnguarded<>(null);
        try { missing.hashCode(); return 3; }
        catch (NullPointerException expected) { }
        HashParent<String> failing = new HashThrowingChild<>("a");
        try { failing.hashCode(); return 4; }
        catch (IllegalStateException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
