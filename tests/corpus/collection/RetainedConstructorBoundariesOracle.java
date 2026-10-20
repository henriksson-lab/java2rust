class RetainedConstructorBoundariesOracle {
    public static void main(String[] args) {
        int result = RetainedConstructorBoundaries.run();
        if (result != 7) throw new AssertionError("retained constructor contract: " + result);
        System.out.println(result);
    }
}
