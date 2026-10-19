class CheckedIterableOracle {
    public static void main(String[] args) {
        if (CheckedIterable.factoryFailure() != -7) throw new AssertionError("factory catch");
        if (CheckedIterable.freshLists() != 14) throw new AssertionError("repeat iteration");
        if (CheckedIterable.sourceView() != 16) throw new AssertionError("live source view");
        System.out.println("factory=-7;repeat=14;source=16");
    }
}
