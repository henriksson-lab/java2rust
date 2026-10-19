import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;

class ListProjectionBoundaries {
    static <T> List<T> cast(Iterable<T> source) { return (List<T>) source; }

    static <T> T element(Iterable<T> source, int index) {
        if (source instanceof List) { return ((List<T>) source).get(index); }
        java.util.Iterator<T> cursor = source.iterator();
        for (int i = 0; i < index; i++) { cursor.next(); }
        return cursor.next();
    }

    static int genericElement() {
        List<Integer> list = new ArrayList<>();
        list.add(4);
        LinkedHashSet<Integer> set = new LinkedHashSet<>();
        set.add(6);
        return element(list, 0) + element(set, 0);
    }

    static int touch(List<Integer> view) {
        int old = view.set(0, 7);
        return old * 10 + view.get(0);
    }

    static int aliases() {
        List<Integer> original = new ArrayList<>();
        original.add(2);
        Iterable<Integer> source = original;
        List<Integer> view = cast(source);
        return touch(view) + original.get(0);
    }

    static int nullCast() {
        List<Integer> view = ListProjectionBoundaries.<Integer>cast(null);
        return view == null ? 7 : 0;
    }

    static int rejectedSet() {
        Iterable<Integer> source = new LinkedHashSet<>();
        try {
            List<Integer> view = cast(source);
            return view.size();
        } catch (ClassCastException error) { return 7; }
    }

    static int nullableElement() {
        List<String> original = new ArrayList<>();
        original.add("x");
        Iterable<String> source = original;
        List<String> view = cast(source);
        view.set(0, null);
        int result = 0;
        for (String value : source) { if (value == null) { result += 100; } }
        if (original.get(0) == null) { result += 10; }
        return result;
    }
}
