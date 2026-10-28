public class ErasedStorageContracts {
    private Object held;
    private String[] typed;
    private static String[] shared;
    private static String[] copiedShared;
    ErasedStorageContracts(String[] values) { Object local = values; this.held = local; this.typed = values; }
    static Object erase(String[] values) { Object local = values; return local; }
    static Object direct(String[] values) { return values; }
    Object value() { return held; }
    Object typedValue() { return typed; }
    Object typedLocal() { Object local = typed; return local; }
    static String[] relay(String[] values) { return values; }
    static Object viaCall(String[] values) { Object local = relay(values); return local; }
    static Object directCall(String[] values) { return relay(values); }
    static Object primitive(int[] values) { return values; }
    static void initializeShared() { shared = new String[]{"shared"}; }
    static Object synchronizedValue() { return shared; }
    static String[] checkedProducer(boolean fail) {
        if (fail) throw new IllegalStateException("producer");
        return new String[]{"checked"};
    }
    static Object checked(boolean fail) { return checkedProducer(fail); }
    static String[] emptyStrings() { return new String[0]; }
    static Object erasedEmptyStrings() { return emptyStrings(); }
    static String[] nullStrings() { return new String[3]; }
    static Object erasedNullStrings() { return nullStrings(); }
    static String[][] emptyRows() { return new String[0][]; }
    static Object erasedRows() { return emptyRows(); }
    static String[][] nullRows() { return new String[2][]; }
    static Object erasedNullRows() { return nullRows(); }
    static char[] units() { return "a".toCharArray(); }
    static Object erasedUnits() { return units(); }
    static String[] cloneStrings(String[] values) { return values.clone(); }
    static String[] copyStrings(String[] values) { return java.util.Arrays.copyOf(values, 5); }
    static String[] rangeStrings(String[] values) { return java.util.Arrays.copyOfRange(values, 1, 4); }
    static Object erasedCloneStrings() { return cloneStrings(nullStrings()); }
    static Object erasedCopyStrings() { return copyStrings(nullStrings()); }
    static Object erasedRangeStrings() { return rangeStrings(nullStrings()); }
    static <T> T[] cloneGeneric(T[] values) { return values.clone(); }
    static Object erasedGenericClone() { return cloneGeneric(nullStrings()); }
    static <T> T[] copyGeneric(T[] values) { return java.util.Arrays.copyOf(values, 4); }
    static Object erasedGenericCopy() { return copyGeneric(nullStrings()); }
    static <T> T[] cloneGenericSync(T[] values) { return values.clone(); }
    static <T> T[] relayGenericSync(T[] values) { return cloneGenericSync(values); }
    static void copyShared() { copiedShared = relayGenericSync(shared); }
    static Object erasedSharedCopy() { return copiedShared; }
    static <T> T[] cloneOrAlias(T[] values, boolean copy) { return copy ? values.clone() : values; }
    static Object erasedConditionalClone() { return cloneOrAlias(nullStrings(), true); }
    static Object[] objectValues(String[] values) { return new Object[]{values, "text", null}; }
    static Object erasedObjects(String[] values) { return objectValues(values); }
    static Object absent() { return erase(null); }
}
