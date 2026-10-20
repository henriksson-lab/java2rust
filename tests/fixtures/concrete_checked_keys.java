import java.util.*;

class ConcreteCheckedKey {
    int value;
    boolean accepting;
    boolean failing;
    int calls;
    ConcreteCheckedKey(int value, boolean accepting, boolean failing) {
        this.value = value; this.accepting = accepting; this.failing = failing;
    }
    static int observe(ConcreteCheckedKey key) { return key.calls; }
    public boolean equals(int other) { return false; }
    public boolean equals(Object other) {
        calls++;
        if (failing) throw new IllegalStateException("equals");
        if (observe(this) != calls) throw new IllegalStateException("reentrant receiver");
        return other instanceof ConcreteCheckedKey &&
            (accepting || value == ((ConcreteCheckedKey) other).value);
    }
    public int hashCode() { return 1; }
}

class GenericCheckedKey<T> {
    boolean failing;
    GenericCheckedKey(boolean failing) { this.failing = failing; }
    public boolean equals(Object other) {
        if (failing) throw new IllegalStateException("generic equals");
        return true;
    }
    public int hashCode() { return 1; }
}

class ConcreteCheckedKeys {
    static <T> GenericCheckedKey<T> generic(boolean failing) {
        GenericCheckedKey<T> key = new GenericCheckedKey<T>(failing);
        GenericCheckedKey<T> alias = key;
        if (key != alias) throw new IllegalStateException("identity");
        return key;
    }
    static boolean search(HashSet<ConcreteCheckedKey> set, ConcreteCheckedKey key) {
        return set.contains(key);
    }
    static boolean immutableSearch(HashSet<GenericCheckedKey<String>> set, GenericCheckedKey<String> key) {
        return set.contains(key);
    }
    static int hashCase() {
        HashSet<ConcreteCheckedKey> set = new HashSet<>();
        ConcreteCheckedKey stored = new ConcreteCheckedKey(1, false, false);
        ConcreteCheckedKey query = new ConcreteCheckedKey(2, true, false);
        ConcreteCheckedKey failing = new ConcreteCheckedKey(3, false, true);
        set.add(stored);
        if (!search(set, query) || stored.calls != 0 || query.calls != 1) return 1;
        if (set.add(query) || set.size() != 1) return 2;
        try { search(set, failing); return 3; } catch (IllegalStateException expected) { }
        try { set.add(failing); return 4; } catch (IllegalStateException expected) { }
        try { set.remove(failing); return 5; } catch (IllegalStateException expected) { }
        if (set.size() != 1 || failing.calls != 3 || stored.calls != 0) return 6;
        if (!set.remove(query) || !set.isEmpty()) return 7;
        if (!stored.equals(stored)) return 19;
        set.add(failing);
        if (!set.contains(failing) || failing.calls != 3) return 8;
        try { failing.equals(failing); return 9; } catch (IllegalStateException expected) { }
        if (!set.remove(failing) || failing.calls != 4) return 10;
        GenericCheckedKey<String> immutable = new GenericCheckedKey<>(false);
        HashSet<GenericCheckedKey<String>> immutableSet = new HashSet<>();
        immutableSet.add(immutable);
        if (!immutableSearch(immutableSet, immutable)) return 20;
        return 0;
    }
    static int linkedCase() {
        LinkedHashSet<ConcreteCheckedKey> set = new LinkedHashSet<>();
        ConcreteCheckedKey stored = new ConcreteCheckedKey(1, false, false);
        ConcreteCheckedKey query = new ConcreteCheckedKey(2, true, false);
        ConcreteCheckedKey failing = new ConcreteCheckedKey(3, false, true);
        set.add(stored);
        if (!set.contains(query) || stored.calls != 0 || query.calls != 1) return 11;
        if (set.add(query)) return 12;
        try { set.contains(failing); return 13; } catch (IllegalStateException expected) { }
        try { set.add(failing); return 14; } catch (IllegalStateException expected) { }
        try { set.remove(failing); return 15; } catch (IllegalStateException expected) { }
        if (set.size() != 1 || failing.calls != 3 || stored.calls != 0) return 16;
        if (!set.remove(query) || !set.isEmpty()) return 17;
        set.add(failing);
        if (!set.contains(failing) || !set.remove(failing) || failing.calls != 3) return 18;
        return 0;
    }
    public static void main(String[] args) { System.out.println(hashCase() + linkedCase()); }
}
