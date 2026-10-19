import java.util.AbstractList;
import java.util.Arrays;
import java.util.Iterator;

class InheritedIterableSource extends AbstractList<Integer> {
    private int value;
    InheritedIterableSource(int value) { this.value = value; }
    public int size() { return 1; }
    public Integer get(int index) {
        if (index != 0) { throw new IndexOutOfBoundsException(); }
        return value;
    }
    public Integer set(int index, Integer next) {
        int previous = get(index);
        value = next;
        return previous;
    }
    public Iterator<Integer> iterator() { return Arrays.asList(value).iterator(); }
}

class InheritedIterableIdentity {
    static int run() {
        InheritedIterableSource source = new InheritedIterableSource(3);
        Iterable<Integer> alias = source;
        source.set(0, 9);
        return alias.iterator().next();
    }
}
