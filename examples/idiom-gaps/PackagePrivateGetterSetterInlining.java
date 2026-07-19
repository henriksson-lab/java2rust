class PackagePrivateGetterSetterInlining {
    private int value;

    int getValue() {
        return value;
    }

    void setValue(int value) {
        this.value = value;
    }

    int copyFrom(PackagePrivateGetterSetterInlining other) {
        setValue(other.getValue());
        return getValue();
    }
}
