class ArrayBackedCollectionBoundariesOracle {
    public static void main(String[] args) {
        int result = ArrayBackedCollectionBoundaries.run();
        if (result != 127) throw new AssertionError("array backing or collection kind/policy");
        System.out.println(result);
    }
}
