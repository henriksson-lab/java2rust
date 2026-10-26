import java.util.function.IntUnaryOperator;

public final class BoundExpressionReferenceProbe {
    static int created;
    static int factories;
    interface Step { int apply(int n); }
    static final class Counter {
        int value;
        Counter(int value) { created++; this.value = value; }
        Counter set(int next) { value = next; return this; }
        int next(int n) { value += n; return value; }
        int next(long n) { return -1000; }
    }
    static final class Holder {
        Counter counter;
        Holder(Counter c) { counter = c; }
        IntUnaryOperator bind() { return this.counter::next; }
    }
    static Counter make(int value) { factories++; return new Counter(value); }
    static Counter missing() { factories++; return null; }
    static IntUnaryOperator construct() { return new Counter(10)::next; }
    static IntUnaryOperator chain() { return make(1).set(20)::next; }
    static IntUnaryOperator cast(Counter counter) { return ((Counter) counter)::next; }
    static IntUnaryOperator divide(int n) { return new Counter(30 / n)::next; }
    static Step sourceCallback() { return new Counter(7)::next; }
    public static int run() {
        created = 0;
        factories = 0;
        IntUnaryOperator first = construct();
        if (created != 1 || first.applyAsInt(2) != 12 || first.applyAsInt(3) != 15) return 1;
        IntUnaryOperator second = chain();
        if (created != 2 || factories != 1 || second.applyAsInt(4) != 24 || second.applyAsInt(1) != 25) return 2;
        Counter shared = new Counter(5);
        IntUnaryOperator third = cast(shared);
        shared.value = 40;
        if (third.applyAsInt(2) != 42 || shared.value != 42) return 3;
        Holder holder = new Holder(shared);
        IntUnaryOperator fourth = holder.bind();
        holder.counter = new Counter(100);
        if (fourth.applyAsInt(3) != 45 || shared.value != 45 || holder.counter.value != 100) return 4;
        try { IntUnaryOperator unused = divide(0); return 5; }
        catch (ArithmeticException expected) { }
        if (created != 4) return 6;
        try { IntUnaryOperator unused = missing()::next; return 7; }
        catch (NullPointerException expected) { }
        if (factories != 2) return 8;
        Step source = sourceCallback();
        if (source.apply(1) != 8 || source.apply(2) != 10 || created != 5) return 9;
        IntUnaryOperator nativeReceiver = new String("cab")::indexOf;
        if (nativeReceiver.applyAsInt('a') != 1 || nativeReceiver.applyAsInt('z') != -1) return 10;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
