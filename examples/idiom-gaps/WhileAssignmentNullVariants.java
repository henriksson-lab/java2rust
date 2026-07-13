class WhileAssignmentNullVariants {
    int drainUntilNull(Cursor cursor) {
        int count = 0;
        Row row;
        while (null != (row = cursor.read())) {
            count += row.size();
        }
        return count;
    }

    Row firstMissing(Cursor cursor) {
        Row row;
        while ((row = cursor.read()) == null) {
            cursor.advance();
        }
        return row;
    }
}
