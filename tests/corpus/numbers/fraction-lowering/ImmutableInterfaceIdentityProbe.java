package immutableinterfaceidentity;

public class ImmutableInterfaceIdentityProbe {
    interface Combine<T> {
        T add(T other);
        T identity();
    }
    interface Arithmetic<T> extends Combine<T> {
        T subtract(T other);
    }
    static final class Value implements Arithmetic<Value> {
        final int n;
        Value(int n) { this.n = n; }
        public Value add(Value other) {
            if (other.n == 0) return this;
            return new Value(n + other.n);
        }
        public Value add(int n) { return add(new Value(n)); }
        public Value subtract(Value other) {
            if (other.n == 0) return this;
            return new Value(n - other.n);
        }
        public Value identity() { return this; }
    }
    static final class Local implements Combine<Local> {
        final int n;
        Local(int n) { this.n = n; }
        public Local add(Local other) {
            return other.n == 0 ? this : new Local(n + other.n);
        }
        public Local identity() { return this; }
    }
    static final Value ZERO = new Value(0);
    static <T extends Combine<T>> T combine(T a, T b) { return a.add(b); }
    public static int run() {
        Value value = new Value(10);
        Arithmetic<Value> arithmetic = value;
        Combine<Value> parent = arithmetic;
        if (arithmetic.add(ZERO) != value || arithmetic.subtract(ZERO) != value) return 1;
        if (parent.identity() != value || parent.add(ZERO) != value) return 2;
        if (combine(value, ZERO) != value || value.add(ZERO) != value) return 3;
        Value next = value.add(3);
        if (next.n != 13 || next == value || value.n != 10) return 4;
        if (arithmetic.add(new Value(2)).n != 12 || parent.add(new Value(2)).n != 12) return 5;
        if (arithmetic.subtract(new Value(3)).n != 7 || value.n != 10) return 6;
        try { arithmetic.add(null); return 7; }
        catch (NullPointerException expected) { }
        if (parent.identity() != value || arithmetic.add(ZERO) != value) return 8;
        Local local = new Local(4);
        Combine<Local> localView = local;
        if (localView.identity() != local || localView.add(new Local(0)) != local) return 9;
        Local added = combine(local, new Local(3));
        if (added.n != 7 || added == local || local.n != 4) return 10;
        return 0;
    }
    public static void main(String[] args) {
        int result = run();
        if (result != 0) throw new AssertionError("immutable interface contract: " + result);
        System.out.println(result);
    }
}
