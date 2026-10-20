class PrimitiveValueFactories {
    static int run() {
        byte b = Byte.valueOf((byte)3);
        short s = Short.valueOf((short)4);
        int i = Integer.valueOf(5);
        long l = Long.valueOf(6);
        float f = Float.valueOf(7);
        double d = Double.valueOf(-0.0);
        char c = Character.valueOf('x');
        boolean truth = Boolean.valueOf(true);
        if (b != 3 || s != 4 || i != 5 || l != 6 || f != 7 || c != 'x' || !truth) return 1;
        if (Double.doubleToLongBits(d) != Long.MIN_VALUE) return 2;
        if (Double.valueOf("2.5") != 2.5 || Float.valueOf("3.5") != 3.5f) return 3;
        if (Boolean.valueOf("false") || !Boolean.valueOf("TrUe")) return 4;
        Double nullable = 9.0;
        if (Double.valueOf(nullable) != 9.0) return 5;
        nullable = null;
        try { Double.valueOf(nullable); return 6; }
        catch (NullPointerException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
