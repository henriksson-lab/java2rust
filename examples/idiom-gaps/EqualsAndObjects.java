class EqualsAndObjects {
    boolean direct(String left, String right) {
        return left.equals(right);
    }

    boolean nullSafe(String left, String right) {
        return Objects.equals(left, right);
    }
}
