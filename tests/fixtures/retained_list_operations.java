import java.util.*;

class RetainedListOperations {
    static <T> List<T> copy(List<T> source) { return new ArrayList<T>(source); }

    static int run() {
        String[] values = new String[] { "a", "b", "c" };
        List<String> backed = Arrays.asList(values);
        ListIterator<String> cursor = backed.listIterator(1);
        ListIterator<String> alias = cursor;
        values[1] = "B";
        if (!alias.next().equals("B") || cursor.nextIndex() != 2) return -1;
        if (!cursor.previous().equals("B") || cursor.previousIndex() != 0) return -2;
        cursor.set("X");
        if (!values[1].equals("X")) return -3;
        int failures = 0;
        try { cursor.add("unsupported"); }
        catch (UnsupportedOperationException expected) { failures++; }
        try { cursor.remove(); }
        catch (UnsupportedOperationException expected) { failures++; }
        Collections.reverse(backed);
        if (!values[0].equals("c") || !values[2].equals("a")) return -4;
        if (!cursor.next().equals("X")) return -5;

        List<String> copied = copy(backed);
        copied.set(0, "copy");
        if (!values[0].equals("c")) return -6;
        copied.add("tail");
        ListIterator<String> mutable = copied.listIterator();
        try { mutable.remove(); }
        catch (IllegalStateException expected) { failures++; }
        if (!mutable.next().equals("copy")) return -7;
        mutable.set("C");
        mutable.add("insert");
        if (!mutable.previous().equals("insert")) return -8;
        mutable.remove();
        if (!mutable.next().equals("X")) return -9;
        mutable.set(null);
        if (copied.get(1) != null) return -10;
        ListIterator<String> stale = copied.listIterator();
        copied.add("late");
        try { stale.previous(); }
        catch (ConcurrentModificationException expected) { failures++; }
        try { stale.next(); }
        catch (ConcurrentModificationException expected) { failures++; }
        try { stale.remove(); }
        catch (IllegalStateException expected) { failures++; }
        try { copied.listIterator(-1); }
        catch (IndexOutOfBoundsException expected) { failures++; }
        try { copied.listIterator(copied.size() + 1); }
        catch (IndexOutOfBoundsException expected) { failures++; }

        List<String> nullable = new ArrayList<String>();
        nullable.add("present");
        ListIterator<String> writer = nullable.listIterator();
        writer.next();
        writer.set(null);
        if (nullable.get(0) != null) return -11;
        String absent = writer.previous();
        if (absent != null) return -15;
        ListIterator<String> nested = nullable.listIterator();
        nested.set(nested.next());
        if (nullable.get(0) != null) return -16;
        List<String> castNull = new ArrayList<String>();
        castNull.add("value");
        ListIterator<String> castWriter = castNull.listIterator();
        castWriter.next();
        castWriter.set((String) null);
        if (castNull.get(0) != null) return -17;

        String[] row = new String[] { "row" };
        List<String[]> rows = new ArrayList<String[]>();
        rows.add(row);
        ListIterator<String[]> references = rows.listIterator();
        String[] observed = references.next();
        observed[0] = "updated";
        references.set(row);
        if (!row[0].equals("updated")) return -14;

        List<String> empty = Collections.emptyList();
        List<String> singleton = Collections.singletonList("one");
        Collections.reverse(empty);
        Collections.reverse(singleton);
        ListIterator<String> end = singleton.listIterator(1);
        if (!end.hasPrevious() || !end.previous().equals("one")) return -12;
        try { end.set("no"); }
        catch (UnsupportedOperationException expected) { failures++; }
        if (failures != 9) return -13;
        return 23;
    }
}
