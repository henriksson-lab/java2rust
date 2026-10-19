import java.util.*;
interface RowProvider<Key, Value> {
    List<Value> items();
    default Iterable<Value> rows() { return items(); }
}
class SwappedRows<Left, Right> implements RowProvider<Right, Left> {
    List<Left> values = new ArrayList<>();
    SwappedRows() { Iterable<Left> alias = values; }
    public List<Left> items() { return values; }
    void add(Left value) { values.add(value); }
}
class InheritedIterableProbe {
    static int run() {
        SwappedRows<String, Integer> source = new SwappedRows<>();
        Iterable<String> rows = source.rows();
        source.add("x"); source.add("y");
        int count = 0;
        for (String value : rows) count++;
        return count;
    }
}
