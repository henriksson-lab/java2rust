import java.util.ArrayList;
import java.util.Collection;
import java.util.List;

interface BridgeArraySink<T> {
    void remember(T[] values);
    Object saved();
}
final class BridgeDatum {
    int value;
    BridgeDatum(int value) { this.value = value; }
}
final class BridgeArrayOwner implements BridgeArraySink<BridgeDatum> {
    BridgeDatum[] values;
    public void remember(BridgeDatum[] values) { this.values = values; }
    public Object saved() { return values; }
}
interface BridgeCollectionSink<T> {
    void remember(Collection<? extends T> values);
    Object saved();
    int total();
    T[] copyTo(T[] target);
}
final class BridgeCollectionOwner implements BridgeCollectionSink<BridgeDatum> {
    Collection<? extends BridgeDatum> values;
    public void remember(Collection<? extends BridgeDatum> values) { this.values = values; }
    public Object saved() { return values; }
    public int total() {
        int total = 0;
        for (BridgeDatum value : values) { if (value != null) total += value.value; }
        return total;
    }
    public BridgeDatum[] copyTo(BridgeDatum[] target) { return values.toArray(target); }
}
class GenericBridgeContractProbe {
    @SuppressWarnings({"rawtypes", "unchecked"})
    static int run() {
        BridgeArraySink arrays = new BridgeArrayOwner();
        BridgeDatum first = new BridgeDatum(3);
        BridgeDatum[] original = new BridgeDatum[] { first };
        Object[] widened = original;
        arrays.remember(widened);
        if (arrays.saved() != original) return 1;
        Object[] wrongArrayClass = new Object[] { first };
        try { arrays.remember(wrongArrayClass); return 2; }
        catch (ClassCastException expected) { }
        if (arrays.saved() != original) return 3;
        arrays.remember(null);
        if (arrays.saved() != null) return 4;

        BridgeCollectionSink collection = new BridgeCollectionOwner();
        List<Object> source = new ArrayList<Object>();
        source.add(first);
        collection.remember(source);
        if (collection.saved() != source) return 5;
        source.add(new BridgeDatum(7));
        source.add(null);
        if (collection.total() != 10) return 6;
        Object alien = new Object();
        source.add(alien);
        // The Collection bridge checks no element when accepting this alias.
        collection.remember(source);
        if (collection.saved() != source) return 7;
        try { collection.total(); return 8; }
        catch (ClassCastException expected) { }
        BridgeDatum[] target = new BridgeDatum[] { null, null, first, first };
        try { collection.copyTo(target); return 9; }
        catch (ArrayStoreException expected) { }
        if (target[0] != first || target[1].value != 7 || target[2] != null || target[3] != first) return 10;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
