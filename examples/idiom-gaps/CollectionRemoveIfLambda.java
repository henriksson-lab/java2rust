class CollectionRemoveIfLambda {
    void removeBlankLines(List<String> lines) {
        lines.removeIf(line -> line.isEmpty());
    }
}

