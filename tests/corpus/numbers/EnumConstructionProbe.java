public class EnumConstructionProbe {
    static int events;

    static int mark(int tag) {
        events = events * 10 + tag;
        return tag;
    }

    static int observe() {
        return Mode.FIRST == null ? -1 : Mode.FIRST.code;
    }

    enum Mode {
        FIRST(mark(1)), ALIAS(FIRST), SECOND(mark(2));

        final int code;
        final int index;
        final int seen = observe();
        int calls = mark(5);

        Mode(int code) {
            this.code = code;
            index = ordinal();
            mark(3);
        }

        Mode(Mode alias) {
            this(alias.code);
            mark(4);
        }

        int apply(int value) {
            calls++;
            return code + value;
        }

        int next() { return ++calls; }
    }

    static int run() {
        if (Mode.FIRST.apply(5) != 6 || Mode.ALIAS.apply(8) != 9 || Mode.SECOND.apply(10) != 12) return 1;
        if (events != 153534253) return 2;
        if (Mode.FIRST == Mode.ALIAS || Mode.FIRST.code != Mode.ALIAS.code) return 3;
        if (Mode.FIRST.index != 0 || Mode.ALIAS.index != 1 || Mode.SECOND.index != 2) return 4;
        if (Mode.FIRST.seen != -1 || Mode.ALIAS.seen != 1 || Mode.SECOND.seen != 1) return 5;
        Mode saved = Mode.FIRST;
        if (saved.next() != 7 || Mode.FIRST.calls != 7 || Mode.ALIAS.calls != 6) return 6;
        if (events != 153534253 || !Mode.ALIAS.name().equals("ALIAS") || Mode.values().length != 3) return 7;
        return 0;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
