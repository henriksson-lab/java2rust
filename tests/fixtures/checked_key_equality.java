import java.util.*;

class CheckedEqualityBase {
    int value;
    boolean fail;
    CheckedEqualityBase(int value, boolean fail) { this.value = value; this.fail = fail; }
    public boolean equals(Object other) {
        if (fail) throw new IllegalStateException("equals");
        return other instanceof CheckedEqualityBase && value == ((CheckedEqualityBase) other).value;
    }
    public int hashCode() { return value; }
}
class CheckedEqualityChild extends CheckedEqualityBase {
    CheckedEqualityChild(int value, boolean fail) { super(value, fail); }
}
class CheckedKeyEquality {
    static CheckedEqualityBase make(int value, boolean fail) {
        return new CheckedEqualityChild(value, fail);
    }
    static int run() {
        CheckedEqualityBase left = make(7, false);
        CheckedEqualityBase right = make(7, false);
        if (!left.equals(right)) return 1;
        if (left.equals(null)) return 4;
        if (left.equals(make(8, false))) return 2;
        try { make(7, true).equals(right); return 3; }
        catch (IllegalStateException expected) { }
        return 0;
    }
    static int bulk() {
        List<CheckedEqualityBase> receiver = new ArrayList<>();
        List<CheckedEqualityBase> alias = receiver;
        List<CheckedEqualityBase> source = new ArrayList<>();
        List<CheckedEqualityBase> sourceAlias = source;
        receiver.add(make(7, false));
        source.add(make(7, true));
        try { receiver.containsAll(source); return 10; }
        catch (IllegalStateException expected) { }
        source.clear();
        source.add(make(7, false));
        if (!receiver.containsAll(source)) return 11;
        receiver.clear();
        receiver.add(make(8, false));
        receiver.add(make(7, true));
        try { receiver.retainAll(source); return 12; }
        catch (IllegalStateException expected) { }
        if (alias.size() != 1) return 13;
        receiver.clear();
        receiver.add(make(7, false));
        receiver.add(make(7, true));
        try { receiver.removeAll(source); return 14; }
        catch (IllegalStateException expected) { }
        if (alias.size() != 1) return 15;
        if (receiver != alias || source != sourceAlias) return 16;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run() + bulk()); }
}
