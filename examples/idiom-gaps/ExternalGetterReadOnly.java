class ExternalGetterReadOnly {
    int read(ExternalCounter counter) {
        return counter.getValue();
    }
}
