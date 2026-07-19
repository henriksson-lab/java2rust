class StringTrimOwnedSlot {
    String cleaned(String raw) {
        String value = raw.trim();
        return value;
    }

    String maybeIndex(String[] pieces) {
        String index = null;
        if (pieces.length == 3) {
            index = pieces[2].trim();
        }
        return index;
    }
}
