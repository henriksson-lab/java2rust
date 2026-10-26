package dimensions;
class ArrayLengthSource<T> {
    public int[] values;
    public int reads;
    ArrayLengthSource(int[] values) { this.values = values; }
    int size() { return values.length; }
    int checkedSize() { reads++; return values.length; }
    int shiftedSize() { return values.length - 3; }
}
class ArrayLengthChild<T> extends ArrayLengthSource<T> {
    ArrayLengthChild(int[] values) { super(values); }
}
