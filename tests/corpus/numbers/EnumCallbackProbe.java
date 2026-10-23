import java.util.function.ToIntFunction;
import java.util.function.BiPredicate;

/** Enum callback payloads, lexical field/method names, and mutex reentry. */
public class EnumCallbackProbe {
    enum Mode {
        NORMAL(Defaults.READ, Defaults.TEST), CONSTANT(Fixed.READ, Fixed.TEST);
        private final ToIntFunction<Entry> read;
        private final BiPredicate<Entry, Integer> test;
        Mode(ToIntFunction<Entry> read, BiPredicate<Entry, Integer> test) {
            this.read = read;
            this.test = test;
        }
        int read(Entry value) { return read.applyAsInt(value); }
        boolean test(Entry value, int expected) { return test.test(value, expected); }
        private static class Defaults {
            static final ToIntFunction<Entry> READ = value -> value.number;
            static final BiPredicate<Entry, Integer> TEST = (value, expected) -> value.number == expected;
        }
        private static class Fixed {
            static final ToIntFunction<Entry> READ = value -> 1;
            static final BiPredicate<Entry, Integer> TEST = (value, expected) -> true;
        }
    }
    static class Entry {
        static final Entry DEFAULT = new Entry(9);
        final int number;
        Mode mode;
        Entry(int number) { this.number = number; mode = Mode.NORMAL; }
        int read() { return mode.read(this); }
        boolean test(int expected) { return mode.test(this, expected); }
        int kind() {
            switch (mode) {
            case NORMAL: return 10;
            case CONSTANT: return 20;
            default: return 30;
            }
        }
    }
    enum Counter {
        A(4), B(8);
        int value;
        int value_ = 100;
        Counter(int value) { this.value = value; }
        int value(int extra) { return value + extra; }
        int advance() { return value++; }
    }
    static int choose(Mode mode) {
        switch (mode) {
        case NORMAL: return 1;
        default: return 2;
        }
    }
    public static int run() {
        Entry value = Entry.DEFAULT;
        Entry alias = value;
        for (int i=0; i<3; i++) {
            if (value.read() != 9 || !value.test(9) || value.test(8) || value.kind() != 10) return 1;
        }
        value.mode = Mode.CONSTANT;
        if (alias != value || alias.read() != 1 || !alias.test(-1) || alias.kind() != 20) return 2;
        value.mode = null;
        try { value.kind(); return 3; } catch (NullPointerException expected) { }
        try { value.read(); return 4; } catch (NullPointerException expected) { }
        value.mode = Mode.NORMAL;
        if (value.read() != 9 || value.kind() != 10) return 5;
        if (Counter.A.value(3) != 7 || Counter.A.advance() != 4 || Counter.A.value(3) != 8) return 6;
        if (Counter.A.value_ != 100 || Counter.B.value(0) != 8) return 7;
        if (choose(Mode.NORMAL) != 1 || choose(Mode.CONSTANT) != 2) return 8;
        try { choose(null); return 9; } catch (NullPointerException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
