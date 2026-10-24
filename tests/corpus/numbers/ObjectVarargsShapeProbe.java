public final class ObjectVarargsShapeProbe {
    static int count(Object... values) { return values.length; }
    static int withPrefix(Object prefix, Object... values) { return values.length; }
    public static int run() {
        Object[] objects = {"a", "b", "c"};
        int[] primitive = {4,5,6,7};
        if (count(objects) != 3) return 1;
        if (count("prefix", objects) != 2) return 2;
        if (count(primitive) != 1) return 3;
        if (withPrefix("prefix", objects) != 3) return 4;
        if (withPrefix("prefix", "one", objects) != 2) return 5;
        if (withPrefix("prefix", primitive) != 1) return 6;
        if (count() != 0) return 7;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
