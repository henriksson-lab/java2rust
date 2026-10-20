import java.util.*;
class SharedIteratorOwner {
    int calls;
    void touch() { calls++; }
    SharedIteratorOwner self() { return this; }
    void removeOne(Iterator<String> iterator, boolean fail) {
        iterator.next();
        iterator.remove();
        if (fail) throw new IllegalStateException("after remove");
    }
}
class SharedIteratorChild extends SharedIteratorOwner { }
class SharedIteratorCleanup {
    static int run() {
        SharedIteratorChild owner = new SharedIteratorChild();
        SharedIteratorChild alias = owner;
        owner.touch();
        if (alias.calls != 1) return 4;
        List<String> values = new ArrayList<>();
        values.add("a"); values.add("b"); values.add("c");
        List<String> retained = values;
        Iterator<String> iterator = values.iterator();
        owner.removeOne(iterator, false);
        if (retained.size() != 2 || !retained.get(0).equals("b")) return 1;
        try { alias.removeOne(iterator, true); return 2; }
        catch (IllegalStateException expected) { }
        if (retained.size() != 1 || !retained.get(0).equals("c")) return 3;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
