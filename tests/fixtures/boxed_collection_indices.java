import java.util.*;

class BoxedCollectionIndices {
    static int read(int value) { return value + 1; }
    static int run() {
        List<Integer> values = new ArrayList<>();
        List<Integer> view = Collections.unmodifiableList(values);
        values.add(10);
        values.add(20);
        Integer index = null;
        index = 1;
        values.set(index, 30);
        values.add(index, 25);
        if (values.get(index) != 25 || view.size() != 3 || read(index) != 2) return 1;
        ListIterator<Integer> cursor = values.listIterator(index);
        if (cursor.next() != 25) return 2;
        index = null;
        try { values.set(index, 40); return 3; }
        catch (NullPointerException expected) { }
        try { values.get(index); return 4; }
        catch (NullPointerException expected) { }
        try { read(index); return 6; }
        catch (NullPointerException expected) { }
        if (values.get(1) != 25 || values.size() != 3) return 5;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
