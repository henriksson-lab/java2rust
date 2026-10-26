public class ConditionalArrayWideningAudit {
    private static int reads;

    private static int index(int position) {
        reads++;
        if (position == -7) throw new IllegalStateException("index");
        return position;
    }

    public static double ints(int size, int[] values, int from) {
        return size == 0 ? Double.NaN : values[index(from)];
    }

    public static double longs(int size, long[] values, int from) {
        return size == 0 ? Double.NaN : values[index(from)];
    }

    public static double floats(boolean empty, float[] values) {
        return empty ? Double.NaN : values[index(0)];
    }

    public static int run() {
        reads = 0;
        if (!Double.isNaN(ints(0, null, 7)) || reads != 0) return 1;
        if (!Double.isNaN(longs(0, null, 7)) || reads != 0) return 2;
        if (ints(1, new int[] {-2147483648}, 0) != -2147483648.0) return 3;
        if (longs(1, new long[] {9007199254740993L}, 0) != 9007199254740992.0) return 4;
        if (floats(false, new float[] {-1.5f}) != -1.5 || reads != 3) return 5;
        try {
            ints(1, new int[] {2}, 1);
            return 6;
        } catch (ArrayIndexOutOfBoundsException expected) {
            if (reads != 4) return 7;
        }
        try {
            longs(1, null, 0);
            return 8;
        } catch (NullPointerException expected) {
            if (reads != 5) return 9;
        }
        try {
            ints(1, null, -7);
            return 10;
        } catch (IllegalStateException expected) {
            if (reads != 6) return 11;
        }
        return 0;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
