class ArrayReferenceIdentityProbe {
    static double[] fresh() { return new double[] {1.0, 2.0}; }
    static double[] relay(double[] value) { return value; }
    static int run() {
        double[] first = fresh();
        double[] second = fresh();
        if (first == second || !(first != second)) return 1;
        double[] alias = relay(first);
        if (first != alias || !(alias == first)) return 2;
        if (fresh() == fresh()) return 3;
        if (first == (first = second) || first != second) return 4;
        double[] absent = null;
        if (absent == first || first == absent || absent != null) return 5;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
