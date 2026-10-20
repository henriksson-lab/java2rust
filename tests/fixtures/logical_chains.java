class LogicalChains {
    static int calls;
    static boolean mark(boolean value) { calls++; return value; }
    static boolean checked(boolean value, boolean fail) {
        calls++;
        if (fail) throw new IllegalArgumentException("must be skipped");
        return value;
    }
    static int run() {
        calls = 0;
        boolean a = mark(true) && mark(true) && mark(false) && mark(true)
            && mark(true) && mark(true) && mark(true) && mark(true);
        int first = calls;
        calls = 0;
        boolean b = mark(false) || mark(false) || mark(false) || mark(true)
            || mark(false) || mark(false) || mark(false) || mark(false);
        int second = calls;
        calls = 0;
        boolean c = checked(false, false) && checked(true, true) && checked(true, true);
        boolean d = checked(true, false) || checked(false, true) || checked(false, true);
        return !a && b && !c && d && first == 3 && second == 4 && calls == 2 ? 1 : 0;
    }
}
