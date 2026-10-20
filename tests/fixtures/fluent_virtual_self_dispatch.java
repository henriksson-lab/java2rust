abstract class FluentVirtualBase<B extends FluentVirtualBase<B>> {
    int calls;
    B cached;
    protected abstract B self();
    B step() { calls++; return self(); }
    B relay(B other) { cached = other; return cached; }
    int read(B other) { return other.calls; }
    int bump(B other) { other.calls++; return other.calls; }
}
class FluentVirtualLeaf extends FluentVirtualBase<FluentVirtualLeaf> {
    @Override protected FluentVirtualLeaf self() { return this; }
}
class FluentVirtualSelfDispatch {
    static int run() {
        FluentVirtualLeaf leaf = new FluentVirtualLeaf();
        FluentVirtualLeaf alias = leaf;
        if (leaf.step() != alias || alias.calls != 1) return 1;
        if (leaf.relay(alias) != leaf || alias.cached != leaf) return 2;
        if (leaf.read(alias) != 1) return 3;
        if (leaf.bump(alias) != 2 || alias.calls != 2) return 4;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
