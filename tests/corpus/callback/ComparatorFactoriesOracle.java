class ComparatorFactoriesOracle {
    public static void main(String[] args) {
        if (ComparatorFactories.noEager() != 0
                || ComparatorFactories.compared() != 12
                || ComparatorFactories.reversed() != -1
                || ComparatorFactories.chained() != -1
                || ComparatorFactories.caught() != 9
                || ComparatorFactories.count() != 1
                || ComparatorFactories.retained() != 0
                || ComparatorFactories.caughtCapture() != 8
                || ComparatorFactories.rebound() != 1
                || ComparatorFactories.boundChain() != -1
                || ComparatorFactories.stringCompared() != 1) {
            throw new AssertionError("comparator factory contract");
        }
        System.out.println("comparator-factories: ok");
    }
}
