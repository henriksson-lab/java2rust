import java.util.Arrays;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import java.util.stream.Collectors;

class PipelineCheck {
    boolean keep(int value) { return FallibleStreamPipeline.keep(value); }
}

class FallibleStreamPipeline {
    static int visits;
    static int mapped;
    static int consumed;
    static void reset() { visits = 0; mapped = 0; consumed = 0; }
    static int visits() { return visits; }
    static int mapped() { return mapped; }
    static int consumed() { return consumed; }
    static boolean keep(int value) {
        visits++;
        if (value < 0) throw new IllegalArgumentException("negative");
        return value != 0;
    }
    static int twice(int value) {
        mapped++;
        if (value == 13) throw new IllegalStateException("mapping");
        return value * 2;
    }
    static void consume(int value) {
        if (value == 26) throw new IllegalStateException("consumer");
        consumed += value;
    }
    static boolean any(int[] values) {
        return Arrays.stream(values).filter(FallibleStreamPipeline::keep)
            .map(FallibleStreamPipeline::twice).filter(value -> value > 0)
            .anyMatch(value -> value >= 4);
    }
    static boolean all(int[] values) {
        return Arrays.stream(values).map(FallibleStreamPipeline::twice)
            .filter(value -> keep(value)).allMatch(value -> value < 4);
    }
    static boolean bound(int[] values) {
        PipelineCheck check = new PipelineCheck();
        return Arrays.stream(values).map(FallibleStreamPipeline::twice)
            .filter(check::keep).anyMatch(value -> value >= 4);
    }
    static int sum(int[] values) {
        return Arrays.stream(values).filter(FallibleStreamPipeline::keep)
            .map(FallibleStreamPipeline::twice).sum();
    }
    static long count(int[] values) {
        return Arrays.stream(values).filter(FallibleStreamPipeline::keep)
            .map(FallibleStreamPipeline::twice).count();
    }
    static int first(int[] values) {
        return Arrays.stream(values).filter(FallibleStreamPipeline::keep)
            .map(FallibleStreamPipeline::twice).findFirst().orElse(-1);
    }
    static List<Integer> collected(int[] values) {
        return Arrays.stream(values).filter(FallibleStreamPipeline::keep)
            .mapToObj(FallibleStreamPipeline::twice).collect(Collectors.toList());
    }
    static void each(int[] values) {
        Arrays.stream(values).filter(FallibleStreamPipeline::keep)
            .map(value -> value * 2).forEach(FallibleStreamPipeline::consume);
    }
    static void eachBlock(int[] values) {
        Arrays.stream(values).filter(FallibleStreamPipeline::keep)
            .map(value -> value * 2).forEach(value -> { consume(value); });
    }
    static String maybe(int value) {
        if (value < 0) throw new IllegalArgumentException("negative");
        return value == 0 ? null : "x";
    }
    static int nonNull(int[] values) {
        return Arrays.stream(values).mapToObj(FallibleStreamPipeline::maybe)
            .filter(Objects::nonNull).mapToInt(String::length).sum();
    }
    static String maybePlain(int value) { return value == 0 ? null : "x"; }
    static int nonNullPlain(int[] values) {
        return Arrays.stream(values).mapToObj(FallibleStreamPipeline::maybePlain)
            .filter(Objects::nonNull).mapToInt(String::length).sum();
    }
    static boolean caught(int[] values) {
        try { return any(values); }
        catch (IllegalArgumentException error) { return false; }
    }
    static boolean mutating(int[] values, Set<Integer> seen) {
        return Arrays.stream(values).anyMatch(value -> seen.add(value));
    }
    static boolean mutatingNull(int[] values) { return mutating(values, null); }
}
