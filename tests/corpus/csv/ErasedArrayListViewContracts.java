import java.util.Arrays;
import java.util.Iterator;
import java.util.List;

class ArrayListViewCallbackValue {
    Object[] backing;
    ArrayListViewCallbackValue(Object[] backing) { this.backing = backing; }
    public String toString() { backing[1] = "callback"; return "value"; }
}

public class ErasedArrayListViewContracts {
    static final String[] SHARED = new String[] { "shared", null };
    static Iterable<Object> view(Object array) { return Arrays.asList((Object[])array); }
    static int run() {
        String[] backing = new String[] { "first", null };
        Object array = backing;
        Iterable<Object> iterable = view(array);
        if (!(iterable instanceof List)) return 1;
        List<Object> list = (List<Object>)iterable;
        if (list.size() != 2 || list.get(1) != null) return 2;
        Iterator<Object> cursor = iterable.iterator();
        backing[0] = "late";
        if (cursor.next() != backing[0]) return 3;
        Object previous = list.set(0, "set");
        if (!previous.equals("late") || !backing[0].equals("set")) return 4;
        try { list.set(0, Integer.valueOf(4)); return 5; }
        catch (ArrayStoreException expected) { }
        if (!backing[0].equals("set")) return 6;
        if (cursor.next() != null) return 7;
        try { cursor.next(); return 17; }
        catch (java.util.NoSuchElementException expected) { }
        try { list.get(-1); return 18; }
        catch (IndexOutOfBoundsException expected) { }
        try { list.set(2, "outside"); return 19; }
        catch (IndexOutOfBoundsException expected) { }
        try { cursor.remove(); return 8; }
        catch (UnsupportedOperationException expected) { }
        try { list.add("extra"); return 9; }
        catch (UnsupportedOperationException expected) { }
        try { view(null); return 10; }
        catch (NullPointerException expected) { }
        try { view(new int[2]); return 11; }
        catch (ClassCastException expected) { }
        if (view(array) == iterable || (Object)list == array) return 12;
        Object[] objects = new Object[] { null, "old" };
        objects[0] = new ArrayListViewCallbackValue(objects);
        Iterator<Object> reentrant = view(objects).iterator();
        if (!reentrant.next().toString().equals("value")) return 13;
        if (!reentrant.next().equals("callback")) return 14;
        Iterable<Object> sharedView = view(SHARED);
        SHARED[0] = "shared-late";
        List<Object> sharedList = (List<Object>)sharedView;
        if (sharedList.get(0) != SHARED[0]) return 15;
        sharedList.set(0, "shared-set");
        if (!SHARED[0].equals("shared-set")) return 16;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
