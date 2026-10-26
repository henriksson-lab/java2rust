import java.util.function.Function;

/** The intermediate array is absent from the composed Function<String,String>. */
public final class GenericCompositionAudit {
    static final class Parse implements Function<String, int[]> {
        int calls;
        public int[] apply(String input) {
            calls++;
            if ("fail".equals(input)) throw new IllegalArgumentException("parse");
            return new int[] {Integer.parseInt(input)};
        }
    }
    static final class Format implements Function<int[], String> {
        int calls;
        int[] last;
        public String apply(int[] input) {
            calls++;
            if (input[0] < 0) throw new IllegalStateException("format");
            last = input;
            input[0] += 2;
            return String.valueOf(input[0]);
        }
    }
    public static int run() {
        Parse parse = new Parse();
        Format format = new Format();
        Function<String, int[]> first = parse;
        Function<int[], String> second = format;
        Function<String, String> after = first.andThen(second);
        Function<String, String> before = second.compose(first);
        if (parse.calls != 0 || format.calls != 0 || after == before) return 1;
        if (!"7".equals(after.apply("5")) || format.last[0] != 7) return 2;
        if (!"10".equals(before.apply("8"))) return 3;
        if (parse.calls != 2 || format.calls != 2) return 4;
        try { first.andThen((Function<int[], String>) null); return 5; }
        catch (NullPointerException expected) {
            if (parse.calls != 2 || format.calls != 2) return 6;
        }
        try { second.compose((Function<String, int[]>) null); return 7; }
        catch (NullPointerException expected) {
            if (parse.calls != 2 || format.calls != 2) return 8;
        }
        try { after.apply("fail"); return 9; }
        catch (IllegalArgumentException expected) {
            if (!"parse".equals(expected.getMessage()) || parse.calls != 3 || format.calls != 2) return 10;
        }
        try { before.apply("-1"); return 11; }
        catch (IllegalStateException expected) {
            if (!"format".equals(expected.getMessage()) || parse.calls != 4 || format.calls != 3) return 12;
        }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
