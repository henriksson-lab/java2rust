class NullableVirtualBase {
    int calls;
    NullableVirtualBase next(boolean empty) {
        calls++;
        if (empty) return null;
        return this;
    }
    NullableVirtualBase forward(boolean empty) { return next(empty); }
    NullableVirtualBase checked(boolean empty, boolean fail) {
        if (fail) throw new IllegalStateException("nullable virtual");
        return next(empty);
    }
}
class NullableVirtualMiddle extends NullableVirtualBase {
    @Override NullableVirtualMiddle next(boolean empty) {
        calls += 10;
        if (empty) return null;
        return this;
    }
}
class NullableVirtualLeaf extends NullableVirtualMiddle {
    @Override NullableVirtualLeaf next(boolean empty) {
        calls += 100;
        if (empty) return null;
        return this;
    }
}
class NullableVirtualAlwaysLeaf extends NullableVirtualMiddle {
    @Override NullableVirtualAlwaysLeaf next(boolean empty) {
        calls += 1000;
        return this;
    }
}
class NullableVirtualSelfDispatch {
    static int run() {
        NullableVirtualLeaf leaf = new NullableVirtualLeaf();
        NullableVirtualBase base = leaf;
        NullableVirtualMiddle middle = leaf;
        if (base.next(false) != leaf || leaf.calls != 100) return 1;
        if (middle.next(true) != null || leaf.calls != 200) return 2;
        if (leaf.forward(false) != leaf || leaf.calls != 300) return 3;
        if (leaf.forward(true) != null || leaf.calls != 400) return 4;
        if (leaf.checked(false, false) != leaf || leaf.calls != 500) return 5;
        if (leaf.checked(true, false) != null || leaf.calls != 600) return 6;
        try { leaf.checked(false, true); return 7; }
        catch (IllegalStateException expected) { }
        if (leaf.calls != 600) return 8;
        NullableVirtualBase nonNull = new NullableVirtualAlwaysLeaf();
        if (nonNull.next(true) != nonNull || nonNull.calls != 1000) return 9;
        if (nonNull.forward(true) != nonNull || nonNull.calls != 2000) return 10;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
