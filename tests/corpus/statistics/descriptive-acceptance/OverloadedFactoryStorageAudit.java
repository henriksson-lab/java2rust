package overloadedfactorystorage;
public class OverloadedFactoryStorageAudit {
    private static final int[] EMPTY = {};
    private static final int[] SEED = {1, 2};
    static int[] build() { return create(SEED); }
    static int[] build(int[] values) { return create(values); }
    static int[] build(int[] values, int from, int to) { return create(values); }
    static int[] create(int[] values) { return values; }
    public static int run() {
        int[] first = build();
        if (first != SEED) return 1;
        first[0] = 7;
        int[] second = build();
        if (second != first || second[0] != 7) return 2;
        int[] local = {3, 4};
        int[] alias = build(local);
        if (alias != local || alias == SEED) return 3;
        alias[1] = 9;
        if (local[1] != 9 || SEED[1] != 2) return 4;
        if (build(EMPTY, 0, 0) != EMPTY) return 5;
        int[] ranged = build(local, 0, 2);
        ranged[0] = 11;
        if (ranged != local || local[0] != 11 || SEED[0] != 7) return 6;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
