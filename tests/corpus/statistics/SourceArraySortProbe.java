import java.util.function.IntUnaryOperator;

public final class SourceArraySortProbe {
    static final class Arrays {
        static void sort(int[] values, IntUnaryOperator transform) {
            values[0] = transform.applyAsInt(values[0]) + 7;
        }
    }
    public static int run() {
        int[] values = {2};
        Arrays.sort(values, x -> x * 3);
        return values[0] == 13 ? 0 : 1;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
