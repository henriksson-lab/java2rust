class HandleCompanionConsumersOracle {
    public static void main(String[] args) {
        int capability = CapabilitySource.run();
        int cleanup = FinallyRetainedResult.run();
        int matrix = SharedMatrix.run();
        int row = SharedMatrix.rowOrder();
        int nil = SharedMatrix.nullOrder();
        if (capability != 2 || cleanup != 5 || matrix != 12 || row != 0 || nil != 1)
            throw new AssertionError("consumer behavior");
        System.out.println(capability + ":" + cleanup + ":" + matrix + ":" + row + ":" + nil);
    }
}
