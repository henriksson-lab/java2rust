import java.util.Arrays;
import java.util.Collection;
import java.util.HashSet;
import java.util.Iterator;
import java.util.List;

class ArrayBackedCollectionBoundaries {
    static <T> Collection<T> pass(Collection<T> value) { return value; }
    static <T> List<T> passList(List<T> value) { return value; }

    static int run() {
        String[] values = new String[] { "before", "tail" };
        List<String> view = Arrays.asList(values);
        Collection<String> collection = pass(view);
        List<String> alias = passList(view);
        alias.set(0, "view");
        int result = values[0].equals("view") ? 1 : -1000;

        values[1] = "array";
        Iterator<String> cursor = collection.iterator();
        if (cursor.next().equals("view")) result += 2;
        values[1] = "late";
        if (cursor.next().equals("late")) result += 4;
        try {
            cursor.remove();
            return -1000;
        } catch (UnsupportedOperationException expected) { }
        try {
            collection.add("unsupported");
            return -1000;
        } catch (UnsupportedOperationException expected) {
            result += 8;
        }
        try {
            collection.remove("view");
            return -1000;
        } catch (UnsupportedOperationException expected) {
            result += 16;
        }
        if (collection.remove("missing")) return -1000;
        try {
            alias.clear();
            return -1000;
        } catch (UnsupportedOperationException expected) {
            result += 32;
        }

        Collection<String> set = pass(new HashSet<String>());
        if (set.add("only") && set.contains("only") && set.remove("only") && set.isEmpty()) {
            result += 64;
        }
        // A primitive array is one varargs element, not an array-backed
        // list of its primitive components.
        int[] numbers = new int[] { 1 };
        List<int[]> single = Arrays.asList(numbers);
        single.get(0)[0] = 7;
        if (numbers[0] != 7 || single.size() != 1) return -1000;

        String[] absent = null;
        try {
            List<String> invalid = Arrays.asList(absent);
            return -1000;
        } catch (NullPointerException expected) { }
        return result;
    }
}
