class InlineArrayFactoryBoundariesOracle {
    public static void main(String[] args) {
        int result = InlineArrayFactoryBoundaries.run();
        if (result != 1) throw new AssertionError("inline factory backing");
        System.out.println(result);
    }
}
