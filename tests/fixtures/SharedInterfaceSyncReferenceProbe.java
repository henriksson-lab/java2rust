interface SyncReferenceCounter {
    int value();
    void add(int delta);
    int reenter(SyncReferenceCounter alias);
}
class SyncReferenceCounterImpl implements SyncReferenceCounter {
    int count;
    public int value() { return count; }
    public void add(int delta) { count += delta; }
    public int reenter(SyncReferenceCounter alias) {
        int before = count;
        alias.add(2);
        return before + count + alias.value();
    }
}
class SharedInterfaceSyncReferenceProbe {
    static SyncReferenceCounter shared = new SyncReferenceCounterImpl();
    static int run() {
        SyncReferenceCounter first = shared;
        SyncReferenceCounter second = shared;
        first.add(3);
        if (first != second || first.reenter(second) != 13 || shared.value() != 5) return 1;
        SyncReferenceCounterImpl concrete = (SyncReferenceCounterImpl) first;
        concrete.add(1);
        if (shared.value() != 6 || first != concrete) return 2;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
