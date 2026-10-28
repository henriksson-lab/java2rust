import java.util.Arrays;
import java.util.List;
import java.util.function.Function;
import java.util.function.Predicate;
import java.util.stream.Collectors;
import java.util.stream.Stream;

public class StreamLifecycleContracts {
    static int visits;
    static int built;
    static int[] captureCounts;
    static String[] arrayValue;
    static boolean keep(String value) { visits++; return true; }
    static String identity(String value) { visits++; return value; }
    static Stream<String> source() { return Arrays.stream(new String[]{"first", null, "last"}); }
    static Function<String, String> callback() { built++; return StreamLifecycleContracts::identity; }
    static Stream<String> named(Stream<String> input) { return input.map(callback()); }
    static Stream<String> mapped(Stream<String> input) { return input.map(StreamLifecycleContracts::identity); }
    static Stream<String> escapingCapture() {
        int[] counter = {0};
        captureCounts = counter;
        Stream<String> input = source();
        return input.map(value -> { if (value == null) counter[0]++; return value; });
    }
    static String[] mirrorArray(String[] value) { return value; }
    static Stream<String[]> arrayStream() { return Stream.of(new String[][]{arrayValue, null}); }
    static int run() {
        Stream<String> original = source();
        Stream<String> alias = original;
        original.close();
        alias.close();
        try { alias.count(); return 1; } catch (IllegalStateException expected) {}

        Stream<String> upstream = source();
        Stream<String> mapped = upstream.map(StreamLifecycleContracts::identity);
        Stream<String> downstream = mapped.filter(StreamLifecycleContracts::keep);
        if (visits != 0) return 2;
        upstream.close();
        try { downstream.count(); return 3; } catch (IllegalStateException expected) {}
        if (visits != 0) return 4;
        try { mapped.count(); return 5; } catch (IllegalStateException expected) {}

        Stream<String> next = source();
        Stream<String> filtered = next.filter(StreamLifecycleContracts::keep);
        Stream<String> retained = filtered;
        filtered.close();
        try { retained.count(); return 6; } catch (IllegalStateException expected) {}
        try { next.count(); return 7; } catch (IllegalStateException expected) {}
        if (visits != 0) return 8;

        Stream<String> consumed = source();
        if (consumed.count() != 3) return 9;
        consumed.close();
        consumed.close();
        try { consumed.count(); return 10; } catch (IllegalStateException expected) {}
        Stream<String> active = source();
        Stream<String> activeMapped = active.map(StreamLifecycleContracts::identity);
        Stream<String> activeFiltered = activeMapped.filter(StreamLifecycleContracts::keep);
        List<String> result = activeFiltered.collect(Collectors.toList());
        if (result.size() != 3 || result.get(0) != "first" || result.get(1) != null || result.get(2) != "last" || visits != 6) return 11;
        Stream<String> captured = escapingCapture();
        int[] retainedCounter = captureCounts;
        retainedCounter[0] = 4;
        captureCounts = new int[]{90};
        List<String> capturedResult = captured.collect(Collectors.toList());
        if (capturedResult.get(1) != null || retainedCounter[0] != 5 || captureCounts[0] != 90) return 12;
        Stream<String> named = named(source());
        if (built != 1 || visits != 6) return 13;
        List<String> namedResult = named.collect(Collectors.toList());
        if (namedResult.get(1) != null || visits != 9) return 14;
        Stream<String> used = source();
        used.close();
        try { mapped(used); return 15; } catch (IllegalStateException expected) {}
        Stream<String> empty = Arrays.stream(new String[0]);
        try { empty.map((Function<String,String>) null); return 16; } catch (NullPointerException expected) {}
        if (empty.count() != 0) return 17;
        try { empty.filter((Predicate<String>) null); return 18; } catch (NullPointerException expected) {}
        int before = built;
        try { named(null); return 19; } catch (NullPointerException expected) {}
        if (built != before + 1) return 20;
        arrayValue = new String[]{"before"};
        String[] arrayAlias = arrayValue;
        Stream<String[]> arrays = arrayStream();
        Stream<String[]> mappedArrays = arrays.map(StreamLifecycleContracts::mirrorArray);
        arrayAlias[0] = "after";
        List<String[]> arrayResult = mappedArrays.collect(Collectors.toList());
        if (arrayResult.size() != 2 || arrayResult.get(0) != arrayAlias || arrayResult.get(1) != null) return 21;
        arrayResult.get(0)[0] = "returned";
        if (!"returned".equals(arrayAlias[0])) return 22;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
