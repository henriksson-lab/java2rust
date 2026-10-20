class GenericVirtualBase<A> {
    int calls;
    A pick(A value) { calls++; return value; }
    A invoke(A value) { return pick(value); }
}
class GenericVirtualMiddle<U> extends GenericVirtualBase<U> {
    @Override U pick(U value) { calls += 10; return value; }
}
class GenericVirtualLeaf extends GenericVirtualMiddle<String> {
    @Override String pick(String value) { calls += 100; return value; }
}
class GenericVirtualInheritedLeaf extends GenericVirtualMiddle<String> { }
class GenericVirtualSelfDispatch {
    static int run() {
        GenericVirtualLeaf leaf = new GenericVirtualLeaf();
        GenericVirtualLeaf alias = leaf;
        if (!leaf.invoke("leaf").equals("leaf") || alias.calls != 100) return 1;
        GenericVirtualInheritedLeaf inherited = new GenericVirtualInheritedLeaf();
        if (!inherited.invoke("middle").equals("middle") || inherited.calls != 10) return 2;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
