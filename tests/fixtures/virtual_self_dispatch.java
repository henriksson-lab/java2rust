class VirtualSelfBase {
    int value;
    int calls;
    VirtualSelfBase self() { calls++; return this; }
    VirtualSelfBase step(int increment) { value += increment; return self(); }
    int inspect() { return self().value; }
    VirtualSelfBase checked(boolean fail) {
        if (fail) throw new IllegalStateException("virtual");
        return self();
    }
}
class VirtualSelfMiddle extends VirtualSelfBase {
    @Override VirtualSelfMiddle self() { calls += 10; return this; }
}
class VirtualSelfLeaf extends VirtualSelfMiddle {
    @Override VirtualSelfLeaf self() { calls += 100; return this; }
}
class VirtualSelfInheritedLeaf extends VirtualSelfMiddle { }
class VirtualSelfDispatch {
    static int run() {
        VirtualSelfLeaf leaf = new VirtualSelfLeaf();
        VirtualSelfLeaf alias = leaf;
        VirtualSelfBase stepped = leaf.step(3);
        if (stepped != leaf || alias.value != 3 || alias.calls != 100) return 1;
        if (leaf.inspect() != 3 || alias.calls != 200) return 2;
        if (leaf.checked(false) != leaf || alias.calls != 300) return 3;
        try { leaf.checked(true); return 4; }
        catch (IllegalStateException expected) { }
        if (alias.calls != 300) return 5;
        VirtualSelfInheritedLeaf middle = new VirtualSelfInheritedLeaf();
        if (middle.step(7) != middle || middle.calls != 10) return 6;
        if (middle.inspect() != 7 || middle.calls != 20) return 7;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
