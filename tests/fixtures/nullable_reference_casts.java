class NullableReferenceCasts {
    static int effects;
    static Object countedArgument() { effects++; return "present"; }
    static int nullReceiverOrder() {
        effects = 0;
        try { nullSeed().equals(countedArgument()); return -1; }
        catch (NullPointerException error) { return effects; }
    }
    static <T> T cast(Object value) { return (T) value; }
    static <T> T throughLocal(Object value) { T local = (T) value; return local; }
    static <T> T assignedLocal(Object value) { T local = null; local = (T) value; return local; }
    static <T> T choose(boolean first, Object left, Object right) {
        return first ? (T) left : (T) right;
    }
    static <T> T element(Object[] values) { return (T) values[0]; }
    static <T> T nestedElement(Object[][] values) { return (T) values[0][0]; }
    static boolean arrayResult() { return NullableReferenceCasts.<String>element(new Object[1]) == null; }
    static boolean nestedArrayResult() { return NullableReferenceCasts.<String>nestedElement(new Object[1][1]) == null; }
    static boolean inferredNullResult() { return nullSeed() == null && localSeed() == null && assignedSeed() == null && chooseSeed() == null; }
    static Object inferredPresent() { return cast("present"); }
    static boolean inferredPresentResult() { return inferredPresent().equals("present"); }
    static boolean nullReceiverCaught() {
        try { nullSeed().equals("present"); return false; }
        catch (NullPointerException error) { return true; }
    }
    static Object nullSeed() { return cast(null); }
    static Object localSeed() { return throughLocal(null); }
    static Object assignedSeed() { return assignedLocal(null); }
    static Object chooseSeed() { return choose(true, null, new Object()); }
    static boolean nullResult() { return NullableReferenceCasts.<String>cast(null) == null; }
    static boolean localResult() { return NullableReferenceCasts.<String>throughLocal(null) == null; }
    static boolean assignedResult() { return NullableReferenceCasts.<String>assignedLocal(null) == null; }
    static boolean chosenResult() { return NullableReferenceCasts.<String>choose(true, null, "present") == null; }
    static boolean presentResult() { return NullableReferenceCasts.<String>cast("present").equals("present"); }
    static boolean badCast() {
        try { String value = NullableReferenceCasts.<String>cast(new Object()); return false; }
        catch (ClassCastException error) { return true; }
    }
    static int primitiveResult() { return (int) 3L; }
}
