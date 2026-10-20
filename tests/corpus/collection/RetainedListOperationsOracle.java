class RetainedListOperationsOracle {
    public static void main(String[] args) {
        int result = RetainedListOperations.run();
        if (result != 23) throw new AssertionError("retained list operations: " + result);
        System.out.println(result);
    }
}
