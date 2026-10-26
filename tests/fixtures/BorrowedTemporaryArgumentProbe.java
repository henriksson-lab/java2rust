interface BorrowedTemporaryProduct {
    BorrowedTemporaryProduct clone();
    int value();
}
class BorrowedTemporaryValue implements BorrowedTemporaryProduct {
    int value;
    BorrowedTemporaryValue(int value) { this.value = value; }
    public BorrowedTemporaryProduct clone() { return new BorrowedTemporaryValue(value); }
    public int value() { return value; }
}
class BorrowedTemporaryTarget {
    int read(BorrowedTemporaryProduct product, int marker) { return product.value() + marker; }
}
class BorrowedTemporaryArgumentProbe {
    static int events;
    static BorrowedTemporaryTarget target(boolean absent) {
        events = events * 10 + 1;
        return absent ? null : new BorrowedTemporaryTarget();
    }
    static BorrowedTemporaryProduct make() {
        events = events * 10 + 2;
        return new BorrowedTemporaryValue(7);
    }
    static int marker() { events = events * 10 + 3; return 3; }
    static int call(boolean absent) { return target(absent).read(make().clone(), marker()); }
    static int run() {
        events = 0;
        if (call(false) != 10 || events != 123) return 1;
        events = 0;
        try { call(true); return 2; }
        catch (NullPointerException expected) { if (events != 123) return 3; }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
