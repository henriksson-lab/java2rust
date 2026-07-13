class ExternalIndexedAccessors {
    String replace(ExternalTable table, int row, int column, String value) {
        String old = table.get(row, column);
        table.set(row, column, value);
        return old;
    }
}
