public final class DeclaredDefaultSamAudit {
    interface TextMeasure {
        default int marker() { return 3; }
        int apply(String value);
    }
    static TextMeasure create() { return value -> value.length(); }
    public static int run() {
        TextMeasure measure = create();
        if (measure.apply("abcd") != 4) return 1;
        if (measure.marker() != 3) return 2;
        if (measure.apply("x") != 1) return 3;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
