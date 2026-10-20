import java.util.*;

class ScopedIdentityNode {
    int value;
    ScopedIdentityNode(int value) { this.value = value; }
}

class ScopedReferenceContracts {
    static <T> boolean nested(T[] values, T target) {
        for (int i = 0; i < values.length; i++) {
            T selected = values[i];
            T candidate = target;
            if (selected == candidate) return true;
        }
        return false;
    }
    static <T> boolean enhanced(T[] values, T target) {
        if (values.length > 0) {
            for (T selected : values) {
                T candidate = target;
                if (selected == candidate) return true;
            }
        }
        return false;
    }
    static <T> boolean valueEquals(T[] values, T target) {
        if (values.length > 0) {
            for (T selected : values) {
                T candidate = target;
                if (selected.equals(candidate)) return true;
            }
        }
        return false;
    }
    static boolean append(List<String> target, List<String> source) {
        return target.addAll(source);
    }
    static <T> boolean liveArray(T[] values, T replacement) {
        int index = 0;
        for (T selected : values) {
            if (index == 0) values[1] = replacement;
            else return selected == replacement;
            index++;
        }
        return false;
    }
    static <T> int stableArray(T[] values, T[] other, T expected) {
        int matches = 0;
        for (T selected : values) {
            values = other;
            if (values.length != other.length) return -1;
            if (selected == expected) matches++;
        }
        return matches;
    }
    static <T> int count(T[] values) {
        int count = 0;
        for (T selected : values) count++;
        return count;
    }
    static <T> int guarded(T[] values) {
        int count = 0;
        if (values != null) {
            for (T selected : values) count++;
        }
        return count;
    }
    static void appendDiscarded(List<String> target, List<String> source) {
        target.addAll(source);
    }
    static void clear(List<String> target) { target.clear(); }
    static String replace(List<String> target, int index, String value) {
        return target.set(index, value);
    }
    static int run() {
        ScopedIdentityNode first = new ScopedIdentityNode(5);
        ScopedIdentityNode second = new ScopedIdentityNode(5);
        ScopedIdentityNode[] values = new ScopedIdentityNode[] { first };
        if (!nested(values, first) || nested(values, second)) return 1;
        if (!enhanced(values, first) || enhanced(values, second)) return 2;
        if (!valueEquals(values, first) || valueEquals(values, second)) return 3;
        List<String> target = new ArrayList<String>();
        List<String> alias = target;
        List<String> source = Arrays.asList(new String[] { "piece" });
        appendDiscarded(alias, source);
        if (target.size() != 1) return 4;
        if (!append(alias, source) || target.size() != 2) return 5;
        if (!append(target, target) || alias.size() != 4) return 6;
        List<String> readonly = Collections.unmodifiableList(target);
        try { appendDiscarded(readonly, source); return 7; }
        catch (UnsupportedOperationException expected) { }
        target.add("later");
        if (readonly.size() != 5) return 13;
        replace(alias, 0, "updated");
        if (!readonly.get(0).equals("updated")) return 14;
        try { replace(readonly, 0, "forbidden"); return 15; }
        catch (UnsupportedOperationException expected) { }
        try { append(readonly, Collections.<String>emptyList()); return 16; }
        catch (UnsupportedOperationException expected) { }
        List<String> empty = new ArrayList<String>();
        List<String> emptyView = Collections.unmodifiableList(empty);
        try { clear(emptyView); return 17; }
        catch (UnsupportedOperationException expected) { }
        Iterator<String> cursor = readonly.iterator();
        target.add("invalidate");
        try { cursor.next(); return 18; }
        catch (ConcurrentModificationException expected) { }
        ScopedIdentityNode[] mutable = new ScopedIdentityNode[] { first, second };
        if (!liveArray(mutable, first) || mutable[1] != first) return 8;
        if (stableArray(mutable, new ScopedIdentityNode[] { second }, first) != 2) return 9;
        if (count(mutable) != 2 || guarded(mutable) != 2) return 10;
        if (guarded((ScopedIdentityNode[]) null) != 0) return 11;
        try { count((ScopedIdentityNode[]) null); return 12; }
        catch (NullPointerException expected) { }
        return 0;
    }
}
