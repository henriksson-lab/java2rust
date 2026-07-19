class NullableStringFormatAndConcat {
    String describe(String value) {
        if (value == null) {
            return String.format("value=%s", value);
        }
        return "value=" + value;
    }
}
