package storedcallback;
interface Window<T, R> { R apply(T values, int from, int to); }
class Metric {
    int total;
    int readTotal() { return total; }
    static Metric create(int[] values, int from, int to) {
        Metric metric = new Metric();
        for (int i = from; i < to; i++) metric.total += values[i];
        return metric;
    }
}
class FactoryHolder {
    Window<int[], Metric> metric = Metric::create;
    Window<int[], Metric> snapshot() { return metric; }
    Metric build(int[] values) { return metric.apply(values, 0, values.length); }
}
interface Reading { int read(); }
class Counter implements Reading {
    int count;
    boolean recurse;
    Reading hook;
    public int read() {
        count++;
        if (count == 4) throw new IllegalStateException("four");
        if (recurse) { recurse = false; return hook.read() + 100; }
        return count;
    }
}
public class StoredSourceCallbackAudit {
    static Reading retain(Reading reading) { return reading::read; }
    public static int run() {
        FactoryHolder holder = new FactoryHolder();
        if (holder.build(new int[] {2, 3, 4}).total != 9) return 1;
        Window<int[], Metric> copied = holder.snapshot();
        if (copied.apply(new int[] {10, 20, 30}, 1, 3).total != 50) return 2;
        Window<int[], Metric> bound = copied::apply;
        if (bound.apply(new int[] {7, 8, 9}, 0, 2).total != 15) return 3;
        Counter counter = new Counter();
        Reading source = counter;
        Reading retained = retain(source);
        counter.hook = retained;
        counter.recurse = true;
        if (retained.read() != 102 || counter.count != 2) return 4;
        if (source.read() != 3 || counter.count != 3) return 5;
        try { retained.read(); return 6; }
        catch (IllegalStateException e) { if (!"four".equals(e.getMessage())) return 7; }
        if (source.read() != 5 || counter.count != 5) return 8;
        try { bound.apply(new int[] {1}, 0, 2); return 9; }
        catch (ArrayIndexOutOfBoundsException e) { }
        if (bound.apply(new int[] {8}, 0, 1).total != 8) return 10;
        try { retain(null); return 11; }
        catch (NullPointerException e) { }
        Metric Metric = new Metric();
        Metric.total = 12;
        Reading direct = Metric::readTotal;
        if (direct.read() != 12) return 12;
        Metric.total = 13;
        if (direct.read() != 13) return 13;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
