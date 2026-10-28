import java.util.List;
import java.util.Arrays;
import java.util.stream.Stream;
import java.util.stream.Collectors;

public class ArrayStreamContracts {
    static String[] values;
    static Stream<String> saved() { return Stream.of(values); }
    static Stream<String> fromParameter(String[] input) { return Stream.of(input); }
    static Stream<String> localMade() {
        String[] input = {"before", null};
        Stream<String> stream = Arrays.stream(input);
        input[0] = "after";
        return stream;
    }
    static Stream<String> freshArray() { return Arrays.stream(new String[]{"fresh", null, "\ud800"}); }
    static Stream<String> freshVarargsArray() { return Stream.of(new String[]{"varargs", null}); }
    static Stream<String> defaultArray() { return Arrays.stream(new String[2]); }
    static long consume(Stream<String> stream) { return stream.count(); }
    static boolean visit(String value) {
        if ("first".equals(value)) values[1] = "changed";
        return "changed".equals(value);
    }
    static int run() {
        values = new String[]{"first", "old", null, "\ud800"};
        Stream<String> stream = saved();
        values[1] = "later";
        if (!stream.anyMatch(ArrayStreamContracts::visit)) return 1;
        try { stream.count(); return 2; }
        catch (IllegalStateException expected) { }
        Stream<String> next = saved();
        values[0] = "new";
        List<String> result = next.collect(Collectors.toList());
        if (!"new".equals(result.get(0)) || result.get(2) != null || !"\ud800".equals(result.get(3))) return 3;
        result.set(0, "detached");
        if (!"new".equals(values[0])) return 4;
        String[] local = {"local", null};
        Stream<String> localStream = fromParameter(local);
        local[0] = "alias";
        List<String> localResult = localStream.collect(Collectors.toList());
        if (!"alias".equals(localResult.get(0)) || localResult.get(1) != null) return 6;
        List<String> escaped = localMade().collect(Collectors.toList());
        if (!"after".equals(escaped.get(0)) || escaped.get(1) != null) return 7;
        try { consume(null); return 8; }
        catch (NullPointerException expected) { }
        Stream<String> original = saved();
        Stream<String> alias = original;
        if (consume(original) != 4) return 9;
        try { consume(alias); return 10; }
        catch (IllegalStateException expected) { }
        List<String> fresh = freshArray().collect(Collectors.toList());
        if (!"fresh".equals(fresh.get(0)) || fresh.get(1) != null || !"\ud800".equals(fresh.get(2))) return 11;
        List<String> varargs = freshVarargsArray().collect(Collectors.toList());
        if (!"varargs".equals(varargs.get(0)) || varargs.get(1) != null) return 12;
        List<String> defaults = defaultArray().collect(Collectors.toList());
        if (defaults.size() != 2 || defaults.get(0) != null || defaults.get(1) != null) return 13;
        values = null;
        try { saved(); return 5; }
        catch (NullPointerException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
