class StringSplitWithLimit {
    String[] fields(String line) {
        return line.split("\\t", -1);
    }
}
