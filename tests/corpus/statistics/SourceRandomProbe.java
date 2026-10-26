import java.util.function.IntUnaryOperator;

public final class SourceRandomProbe {
    static final class SplittableRandom {
        int state;
        int nextInt(int bound) { state += bound + 100; return state; }
    }
    public static int run() {
        SplittableRandom source = new SplittableRandom();
        IntUnaryOperator bound = source::nextInt;
        if (bound.applyAsInt(0) != 100) return 1;
        if (source.nextInt(2) != 202) return 2;
        java.util.SplittableRandom platform = new java.util.SplittableRandom(42L);
        IntUnaryOperator nativeDraw = platform::nextInt;
        if (nativeDraw.applyAsInt(1) != 0) return 3;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
