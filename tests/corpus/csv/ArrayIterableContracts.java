import java.util.Arrays;
import java.util.List;
import java.util.Iterator;
import java.util.ArrayList;

public class ArrayIterableContracts {
    static String[] backing;
    static Iterable<String> retained;
    static List<String> list() { return Arrays.asList(backing); }
    static Iterable<String> direct() { return Arrays.asList(backing); }
    static Iterable<String> project(List<String> input) { return input; }
    static int traverse(Iterable<String> input) {
        int result = 0;
        for (String value : input) {
            if ("first".equals(value)) backing[1] = "changed";
            if ("changed".equals(value)) result += 1;
            if (value == null) result += 2;
            if ("\ud800".equals(value)) result += 4;
        }
        return result;
    }
    static int run() {
        backing = new String[]{"first", "old", null, "\ud800"};
        List<String> list = list();
        Iterable<String> iterable = project(list);
        retained = iterable;
        if (iterable != list || !(iterable instanceof List)) return 1;
        if (traverse(retained) != 7) return 2;
        List<String> cast = (List<String>) iterable;
        cast.set(0, "set");
        if (!"set".equals(backing[0])) return 3;
        try { cast.add("no"); return 4; }
        catch (UnsupportedOperationException expected) { }
        Iterator<String> iterator = iterable.iterator();
        iterator.next();
        try { iterator.remove(); return 5; }
        catch (UnsupportedOperationException expected) { }
        backing[0] = "first";
        backing[1] = "old";
        if (traverse(direct()) != 7) return 6;
        String[] local = {"local", null};
        Iterable<String> localView = Arrays.asList(local);
        local[0] = "after";
        List<String> localList = (List<String>) localView;
        if (!"after".equals(localList.get(0)) || localList.get(1) != null) return 7;
        localList.set(0, "through");
        if (!"through".equals(local[0])) return 8;
        List<String> mutable = new ArrayList<String>();
        mutable.add("one");
        List<String> mutableView = (List<String>) project(mutable);
        if (!mutableView.add("two") || mutable.size() != 2) return 9;
        if (project(null) != null) return 10;
        try { traverse(project(null)); return 11; }
        catch (NullPointerException expected) { }
        backing = null;
        if (!"first".equals(cast.get(0))) return 12;
        try { direct(); return 13; }
        catch (NullPointerException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
