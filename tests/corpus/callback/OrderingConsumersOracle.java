class OrderingConsumersOracle {
    public static void main(String[] args) {
        if (OrderingConsumers.listSorted() != 11122122
                || OrderingConsumers.collectionsSorted() != 11122122
                || OrderingConsumers.extrema() != 2111
                || OrderingConsumers.caughtSort() != 9
                || OrderingConsumers.caughtCollectionsSort() != 9
                || OrderingConsumers.caughtMaximum() != 9
                || OrderingConsumers.emptyMinimum() != 7) {
            throw new AssertionError("ordering consumer contract");
        }
        System.out.println("ordering-consumers: ok");
    }
}
