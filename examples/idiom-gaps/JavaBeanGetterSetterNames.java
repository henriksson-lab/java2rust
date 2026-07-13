class JavaBeanGetterSetterNames {
    private int value;

    public int getValue() {
        return value;
    }

    public void setValue(int value) {
        this.value = value;
    }

    public int copyFrom(JavaBeanGetterSetterNames other) {
        setValue(other.getValue());
        return getValue();
    }
}

