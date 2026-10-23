public class SourceSamReferenceProbe {
    interface Pair { double of(double x, double y); }
    interface Triple { double of(double x, double y, double z); }
    interface Vector { double of(double[] values); }

    static double sum(double x, double y) { return x + y; }
    static double sum(double x, double y, double z) { return x + y + z; }
    static double sum(double[] values) {
        double result = 0;
        for (double value : values) result += value;
        return result;
    }

    enum Operation {
        SUM(SourceSamReferenceProbe::sum, SourceSamReferenceProbe::sum, SourceSamReferenceProbe::sum),
        ALIAS(SUM);

        final Pair pair;
        final Triple triple;
        final Vector vector;

        Operation(Pair pair, Triple triple, Vector vector) {
            this.pair = pair;
            this.triple = triple;
            this.vector = vector;
        }

        Operation(Operation alias) { this(alias.pair, alias.triple, alias.vector); }

        double of(double x, double y) { return pair.of(x, y); }
        double of(double x, double y, double z) { return triple.of(x, y, z); }
        double of(double[] values) { return vector.of(values); }
    }

    static int run() {
        double[] values = {1, 2, 4};
        if (Operation.SUM.of(2, 3) != 5 || Operation.ALIAS.of(3, 4) != 7) return 1;
        if (Operation.SUM.of(2, 3, 4) != 9 || Operation.ALIAS.of(3, 4, 5) != 12) return 2;
        if (Operation.SUM.of(values) != 7 || Operation.ALIAS.of(values) != 7) return 3;
        values[0] = 8;
        if (Operation.SUM.of(values) != 14 || Operation.ALIAS.of(values) != 14) return 4;
        Pair local = SourceSamReferenceProbe::sum;
        if (local.of(9, 2) != 11) return 5;
        return 0;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
