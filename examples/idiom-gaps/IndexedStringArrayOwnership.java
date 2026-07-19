class IndexedStringArrayOwnership {
    boolean valid(String value) {
        return value != null && !value.trim().isEmpty();
    }

    String sample(String line) {
        String[] parts = line.split("\\t", -1);
        if (!valid(parts[0]) || parts[1].trim().isEmpty()) {
            return "bad";
        }
        String sample = parts[0];
        return sample + ":" + parts[1].trim();
    }
}
