import java.util.Arrays;
import java.util.EnumSet;
class EnumNativeContractsProbe {
    enum Choice {
        FIRST("first", "a", "b"), SECOND("second", "");
        final String label;
        final String[] allowed;
        String value;
        Choice(String label, String... allowed) {
            if (allowed.length == 0) throw new IllegalArgumentException("empty choices");
            this.label = label;
            this.allowed = allowed;
            this.value = label;
        }
        String get() { return value; }
        public String toString() { return value; }
        String[] allowedValues() { return allowed; }
    }
    static <E extends Enum<E>> int universeSize(Class<E> token) {
        return EnumSet.allOf(token).size();
    }
    static int run() {
        if (!"first".equals(Choice.FIRST.get())) return 1;
        Choice selected = Choice.FIRST;
        if (!selected.toString().equals("first")) return 2;
        if (selected.allowedValues().length != 2) return 3;
        if (!Arrays.asList(selected.allowedValues()).contains("a")) return 4;
        if (Arrays.asList(selected.allowed).contains("z")) return 5;
        EnumSet<Choice> seen = EnumSet.of(Choice.FIRST);
        int remaining = 0;
        for (Choice choice : EnumSet.complementOf(seen)) {
            if (choice != Choice.SECOND) return 6;
            remaining++;
        }
        if (remaining != 1 || !Choice.FIRST.toString().equals("first")) return 7;
        String copy = new String("first");
        Choice.FIRST.value = copy;
        if (Choice.FIRST.toString() != copy) return 8;
        if (universeSize(Choice.class) != 2) return 9;
        try { universeSize((Class<Choice>) null); return 10; }
        catch (NullPointerException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
