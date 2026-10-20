import java.util.*;

class BulkMembershipEffects {
    int calls;
    Set<String> touch(Set<String> source) { calls++; return source; }
}

class MixedBulkMembership {
    static <T> boolean includes(Set<T> receiver, Set<T> source) {
        return receiver.containsAll(source);
    }
    static <T> boolean keep(Set<T> receiver, Set<T> source) {
        return receiver.retainAll(source);
    }
    static <T> boolean includesWithIdentity(Set<T> receiver, Set<T> source) {
        if (receiver == source) return true;
        return receiver.containsAll(source);
    }
    static int run() {
        Set<String> source = new HashSet<>();
        source.add("a");
        source.add("b");
        Set<String> alias = source;
        if (source != alias) return 1;
        Set<String> receiver = new HashSet<>();
        receiver.add("a");
        receiver.add("b");
        receiver.add("extra");
        if (!includes(receiver, source)) return 2;
        source.add("c");
        if (receiver.containsAll(source)) return 3;
        if (!keep(receiver, source) || receiver.contains("extra")) return 4;
        if (receiver.retainAll(source) || receiver.size() != 2) return 5;
        if (!receiver.addAll(source) || receiver.size() != 3) return 6;
        if (!receiver.removeAll(source) || !receiver.isEmpty()) return 7;
        Set<String> missing = null;
        try { receiver.containsAll(missing); return 8; }
        catch (NullPointerException expected) { }
        try { receiver.retainAll(missing); return 9; }
        catch (NullPointerException expected) { }
        BulkMembershipEffects effects = new BulkMembershipEffects();
        try { missing.containsAll(effects.touch(source)); return 10; }
        catch (NullPointerException expected) { }
        try { missing.retainAll(effects.touch(source)); return 11; }
        catch (NullPointerException expected) { }
        if (effects.calls != 2) return 12;
        if (alias != source || source.size() != 3) return 13;
        Set<String> retainedReceiver = new HashSet<>();
        retainedReceiver.add("a");
        if (!includesWithIdentity(source, retainedReceiver)) return 14;
        Set<String> nullableElements = new HashSet<>();
        nullableElements.add(null);
        if (nullableElements == source) return 15;
        receiver.add("a");
        if (receiver.containsAll(nullableElements)) return 16;
        if (!receiver.retainAll(nullableElements) || !receiver.isEmpty()) return 17;
        Set<String> plainQuery = new HashSet<>();
        plainQuery.add("a");
        if (!source.containsAll(plainQuery)) return 18;
        if (!source.retainAll(plainQuery) || source.size() != 1) return 19;
        if (source.retainAll(plainQuery)) return 20;
        source.add("b");
        if (!source.removeAll(plainQuery) || source.contains("a")) return 21;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
