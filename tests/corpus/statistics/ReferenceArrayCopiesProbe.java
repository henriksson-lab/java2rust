import java.util.Arrays;

public final class ReferenceArrayCopiesProbe {
    static final class Value { int n; Value(int n) { this.n = n; } }
    static Value[] copy(Value[] values, int length) {
        Value[] result = Arrays.copyOf(values, length);
        return result;
    }
    static Value[] choose(boolean first, Value[] values) {
        return first ? copy(values, 3) : Arrays.copyOfRange(values, 1, 4);
    }
    static int change(Value[] values) { values[0].n = 17; return 3; }
    static Value[] shrink(boolean truncate, Value[] input) {
        Value[] result = new Value[input.length];
        for (int i = 0; i < input.length; i++) result[i] = input[i];
        return truncate ? Arrays.copyOf(result, 1) : result;
    }
    public static int run() {
        Value a = new Value(1);
        Value b = new Value(2);
        Value[] original = {a, b};
        Value[] first = choose(true, original);
        Value[] second = choose(false, original);
        if (first == original || second == original || first == second) return 1;
        if (first.length != 3 || first[0] != a || first[1] != b || first[2] != null) return 2;
        if (second.length != 3 || second[0] != b || second[1] != null || second[2] != null) return 3;
        first[0].n = 9;
        if (original[0].n != 9) return 4;
        first[1] = null;
        if (original[1] != b || second[0] != b) return 5;
        Value[] changed = Arrays.copyOf(original, change(original));
        if (changed[0] != a || changed[0].n != 17) return 6;
        int[] row = {3};
        int[][] rows = {row, row};
        int[][] copied = Arrays.copyOf(rows, 3);
        if (copied == rows || copied[0] != row || copied[1] != row || copied[2] != null) return 7;
        copied[0][0] = 21;
        if (rows[1][0] != 21) return 8;
        String[] text = {"x"};
        String[] padded = Arrays.copyOf(text, 2);
        if (padded[0] != text[0] || padded[1] != null) return 9;
        Value[] full = shrink(false, original);
        Value[] shortCopy = shrink(true, original);
        if (full == original || shortCopy == original || full.length != 2 || shortCopy.length != 1) return 10;
        if (full[0] != a || full[1] != b || shortCopy[0] != a) return 11;
        full[0] = null;
        if (original[0] != a || shortCopy[0] != a) return 12;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
