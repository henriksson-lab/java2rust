class NativeContractBase {
    int inspect(int[] values) { return values[0]; }
}
class NativeContractChild extends NativeContractBase {
    int[] saved;
    int inspect(int[] values) {
        saved = values;
        values = new int[] { 9 };
        return saved[0] + values[0];
    }
}
class RetainedOverrideArgumentProbe {
    static int run() {
        NativeContractChild child = new NativeContractChild();
        NativeContractBase base = child;
        int[] values = new int[] { 3 };
        if (base.inspect(values) != 12) return 1;
        values[0] = 7;
        if (child.saved[0] != 7) return 2;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
