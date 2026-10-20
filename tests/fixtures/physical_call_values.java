import java.util.*;

interface ValueListener { void ping(int value); }
class ValueListenerImpl implements ValueListener {
    int total;
    public void ping(int value) { total += value; }
}
class ClearingValueListener implements ValueListener {
    PhysicalValueOwner owner;
    ClearingValueListener(PhysicalValueOwner owner) { this.owner = owner; }
    public void ping(int value) { owner.listeners.clear(); }
}
class PhysicalValueOwner {
    List<ValueListener> listeners = new ArrayList<>();
    List<Integer> rows = new ArrayList<>();
    int reads;
    int ticks;
    List<Integer> first() { reads++; return rows; }
    List<Integer> second() { reads++; return rows; }
    int flush() { ticks++; if (rows != null) rows.add(9); return ticks; }
    int observe(List<Integer> first, List<Integer> second, int tick) {
        ticks++;
        return first.size() * 100 + second.size() * 10 + tick;
    }
    int call() { return observe(first(), second(), flush()); }
    List<Integer> copy() { return new ArrayList<>(first()); }
    int accept(List<Integer> values, int value) { values.add(value); return values.size(); }
    int mutableProduced() { return accept(copy(), flush()); }
    void fire() { for (ValueListener listener : listeners) listener.ping(2); }
}
class PhysicalCallValues {
    static int run() {
        PhysicalValueOwner owner = new PhysicalValueOwner();
        ValueListenerImpl listener = new ValueListenerImpl();
        owner.listeners.add(listener);
        owner.fire(); owner.fire();
        if (listener.total != 4) return 1;
        owner.rows.add(2); owner.rows.add(3);
        List<Integer> alias = owner.first();
        owner.reads = 0;
        if (owner.call() != 331 || owner.reads != 2 || owner.ticks != 2) return 2;
        if (alias.size() != 3) return 3;
        owner.reads = 0;
        List<Integer> copy = owner.copy();
        if (owner.reads != 1 || copy.size() != 3) return 4;
        copy.add(8);
        if (alias.size() != 3 || copy.size() != 4) return 5;
        owner.reads = 0; owner.ticks = 0;
        if (owner.mutableProduced() != 4 || owner.reads != 1 || owner.ticks != 1 || alias.size() != 4) return 12;
        owner.rows = null;
        owner.reads = 0;
        try { owner.copy(); return 6; }
        catch (NullPointerException expected) { }
        if (owner.reads != 1) return 7;
        owner.reads = 0; owner.ticks = 0;
        try { owner.call(); return 8; }
        catch (NullPointerException expected) { }
        if (owner.reads != 2 || owner.ticks != 2) return 9;
        owner.listeners.clear();
        owner.listeners.add(new ClearingValueListener(owner));
        owner.listeners.add(listener);
        try { owner.fire(); return 10; }
        catch (ConcurrentModificationException expected) { }
        if (!owner.listeners.isEmpty() || listener.total != 4) return 11;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
