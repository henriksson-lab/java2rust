class ForwardedPayload {
    int value;
}
class UnseededErasedForward {
    Object[][] matrix = new Object[1][1];
    static Object pass(Object value) { return value; }
    Object load() { return pass(matrix[0][0]); }
    static boolean run() { return new UnseededErasedForward().load() == null; }
}
class SeededErasedForward {
    Object[] values = new Object[1];
    static Object pass(Object value) { return value; }
    static Object nullSeed() { return pass(null); }
    Object load() { return pass(values[0]); }
    static boolean run() { return new SeededErasedForward().load() == null; }
}
class ErasedNullSink {
    static int calls;
    ErasedNullSink(Object value) { if (value == null) calls++; }
}
class ErasedArrayForwarding {
    Object[] values = new Object[1];
    Object[][] matrix = new Object[1][1];
    static int effects;
    static Object pass(Object value) { return value; }
    static <T> boolean genericNull(Object value) { return value == null; }
    static int primitive(int value) { return value; }
    static Object present() { effects++; return new ForwardedPayload(); }
    static Object presentChecked(boolean fail) {
        effects++;
        if (fail) throw new IllegalArgumentException();
        return new ForwardedPayload();
    }
    static int checkedComparisonEffects() {
        effects = 0;
        try {
            boolean absent = presentChecked(true) == null;
            return absent ? 8 : 9;
        } catch (IllegalArgumentException error) { return effects; }
    }
    static boolean presentOrThrow(boolean fail) {
        try {
            if (fail) throw new IllegalArgumentException();
            return present() != null;
        } catch (IllegalArgumentException error) { return true; }
    }
    static int comparisonEffects() {
        effects = 0;
        boolean absent = present() == null;
        boolean present = presentOrThrow(false);
        return effects * 10 + (absent ? 1 : 0) + (present ? 2 : 0);
    }
    static boolean nulls() {
        ErasedArrayForwarding owner = new ErasedArrayForwarding();
        return pass(owner.values[0]) == null && pass(owner.matrix[0][0]) == null
            && ErasedArrayForwarding.<String>genericNull(owner.matrix[0][0]);
    }
    static int aliasMutation() {
        ErasedArrayForwarding owner = new ErasedArrayForwarding();
        ForwardedPayload original = new ForwardedPayload();
        owner.values[0] = original;
        ForwardedPayload alias = (ForwardedPayload) pass(owner.values[0]);
        alias.value = 7;
        return original.value;
    }
    static int localAndConstructor() {
        Object[] local = new Object[1];
        ErasedNullSink.calls = 0;
        new ErasedNullSink(local[0]);
        return ErasedNullSink.calls;
    }
    static int primitiveNegative() {
        int[] values = new int[] { 3 };
        return primitive(values[0]);
    }
}
