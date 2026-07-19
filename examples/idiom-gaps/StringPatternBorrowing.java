class StringPatternBorrowing {
    boolean matches(String value, String needle, String suffix) {
        return value.contains(needle) && value.endsWith(suffix);
    }
}
