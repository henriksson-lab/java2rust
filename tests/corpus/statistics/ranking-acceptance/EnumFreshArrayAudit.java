public class EnumFreshArrayAudit {
    enum Mode {
        FIRST, SECOND, THIRD;

        static Mode[] snapshot() { return values(); }
    }

    public static int run() {
        Mode[] first = Mode.values();
        Mode[] alias = first;
        Mode[] second = Mode.values();
        if (first == second || alias != first) return 1;
        first[0] = null;
        if (alias[0] != null || second[0] != Mode.FIRST) return 2;
        Mode[] third = Mode.snapshot();
        if (third == first || third == second || third[0] != Mode.FIRST) return 3;
        third[1] = Mode.FIRST;
        if (second[1] != Mode.SECOND) return 4;
        if (Mode.FIRST.compareTo(Mode.THIRD) != -2) return 5;
        if (Mode.THIRD.compareTo(Mode.FIRST) != 2) return 6;
        return 0;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
