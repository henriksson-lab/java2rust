import java.util.*;

class RetainedBulk {
    private List<Integer> stored = new ArrayList<>();

    private List<Integer> replaceSource() {
        stored = new ArrayList<>();
        stored.add(9);
        return stored;
    }

    static int receiverReplacement() {
        RetainedBulk holder = new RetainedBulk();
        Iterable<Integer> old = holder.stored;
        holder.stored.add(1);
        holder.stored.addAll(holder.replaceSource());
        int total = 0;
        for (int value : old) { total += value; }
        return total * 10 + holder.stored.size();
    }

    static int sameOwnerReplacement() {
        RetainedBulk holder = new RetainedBulk();
        return holder.receiverReplacementInner();
    }

    private int receiverReplacementInner() {
        Iterable<Integer> old = stored;
        stored.add(1);
        stored.addAll(replaceSource());
        int total = 0;
        for (int value : old) { total += value; }
        return total * 10 + stored.size();
    }

    static int run() {
        List<Integer> values = new ArrayList<>();
        Iterable<Integer> view = values;
        values.add(2);
        List<Integer> alias = values;
        boolean changed = values.addAll(alias);
        Set<Integer> unique = new LinkedHashSet<>();
        Iterable<Integer> setView = unique;
        unique.add(2);
        boolean duplicate = unique.addAll(values);
        List<Integer> plain = new ArrayList<>();
        plain.add(3);
        unique.addAll(plain);
        plain.addAll(unique);
        if (plain.size() != 3) { return -99; }
        int total = 0;
        for (int value : view) { total += value; }
        for (int value : setView) { total += value; }
        if (changed && !duplicate) { total += 10; }
        return total;
    }

    static int nullSource() {
        List<Integer> values = new ArrayList<>();
        Iterable<Integer> view = values;
        values.add(7);
        List<Integer> absent = null;
        try { values.addAll(absent); }
        catch (NullPointerException failure) { return values.size(); }
        return -1;
    }

    static int emptyInvalidation() {
        List<Integer> empty = new ArrayList<>();
        List<Integer> array = new ArrayList<>();
        Iterable<Integer> arrayView = array;
        array.add(1);
        array.add(2);
        Iterator<Integer> first = array.iterator();
        first.next();
        array.addAll(empty);
        int result = 0;
        try { first.next(); }
        catch (ConcurrentModificationException failure) { result = 1; }
        List<Integer> linked = new LinkedList<>();
        Iterable<Integer> linkedView = linked;
        linked.add(1);
        linked.add(2);
        Iterator<Integer> second = linked.iterator();
        second.next();
        linked.addAll(empty);
        if (second.next() == 2) { result += 10; }
        return result;
    }
}
