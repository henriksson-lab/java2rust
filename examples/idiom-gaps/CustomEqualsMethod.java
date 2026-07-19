class CustomEqualsMethod {
    boolean same(Value left, Value right) {
        return left.equals(right);
    }
}

class Value {
    private final int id;

    Value(int id) {
        this.id = id;
    }

    boolean equals(Value other) {
        return id == other.id;
    }
}
