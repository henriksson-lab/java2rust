package producer;

public enum OverloadedEnum {
    FIRST(OverloadedEnum::pick, OverloadedEnum::pick, OverloadedEnum::pick);
    interface Two { double call(double a, double b); }
    interface Three { double call(double a, double b, double c); }
    interface Array { double call(double[] values); }
    private final Two two;
    private final Three three;
    private final Array array;
    OverloadedEnum(Two two, Three three, Array array) {
        this.two = two; this.three = three; this.array = array;
    }
    private static double pick(double a, double b) { return a + b; }
    private static double pick(double a, double b, double c) { return a + b + c; }
    private static double pick(double[] values) { return values[0]; }
    public double call(double a, double b) { return two.call(a,b); }
    public double call(double a, double b, double c) { return three.call(a,b,c); }
    public double call(double[] values) { return array.call(values); }
}
