class IdentityNode {
    final int value;
    IdentityNode(int value) { this.value = value; }
    public boolean equals(Object other) { return true; }
}

class IdentityBox<T> {
    T value;
    IdentityBox(T value) { this.value = value; }
    boolean same(T other) { return value == other; }
}

class GenericReferenceIdentity {
    static <T> boolean same(T left, T right) { return left == right; }
    static <T> boolean forward(T left, T right) { return same(left, right); }
    static <T> T pass(T value) { return value; }

    static int run() {
        IdentityNode a = new IdentityNode(5);
        IdentityNode b = new IdentityNode(5);
        if (same(a, b) || !same(a, a) || !forward(a, a)) return 1;
        if (!a.equals(b)) return 2;
        if (!same(null, null) || same(a, null) || same(null, b)) return 3;
        IdentityBox<IdentityNode> box = new IdentityBox<>(a);
        if (!box.same(a) || box.same(b) || !same(pass(a), a)) return 4;
        Object erased = a;
        Object alias = erased;
        Object other = b;
        if (!same(erased, alias) || same(erased, other)) return 5;
        String text = new String("equal");
        String copy = new String("equal");
        if (forward(text, copy) || !forward(text, text)) return 6;
        return 0;
    }
}
