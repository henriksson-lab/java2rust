import java.util.Arrays;
import java.util.Objects;
import java.util.function.IntUnaryOperator;
import java.util.function.IntFunction;

public class ArraySetAllContracts {
    static int[] held;
    static String[] text;
    static int evaluations;
    static int[] target() { evaluations++; return held; }
    static int readAndChange(int i) {
        if (i == 0) held[1] = 7;
        return held[i] + 1;
    }
    static int fail(int i) {
        if (i == 2) throw new IllegalStateException("stop");
        return held[i] + 10;
    }
    static IntUnaryOperator generator() {
        evaluations += 10;
        held = new int[]{-9, -9, -9};
        return i -> 100 + i;
    }
    static int indexed(int i) { return 20 + i; }
    static int run() {
        held = new int[]{1, 2, 3};
        Arrays.setAll(target(), i -> readAndChange(i));
        if (evaluations != 1 || held[0] != 2 || held[1] != 8 || held[2] != 4) return 1;
        text = new String[]{"\ud800", null};
        Arrays.setAll(text, i -> Objects.toString(text[i], null));
        if (!text[0].equals("\ud800") || text[1] != null) return 2;
        try { Arrays.setAll(held, i -> fail(i)); return 3; }
        catch (IllegalStateException expected) { }
        if (held[0] != 12 || held[1] != 18 || held[2] != 4) return 4;
        int[] original = held;
        Arrays.setAll(target(), generator());
        if (evaluations != 12 || original[0] != 100 || original[2] != 102 || held[0] != -9) return 5;
        IntUnaryOperator absent = null;
        try { Arrays.setAll(new int[0], absent); return 6; }
        catch (NullPointerException expected) { }
        held = null;
        try { Arrays.setAll(target(), generator()); return 7; }
        catch (NullPointerException expected) { }
        if (evaluations != 23 || held[0] != -9) return 8;
        int[] local = {1, 2};
        Arrays.setAll(local, i -> { if (i == 0) local[1] = 9; return local[i] + 1; });
        if (local[0] != 2 || local[1] != 10) return 9;
        Arrays.setAll(local, ArraySetAllContracts::indexed);
        if (local[0] != 20 || local[1] != 21) return 10;
        IntFunction<String> texts = i -> i == 1 ? null : "\ud800";
        Arrays.setAll(text, texts);
        if (!text[0].equals("\ud800") || text[1] != null) return 11;
        Arrays.fill(text, (String)null);
        if (text[0] != null || text[1] != null) return 12;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
