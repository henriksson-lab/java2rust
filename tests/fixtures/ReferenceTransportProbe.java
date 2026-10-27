interface NullableTransportView<T> {
    T bump();
}
class NullableTransportCounter implements NullableTransportView<NullableTransportCounter> {
    int value;
    NullableTransportCounter(int value) { this.value = value; }
    public NullableTransportCounter bump() { value++; return this; }
}
class NullableTransportPaths {
    static NullableTransportView<NullableTransportCounter> returnView(NullableTransportCounter source) {
        if (source == null) return null;
        return source;
    }
    static int consume(NullableTransportView<NullableTransportCounter> view) {
        if (view == null) return 0;
        return view.bump().value;
    }
    static int run(boolean present) {
        NullableTransportCounter source = null;
        if (present) source = new NullableTransportCounter(1);
        NullableTransportView<NullableTransportCounter> local = source;
        if (local == null) return source == null ? 0 : -1;
        local.bump();
        if (source.value != 2) return 1;
        int argument = consume(source);
        if (source.value != 3 || argument != 3) return 2;
        NullableTransportView<NullableTransportCounter> returned = returnView(source);
        returned.bump();
        if (source.value != 4) return 3;
        return 0;
    }
}
class TransportNode {
    int value;
    TransportNode self() { return this; }
    void set(int value) { this.value = value; }
}
class TransportPair<T> {
    T first;
    T second;
    TransportPair(T value) { first = value; second = value; }
}
class TransportSingle<T> {
    T value;
    TransportSingle(T value) { this.value = value; }
}
class ReferenceTransportProbe {
    static TransportNode kept;
    static TransportNode transfer() {
        TransportNode value = new TransportNode();
        value.set(7);
        TransportNode result = value;
        return result;
    }
    static void store() {
        TransportNode value = new TransportNode();
        value.set(9);
        kept = value;
    }
    static int same(TransportNode left, TransportNode right) {
        left.set(left.value + 1);
        return left == right ? right.value : -100;
    }
    static int run() {
        TransportNode moved = transfer();
        if (moved.value != 7) return 1;
        store();
        if (kept.value != 9) return 2;
        TransportNode reused = new TransportNode();
        TransportPair<TransportNode> pair = new TransportPair<TransportNode>(reused);
        pair.first.set(4);
        if (pair.second.value != 4 || pair.first != reused || pair.second != reused) return 3;
        if (same(reused, reused) != 5) return 4;
        for (int i = 0; i < 3; i++) {
            kept = reused;
            reused.set(reused.value + 1);
        }
        if (kept != reused || kept.value != 8) return 5;
        if (NullableTransportPaths.run(false) != 0) return 6;
        if (NullableTransportPaths.run(true) != 0) return 7;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
