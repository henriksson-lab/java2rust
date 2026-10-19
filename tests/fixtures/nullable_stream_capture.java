import java.util.Arrays;
import java.util.Set;
class NullableStreamCapture {
    static int calls;
    static void reset() { calls = 0; }
    static int getCalls() { return calls; }
    static int tick(int value) { calls++; return value; }
    static int fail(int value) { throw new IllegalArgumentException("argument"); }
    static boolean any(int[] values, Set<Integer> accepted) {
        return Arrays.stream(values).anyMatch(value -> accepted.contains(value));
    }
    static boolean all(int[] values, Set<Integer> accepted) {
        return Arrays.stream(values).allMatch(value -> accepted.contains(value));
    }
    static boolean anyNull(int[] values) { return any(values, null); }
    static boolean allNull(int[] values) { return all(values, null); }
    static boolean anyOrdered(int[] values, Set<Integer> accepted) {
        return Arrays.stream(values).anyMatch(value -> accepted.contains(tick(value)));
    }
    static boolean orderedNull(int[] values) { return anyOrdered(values, null); }
    static boolean argumentFailure(int[] values, Set<Integer> accepted) {
        return Arrays.stream(values).anyMatch(value -> accepted.contains(fail(value)));
    }
    static boolean argumentFailureNull(int[] values) { return argumentFailure(values, null); }
    static boolean anyNullable(String[] values) {
        return Arrays.stream(values).anyMatch(String::isEmpty);
    }
    static boolean allNullable(String[] values) {
        return Arrays.stream(values).allMatch(String::isEmpty);
    }
    static boolean anyNullElement() { return anyNullable(new String[] { null }); }
    static boolean allNullElement() { return allNullable(new String[] { null }); }
    static boolean caughtNullable(String[] values) {
        try { return Arrays.stream(values).anyMatch(String::isEmpty); }
        catch (NullPointerException ignored) { return false; }
    }
    static boolean caughtNullElement() { return caughtNullable(new String[] { null }); }
    static boolean acceptsAny(String value) { return true; }
    static boolean staticReference(String[] values) {
        return Arrays.stream(values).anyMatch(NullableStreamCapture::acceptsAny);
    }
    static boolean staticNullElement() { return staticReference(new String[] { null }); }
    static boolean shadowed(int[] values, Set<Integer> accepted) {
        return Arrays.stream(values).anyMatch(__java_call_receiver -> accepted.contains(__java_call_receiver));
    }
    static boolean shadowedNull(int[] values) { return shadowed(values, null); }
    static boolean acceptsLambda(String value) { return true; }
    static boolean lambdaReference(String[] values) {
        return Arrays.stream(values).anyMatch(value -> acceptsLambda(value));
    }
    static boolean lambdaNullElement() { return lambdaReference(new String[] { null }); }
    static boolean filteredReference(String[] values) {
        return Arrays.stream(values).filter(java.util.Objects::nonNull).anyMatch(String::isEmpty);
    }
    static boolean filteredNullElement() { return filteredReference(new String[] { null }); }
}
