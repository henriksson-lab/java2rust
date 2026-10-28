import java.util.Arrays;
import java.util.Iterator;
import java.util.List;

class ErasedCopyCell {
    int count;
    Object[] backing;
    ErasedCopyCell(Object[] backing) { this.backing = backing; }
    public String toString() {
        count++;
        backing[1] = "changed";
        return "cell";
    }
}

public class ErasedCollectionCopyContracts {
    static Iterable<Object> view(Object[] values) { return Arrays.asList(values); }
    static Iterator<Object> cursor(Object[] values) { return view(values).iterator(); }
    static int run() {
        Object[] backing = new Object[] { null, "old", null };
        ErasedCopyCell cell = new ErasedCopyCell(backing);
        backing[0] = cell;
        Iterable<Object> iterable = view(backing);
        Iterator<Object> first = iterable.iterator();
        Object retained = first.next();
        if (retained != cell || !retained.toString().equals("cell") || cell.count != 1) return 1;
        if (first.next() != backing[1] || first.next() != null) return 2;
        List<Object> list = (List<Object>)iterable;
        if (list.get(0) != cell || list.get(2) != null) return 3;
        Object previous = list.set(1, cell);
        if (!previous.equals("changed") || backing[1] != cell) return 4;
        try { list.add(cell); return 5; } catch (UnsupportedOperationException expected) { }
        Iterator<Object> escaped = cursor(backing);
        backing[0] = "late";
        if (escaped.next() != backing[0] || escaped.next() != cell) return 6;
        try { escaped.remove(); return 7; } catch (UnsupportedOperationException expected) { }
        if (escaped.next() != null) return 8;
        try { escaped.next(); return 9; } catch (java.util.NoSuchElementException expected) { }
        if (!(iterable instanceof List)) return 10;
        List<Object> direct = Arrays.asList(backing);
        if (direct.get(1) != cell || direct.get(2) != null) return 11;
        Iterator<Object> directCursor = direct.iterator();
        backing[0] = cell;
        if (directCursor.next() != cell || directCursor.next() != cell) return 12;
        if (directCursor.next() != null) return 13;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
