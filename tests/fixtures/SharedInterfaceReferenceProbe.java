import java.util.List;
import java.util.ArrayList;
interface RefCounter<T> {
    int value();
    void add(int delta);
    int reenter(RefCounter<T> alias);
    default int doubled() { return value() + value(); }
    RefCounter<T> clone();
    T item();
    void item(T value);
}
class RefCounterImpl implements RefCounter<String> {
    int count;
    String item = "initial";
    public String item() { return item; }
    public void item(String value) { item = value; }
    public int value() { return count; }
    public String toString() { return item + ":" + count; }
    public RefCounterImpl clone() {
        RefCounterImpl result = new RefCounterImpl();
        result.count = count;
        result.item = item;
        return result;
    }
    public void add(int delta) { count += delta; }
    public int reenter(RefCounter<String> alias) {
        int before = count;
        alias.add(2);
        return before + count + alias.value();
    }
}
class RefCounterHolder {
    List<RefCounter<String>> counters = new ArrayList<RefCounter<String>>();
    RefCounterHolder(RefCounter<String> counter) { counters.add(counter); }
    RefCounter<String> get() { return counters.get(0); }
}
class SharedInterfaceReferenceProbe {
    static RefCounterHolder holder() { return new RefCounterHolder(new RefCounterImpl()); }
    static int readCopies(RefCounterHolder holder, int times) {
        int total = 0;
        for (int i = 0; i < times; i++) total += holder.get().value();
        return total;
    }
    static int run() {
        RefCounterImpl source = new RefCounterImpl();
        RefCounterHolder holder = new RefCounterHolder(source);
        RefCounter<String> first = holder.get();
        RefCounter<String> second = holder.get();
        if (first != second || first != source) return 1;
        first.item("changed");
        if (!second.item().equals("changed")) return 7;
        first.add(3);
        if (second.value() != 3 || source.value() != 3) return 2;
        if (first.reenter(second) != 13 || source.value() != 5) return 3;
        RefCounterImpl recovered = (RefCounterImpl) second;
        recovered.add(1);
        if (first.value() != 6 || recovered != source) return 4;
        if (!(first instanceof RefCounterImpl) || first.doubled() != 12) return 5;
        RefCounter<String> distinct = first.clone();
        distinct.add(4);
        if (distinct == first || first.value() != 6 || distinct.value() != 10) return 8;
        if (!("seen:" + first).equals("seen:changed:6")) return 9;
        Object erased = first;
        if (erased != source) return 6;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
