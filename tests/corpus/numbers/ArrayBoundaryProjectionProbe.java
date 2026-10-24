import java.util.Arrays;

public final class ArrayBoundaryProjectionProbe {
    interface View { int read(); }
    static final class Counter implements View {
        int n;
        public int read() { return n; }
        void step() { n++; }
    }
    static final class Other implements View { public int read() { return -1; } }
    static boolean counter(View value) { return value instanceof Counter; }
    static int counterValue(View value) {
        if (value instanceof Counter found) return found.read();
        return -2;
    }
    static int scan(int remaining) {
        for (;;) {
            if (remaining == 0) return 23;
            remaining--;
        }
    }
    static int labeled() {
        int i = 0;
        scan: for (;;) {
            if (++i < 3) continue scan;
            break scan;
        }
        return i;
    }
    static final class Sizes {
        final int[] values;
        Sizes(int[] values) { this.values = values; }
        void set(int value) { values[0] = value; }
        public String toString() { return Arrays.toString(values); }
    }
    static int[] shared = new int[]{7,8};
    static String sharedText() { return Arrays.toString(shared); }
    public static int run() {
        if (scan(0) != 23 || scan(4) != 23 || labeled() != 3) return 1;
        if (counter(null) || counter(new Other())) return 2;
        Counter value = new Counter(); value.step();
        if (!counter(value) || counterValue(value) != 1) return 3;
        if (counterValue(null) != -2 || counterValue(new Other()) != -2) return 4;
        int[] values = {1,2};
        Sizes sizes = new Sizes(values);
        if (!sizes.toString().equals("[1, 2]")) return 5;
        values[1] = 9; sizes.set(-3);
        if (!sizes.toString().equals("[-3, 9]")) return 6;
        Sizes absent = new Sizes(null);
        if (!absent.toString().equals("null")) return 7;
        if (!sharedText().equals("[7, 8]")) return 8;
        shared[0] = 4;
        if (!sharedText().equals("[4, 8]")) return 9;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
