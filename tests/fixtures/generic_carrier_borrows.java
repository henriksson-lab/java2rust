class BorrowNode {
    int value;
    BorrowNode(int value) { this.value = value; }
    void set(int value) { this.value = value; }
}
class BorrowHolder {
    BorrowNode node;
    int calls;
    BorrowHolder(BorrowNode node) { this.node = node; }
    int touch() { calls++; return 0; }
}
interface BorrowAction { int get(); }
class BorrowReader<T extends BorrowNode> {
    T retained;
    BorrowReader(T retained) { this.retained = retained; }
    int read(T argument) { return argument.value; }
}
class GenericCarrierBorrows {
    static <T extends BorrowNode> int read(T value) { return value.value; }
    static <T extends BorrowNode> int maybe(T value) {
        return value == null ? -1 : value.value;
    }
    static <T extends BorrowNode> void write(T value, int n) { value.value = n; }
    static <T extends BorrowNode> int nested(T value) {
        write(value, read(value) + 1);
        return read(value);
    }
    static <T extends BorrowNode> int invoke(T value, BorrowAction action) {
        return action.get() + value.value;
    }
    static int failure(BorrowAction action) {
        try { return action.get(); }
        catch (IllegalArgumentException ex) { return -7; }
    }
    static int run() {
        BorrowNode node = new BorrowNode(3);
        BorrowHolder holder = new BorrowHolder(node);
        BorrowReader<BorrowNode> reader = new BorrowReader<>(node);
        if (read(node) != 3 || read(holder.node) != 3 || reader.read(node) != 3) return 1;
        write(node, 7);
        if (holder.node.value != 7 || reader.retained.value != 7) return 2;
        if (nested(node) != 8 || holder.node.value != 8) return 3;
        BorrowNode nil = null;
        if (maybe(nil) != -1 || maybe((BorrowNode) null) != -1 || maybe(null) != -1) return 4;
        if (maybe(node) != 8) return 5;
        if (invoke(holder.node, () -> holder.touch()) != 8 || holder.calls != 1) return 6;
        if (failure(() -> { throw new IllegalArgumentException(); }) != -7) return 7;
        return 0;
    }
}
