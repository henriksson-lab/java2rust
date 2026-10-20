class ErasedRowRead {
    Object[][] matrix = new Object[1][1];
    boolean empty() { return matrix[0][0] == null; }
    static boolean run() { return new ErasedRowRead().empty(); }
    boolean copiedEmpty() {
        Object[][] copy = matrix.clone();
        return copy[0][0] == null;
    }
    static boolean cloneRun() { return new ErasedRowRead().copiedEmpty(); }
    boolean copiedLeafEmpty() {
        Object[] copy = matrix[0].clone();
        return copy[0] == null;
    }
    static boolean leafCloneRun() { return new ErasedRowRead().copiedLeafEmpty(); }
}
class NullableArrayRows {
    Object[][] matrix = new Object[1][];
    boolean absent() { return matrix[0] == null; }
    boolean caught() {
        try { return matrix[0][0] != null; }
        catch (NullPointerException error) { return true; }
    }
    static boolean absentResult() { return new NullableArrayRows().absent(); }
    static boolean caughtResult() { return new NullableArrayRows().caught(); }
}
class RetainedArrayRows {
    String[][] matrix = new String[2][1];
    String aliases() {
        String[] first = matrix[0];
        matrix[0] = new String[1];
        first[0] = "old";
        return first[0] + ":" + matrix[0][0] + ":" + matrix[1][0];
    }
    String shallowCopy() {
        String[][] copy = matrix.clone();
        copy[0][0] = "shared";
        copy[0] = new String[1];
        return matrix[0][0] + ":" + copy[0][0];
    }
    static String aliasResult() { return new RetainedArrayRows().aliases(); }
    static String cloneResult() { return new RetainedArrayRows().shallowCopy(); }
    static void put(String[] row) { row[0] = "call"; }
    static void putDirect(String[] row) { row[0] = "direct"; }
    String[] row() { return matrix[0]; }
    String throughCalls() {
        putDirect(matrix[1]);
        put(row());
        return matrix[0][0] + ":" + matrix[1][0];
    }
    static String callResult() { return new RetainedArrayRows().throughCalls(); }
    static int negativeDimension() {
        RetainedArrayRows owner = new RetainedArrayRows();
        try { owner.matrix = new String[0][-1]; }
        catch (NegativeArraySizeException error) { return 9; }
        return 0;
    }
}
