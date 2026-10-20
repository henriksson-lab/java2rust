class LogicalChainsOracle {
    public static void main(String[] args) {
        int result = LogicalChains.run();
        if (result != 1) throw new AssertionError("short-circuit order");
        System.out.println(result);
    }
}
