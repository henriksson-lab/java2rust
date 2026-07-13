class ExternalGetterSetterMutability {
    int update(ExternalCounter counter, int delta) {
        int before = counter.getValue();
        counter.setValue(before + delta);
        return counter.getValue();
    }
}
