import java.util.*;
interface Traversal<Key, Value> {
    default Iterable<Value> iterate(List<Value> values) {
        Iterable<Value> alias = values;
        return values;
    }
}
interface FlippedTraversal<Left, Right> extends Traversal<Right, Left> {}
interface Copying<T> { T copy(T value); }
class MutableCursor<Left, Right> implements FlippedTraversal<Left, Right>, Copying<Left> {
    int marker;
    void mark(int value) { marker = value; }
    public Left copy(Left value) { Left saved = value; return value; }
}
class ScopedCursorFactory {
    static <Left, Right> Traversal<Right, Left> make() { return new MutableCursor<Left, Right>(); }
    static <Left, Right> Copying<Left> copying() { return new MutableCursor<Left, Right>(); }
    static int run() {
        List<String> values = new ArrayList<>();
        Traversal<Integer, String> cursor = ScopedCursorFactory.<String, Integer>make();
        Iterable<String> rows = cursor.iterate(values);
        values.add("x");
        int count = 0;
        for (String value : rows) count++;
        return count + ScopedCursorFactory.<String, Integer>copying().copy("ok").length();
    }
}
