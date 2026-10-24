public final class NullableArrayCreationProbe {
    enum Mode {
        FIRST(1), SECOND(2);
        private final int marker;
        Mode(int marker) { this.marker = marker; }
        int count(int[] key, int[]... rows) { return marker + key.length + rows.length; }
    }
    public static int run() {
        if (Mode.FIRST.count(new int[]{1},new int[0][]) != 2) return 1;
        if (Mode.SECOND.count(new int[]{1,2},new int[3][]) != 7) return 2;
        try { Mode.FIRST.count(new int[]{1},(int[][])null); return 3; }
        catch (NullPointerException expected) { }
        try { Mode.SECOND.count((int[])null,new int[0][]); return 4; }
        catch (NullPointerException expected) { }
        try { Mode.FIRST.count(new int[]{1},new int[-1][]); return 5; }
        catch (NegativeArraySizeException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
