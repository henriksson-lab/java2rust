class NativeCollectionBoundariesOracle {
    public static void main(String[] args) {
        int result = NativeCollectionBoundaries.run();
        if (result != 2) throw new AssertionError("reference transfers, query equality, or fresh copy");
        System.out.println(result);
    }
}
