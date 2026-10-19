class NullableDescription {
    String text;
    NullableDescription(String value) { text = value; }
    public String toString() { return toString(false); }
    String toString(boolean empty) { return empty ? null : text; }
}
class ObjectStringContracts {
    static String nativeBuilder(boolean present) {
        StringBuilder builder = new StringBuilder();
        builder.append(" value ");
        return present ? builder.toString().trim() : "absent";
    }
    static String nativeBuffer() {
        StringBuffer buffer = new StringBuffer();
        buffer.append("buffer");
        return buffer.toString().toUpperCase();
    }
    static boolean nativeRandom() {
        java.util.Random random = new java.util.Random();
        return random.toString().startsWith("java.util.Random@");
    }
    static String description(NullableDescription value) {
        return value == null ? "missing" : value.toString(false);
    }
    static boolean nullableBranch() { return description(new NullableDescription(null)) == null; }
    static String presentBranch() { return description(new NullableDescription("present")); }
    static String absentBranch() { return description(null); }
    static <T> Object boxedString(T value) { return value.toString(); }
    static boolean boxedNullResult() { return boxedString(new NullObjectString()) == null; }
    static boolean boxedPresentResult() {
        Object value = boxedString("kept");
        return value.toString().equals("kept");
    }
}
class NullObjectString {
    public String toString() { return null; }
}
