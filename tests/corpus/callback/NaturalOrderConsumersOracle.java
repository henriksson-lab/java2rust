class NaturalOrderConsumersOracle {
    public static void main(String[] args) {
        if (NaturalOrderConsumers.compared() != 89
                || NaturalOrderConsumers.wordCompared() != -3
                || NaturalOrderConsumers.reverseWordCompared() != 3
                || NaturalOrderConsumers.utf16Compared() != -1987
                || NaturalOrderConsumers.sorted() != 259
                || NaturalOrderConsumers.reverseSorted() != 952
                || NaturalOrderConsumers.nullSorted() != 259
                || NaturalOrderConsumers.storedNullSorted() != 259
                || NaturalOrderConsumers.nullableSorted(false) != 259
                || NaturalOrderConsumers.nullableSorted(true) != 952
                || NaturalOrderConsumers.extrema() != 209
                || NaturalOrderConsumers.emptyMinimum() != 7
                || NaturalOrderConsumers.emptySort() != 0
                || NaturalOrderConsumers.nullableCompare(false) != 8
                || NaturalOrderConsumers.nullableCompare(true) != 1
                || NaturalOrderConsumers.repeatedNullable(false) != 202
                || NaturalOrderConsumers.repeatedNullable(true) != 909
                || NaturalOrderConsumers.forwardedNull() != 2) {
            throw new AssertionError("natural order contract");
        }
        if (NaturalComparableConsumers.noEager() != 0
                || NaturalComparableConsumers.compared() != 7
                || NaturalComparableConsumers.reversed() != -7
                || NaturalComparableConsumers.caught() != 8
                || NaturalComparableConsumers.caughtReversed() != 8
                || NaturalComparableConsumers.caughtSort() != 8) {
            throw new AssertionError("source comparable contract");
        }
        System.out.println("natural-order-consumers: ok");
    }
}
