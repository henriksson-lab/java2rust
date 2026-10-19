import java.util.AbstractList;
import java.util.Arrays;
import java.util.Iterator;
import java.util.List;

class ProjectedSourceList extends AbstractList<Integer> {
    private int base;
    ProjectedSourceList(int base) { this.base = base; }
    public int size() { return 2; }
    public Integer get(int index) {
        if (index < 0 || index >= size()) { throw new IndexOutOfBoundsException(); }
        return base + index;
    }
    public Integer set(int index, Integer value) {
        int previous = get(index);
        if (index != 0) { throw new UnsupportedOperationException(); }
        base = value;
        return previous;
    }
    // Deliberately different from get: preserving virtual List dispatch is
    // observable, so a cast cannot be implemented with iterator traversal.
    public Iterator<Integer> iterator() {
        return Arrays.asList(base + 100, base + 101).iterator();
    }
}

class SourceListProjection {
    static int read(Iterable<Integer> values, int index) {
        if (values instanceof List) { return ((List<Integer>) values).get(index); }
        Iterator<Integer> cursor = values.iterator();
        for (int i = 0; i < index; i++) { cursor.next(); }
        return cursor.next();
    }
    static int liveRead() {
        ProjectedSourceList source = new ProjectedSourceList(3);
        Iterable<Integer> alias = source;
        int first = read(alias, 1);
        source.set(0, 7);
        return first * 10 + read(alias, 0);
    }
    static int mutationThroughCast() {
        ProjectedSourceList source = new ProjectedSourceList(2);
        Iterable<Integer> alias = source;
        List<Integer> list = (List<Integer>) alias;
        int previous = list.set(0, 7);
        return previous * 10 + source.get(1);
    }
}
