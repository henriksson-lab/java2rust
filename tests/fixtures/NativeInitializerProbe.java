
class NativeInitializerProbe {
    int[] values = make();
    NativeInitializerProbe() {}
    static int[] make() { return new int[] { 4 }; }
    static int run() {
        NativeInitializerProbe probe = new NativeInitializerProbe();
        int[] alias = probe.values;
        alias[0] = 9;
        return probe.values[0] - 9;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
