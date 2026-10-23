interface AnonymousHelperOperation { int get(); }

class AnonymousHelperCallProbe {
    static int helper(int value) { return -1000; }
    static AnonymousHelperOperation make(int divisor) {
        return new AnonymousHelperOperation() {
            int saved = divisor;
            int helper(int value) { return 42 / value; }
            int widen(double value) { return (int) value; }
            public int get() { return helper(saved) + widen(0); }
        };
    }
    static int run() {
        if (make(2).get() != 21) return 1;
        try { make(0).get(); return 2; }
        catch (ArithmeticException error) {
            if (!"/ by zero".equals(error.getMessage())) return 3;
        }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
