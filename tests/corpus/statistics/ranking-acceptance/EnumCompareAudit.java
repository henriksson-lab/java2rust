public class EnumCompareAudit {
    enum Mode { FIRST, SECOND, THIRD }
    enum Loaded {
        FIRST(11), SECOND(22);
        private int tag;
        Loaded(int tag) { this.tag = tag; }
    }
    static class Other {
        int compareTo(Mode value) { return 17; }
    }
    static int calls;

    static Mode receiver(boolean absent) {
        calls = calls * 10 + 1;
        return absent ? null : Mode.THIRD;
    }
    static Mode argument(boolean fail) {
        calls = calls * 10 + 2;
        if (fail) throw new IllegalArgumentException("argument");
        return Mode.FIRST;
    }
    static Mode replace(Mode[] values) {
        values[0] = Mode.FIRST;
        return Mode.FIRST;
    }
    public static int nullable(Mode other) { return Mode.FIRST.compareTo(other); }
    public static int plain() { return Mode.FIRST.compareTo(Mode.THIRD); }

    public static int run() {
        Mode[] values = Mode.values();
        for (int i = 0; i < 3; ++i) {
            for (int j = 0; j < 3; ++j) {
                if (values[i].compareTo(values[j]) != i - j) return 1;
            }
        }
        calls = 0;
        try { receiver(true).compareTo(argument(false)); return 2; }
        catch (NullPointerException expected) { if (calls != 12) return 3; }
        calls = 0;
        try { receiver(true).compareTo(argument(true)); return 4; }
        catch (IllegalArgumentException expected) {
            if (calls != 12 || !"argument".equals(expected.getMessage())) return 5;
        }
        calls = 0;
        if (receiver(false).compareTo(argument(false)) != 2 || calls != 12) return 6;
        try { Mode.FIRST.compareTo((Mode) null); return 7; }
        catch (NullPointerException expected) { }
        try { nullable(null); return 8; }
        catch (NullPointerException expected) { }
        if (nullable(Mode.SECOND) != -1 || plain() != -2) return 9;
        Mode[] slot = { Mode.THIRD };
        if (slot[0].compareTo(replace(slot)) != 2 || slot[0] != Mode.FIRST) return 10;
        java.util.function.ToIntFunction<Mode> compare = Mode.THIRD::compareTo;
        if (compare.applyAsInt(Mode.FIRST) != 2) return 11;
        try { compare.applyAsInt(null); return 12; }
        catch (NullPointerException expected) { }
        if (Loaded.SECOND.compareTo(Loaded.FIRST) != 1) return 13;
        if (new Other().compareTo(null) != 17) return 14;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
