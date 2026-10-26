package dimensions;
class ArrayLengthConsumer {
    static <T> int[][] make(ArrayLengthChild<T> source) {
        return new int[source.size()][source.checkedSize()];
    }
    static <T> int sizeOnly(ArrayLengthChild<T> source) { return source.size(); }
    static <T> int[] initialized(ArrayLengthChild<T> source) {
        return new int[] { source.size(), source.checkedSize() };
    }
    static <T> int[] braceInitialized(ArrayLengthChild<T> source) {
        int[] result = { source.size(), source.checkedSize() };
        return result;
    }
    static <T> int[][] negative(ArrayLengthChild<T> source) {
        return new int[source.shiftedSize()][source.checkedSize()];
    }
    static int run() {
        ArrayLengthChild<String> source = new ArrayLengthChild<>(new int[] { 1, 2 });
        if (make(source).length != 2 || source.reads != 1 || sizeOnly(source) != 2) return 1;
        if (initialized(source)[1] != 2 || source.reads != 2) return 7;
        if (braceInitialized(source)[0] != 2 || source.reads != 3) return 8;
        try { negative(source); return 2; }
        catch (NegativeArraySizeException expected) {}
        if (source.reads != 4) return 3;
        source.values = null;
        try { make(source); return 4; }
        catch (NullPointerException expected) {}
        try { sizeOnly(source); return 5; }
        catch (NullPointerException expected) {}
        try { initialized(source); return 9; }
        catch (NullPointerException expected) {}
        try { braceInitialized(source); return 10; }
        catch (NullPointerException expected) {}
        return source.reads == 4 ? 0 : 6;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
