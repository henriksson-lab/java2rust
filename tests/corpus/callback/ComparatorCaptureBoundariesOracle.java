class ComparatorCaptureBoundariesOracle {
    public static void main(String[] args) {
        if (ComparatorCaptureBoundaries.borrowed() != 3
                || ComparatorCaptureBoundaries.borrowedNull() != -1
                || ComparatorCaptureBoundaries.constructorBorrowed() != 2
                || ComparatorCaptureBoundaries.localNullableSample() != 11
                || ComparatorCaptureBoundaries.localNullableNull() != -1
                || ComparatorCaptureBoundaries.nullBoundReference() != 8
                || ComparatorCaptureBoundaries.retained() != 0) {
            throw new AssertionError("interface capture/borrow contract");
        }
        System.out.println("comparator-capture-boundaries: ok");
    }
}
