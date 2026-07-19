class NullableEqualsComparison {
    boolean bothNullable(String left, String right) {
        if (left == null) {
            return right == null;
        }
        if (right == null) {
            return false;
        }
        return Objects.equals(left, right);
    }

    boolean nullableArgument(String left, String right) {
        if (right == null) {
            return false;
        }
        return left.equals(right);
    }
}
