import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;

class NullableCursorBase {
    int nulls;
    <T> NullableCursorBase(List<T> values) {
        Iterator<T> cursor = values.iterator();
        if (cursor.hasNext()) cursor.next();
        while (cursor.hasNext()) {
            T value = cursor.next();
            nulls += absent(value);
        }
    }
    static <T> int first(List<T> values) {
        T value = values.iterator().next();
        return absent(value);
    }
    static <T> int absent(T value) { return value == null ? 1 : 0; }
    int nullCount() { return nulls; }
}
class NullableCursorDerived extends NullableCursorBase {
    <T> NullableCursorDerived(List<T> values) { super(values); }
}
class NullableCursorDomainProbe {
    static int run() {
        List<String> values = new ArrayList<>();
        values.add("skip");
        values.add(null);
        values.add("present");
        values.add(null);
        NullableCursorBase ordinary = new NullableCursorBase(values);
        NullableCursorDerived inherited = new NullableCursorDerived(values);
        if (ordinary.nullCount() != 2 || inherited.nullCount() != 2) return 1;
        List<String> firstNull = new ArrayList<>();
        firstNull.add(null);
        if (NullableCursorBase.first(firstNull) != 1) return 2;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
