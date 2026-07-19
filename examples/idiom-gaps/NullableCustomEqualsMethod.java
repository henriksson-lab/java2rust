class NullableCustomEqualsMethod {
    boolean same(NullableValue left, NullableValue right) {
        if (left == null) {
            return Objects.equals(left, right);
        }
        if (right == null) {
            return Objects.equals(left, right);
        }
        return Objects.equals(left, right);
    }
}

class NullableValue {
    private final int id;

    NullableValue(int id) {
        this.id = id;
    }

    boolean equals(NullableValue other) {
        return id == other.id;
    }
}
