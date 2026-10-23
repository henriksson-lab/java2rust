package conditioneffects;

public class ConditionalEffectsProbe {
    static int trace;
    static int mark(int x) { trace = trace * 10 + x; return x; }
    static int choose(int x, int divisor) { return mark(x) % mark(divisor) == 0 ? 1 : 2; }
    static int pass(int x) { return x; }
    static int argument(int x, int divisor) { return pass(mark(x) % mark(divisor) == 0 ? 1 : 2); }
    static int branch(int divisor) { return divisor != 0 ? mark(8) % mark(divisor) : mark(5); }
    public static int run() {
        trace = 0;
        if (choose(6,3) != 1 || trace != 63) return 1;
        trace = 0;
        if (choose(7,3) != 2 || trace != 73) return 2;
        trace = 0;
        if (argument(6,3) != 1 || trace != 63) return 3;
        trace = 0;
        if (argument(7,3) != 2 || trace != 73) return 4;
        trace = 0;
        try { choose(7,0); return 5; } catch (ArithmeticException expected) { }
        if (trace != 70) return 6;
        trace = 0;
        try { argument(7,0); return 7; } catch (ArithmeticException expected) { }
        if (trace != 70) return 8;
        trace = 0;
        if (branch(3) != 2 || trace != 83) return 9;
        trace = 0;
        if (branch(0) != 5 || trace != 5) return 10;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
