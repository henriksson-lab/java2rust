class CachedObjectString {
    private final String cached;
    CachedObjectString(String value) { cached = value; }
    public String toString() { return cached; }
}
class ObjectStringIdentity {
    static boolean sourceResult() {
        Object value = new CachedObjectString(new String("kept"));
        String first = value.toString();
        String second = value.toString();
        return first == second;
    }
    static boolean nativeResult() {
        String cached = new String("kept");
        Object value = cached;
        return value.toString() == cached;
    }
    static String nullableString(boolean empty, int[] order) {
        order[0] = order[0] * 10 + 1;
        if (empty) return null;
        return new String("kept");
    }
    static String directString(int[] order) {
        order[0] = order[0] * 10 + 2;
        return new String("kept");
    }
    static int nullableOperandOrder() {
        int[] order = new int[] {0};
        boolean same = nullableString(true, order) == directString(order);
        return order[0] + (same ? 100 : 0);
    }
    static int reverseNullableOperandOrder() {
        int[] order = new int[] {0};
        boolean different = directString(order) != nullableString(true, order);
        return order[0] + (different ? 0 : 100);
    }
    static boolean twoNullableStrings() {
        int[] order = new int[] {0};
        String first = nullableString(true, order);
        String second = nullableString(true, order);
        String fresh = nullableString(false, order);
        String otherFresh = nullableString(false, order);
        return first == second && first != fresh && fresh != otherFresh;
    }
    static boolean snapshotNullableOperands() {
        int[] order = new int[] {0};
        String first = nullableString(false, order);
        String second = nullableString(false, order);
        boolean same = first == (first = second);
        return !same && first == second;
    }
}
