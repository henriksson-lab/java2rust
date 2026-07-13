class WhileAssignmentNotNull {
    int drain(Cursor cursor) {
        int count = 0;
        Row row;
        while ((row = cursor.next()) != null) {
            count += row.size();
        }
        return count;
    }
}
