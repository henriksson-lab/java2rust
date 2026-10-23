class BoxedPrimitiveEqualsProbe {
    static int calls;
    static Object argument() { calls++; return Double.valueOf(1.0); }
    static boolean doubleEquals(Double left, Object right) { return left.equals(right); }
    static boolean floatEquals(Float left, Object right) { return left.equals(right); }

    static int run() {
        Double nan = Double.NaN;
        Float singleNan = Float.NaN;
        if (!nan.equals(Double.NaN) || !singleNan.equals(Float.NaN)) return 1;
        Double zero = 0.0;
        Float singleZero = 0.0f;
        if (zero.equals(-0.0) || singleZero.equals(-0.0f)) return 2;
        if (!Double.valueOf(-0.0).equals(-0.0) || !Float.valueOf(-0.0f).equals(-0.0f)) return 3;
        if (nan.equals(singleNan) || singleNan.equals(nan)) return 4;
        if (Double.valueOf(1.0).equals(Integer.valueOf(1))) return 5;
        if (Integer.valueOf(1).equals(Long.valueOf(1))) return 6;
        if (!Integer.valueOf(1).equals(1)) return 7;
        if (!doubleEquals(nan, Double.NaN) || !floatEquals(singleNan, Float.NaN)) return 8;
        if (doubleEquals(nan, "NaN") || floatEquals(singleNan, null)) return 9;
        if (nan.equals(null) || zero.equals(null)) return 10;
        Double absent = null;
        try { absent.equals(argument()); return 11; }
        catch (NullPointerException expected) { if (calls != 1) return 12; }
        Double before = 1.0;
        if (before.equals(before = 2.0) || !before.equals(2.0)) return 13;
        double primitiveNan = Double.NaN;
        if (primitiveNan == primitiveNan || !(0.0 == -0.0)) return 14;
        // The component-wise equals form used by production Quaternion.
        if (!((Double) primitiveNan).equals(primitiveNan)
            || ((Double) 0.0).equals(-0.0)
            || !((Float) Float.NaN).equals(Float.NaN)) return 15;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
