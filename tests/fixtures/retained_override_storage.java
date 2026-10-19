import java.util.*;

interface SetSlot {
    Set<Integer> values();
    void accept(Set<Integer> values);
}

class PlainSets implements SetSlot {
    private Set<Integer> items = new LinkedHashSet<>();
    public Set<Integer> values() { return items; }
    public void accept(Set<Integer> values) { values.add(9); }
}

class ReturningSets implements SetSlot {
    private List<Set<Integer>> groups = new ArrayList<>();
    private List<Set<Integer>> spare = new ArrayList<>(4);
    ReturningSets() {
        groups.add(new LinkedHashSet<Integer>());
        spare.add(groups.get(0));
    }
    public Set<Integer> values() { return groups.get(0); }
    public void accept(Set<Integer> values) { groups.add(values); }
}

class OverrideStorage {
    static int run() {
        SetSlot retained = new ReturningSets();
        Set<Integer> a = retained.values();
        a.add(3);
        SetSlot plain = new PlainSets();
        plain.accept(a);
        Set<Integer> b = plain.values();
        b.add(4);
        return a.size() * 10 + plain.values().size();
    }
}
