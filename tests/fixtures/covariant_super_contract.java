class CovariantItem {
    int value;
    CovariantItem(int initial) { value = initial; }
    CovariantItem identity() { return this; }
}
class CovariantLeaf extends CovariantItem {
    CovariantLeaf(int initial) { super(initial); }
}
class CovariantParent {
    CovariantItem stored;
    CovariantParent(CovariantItem initial) { stored = initial; }
    CovariantItem get() { return stored; }
}
class CovariantChild extends CovariantParent {
    CovariantChild(CovariantItem initial) { super(initial); }
    @Override CovariantLeaf get() { return (CovariantLeaf) super.get(); }
    CovariantChild self() { return this; }
}
class CovariantSuperContract {
    static int run() {
        CovariantLeaf leaf = new CovariantLeaf(7);
        CovariantChild child = new CovariantChild(leaf);
        if (child.get() != leaf || leaf.value != 7) return 1;
        if (child.self() != child) return 2;
        CovariantChild invalid = new CovariantChild(new CovariantItem(9));
        try { invalid.get(); return 3; }
        catch (ClassCastException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
