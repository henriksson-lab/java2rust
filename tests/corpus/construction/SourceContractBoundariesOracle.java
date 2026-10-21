public final class SourceContractBoundariesOracle {
    public static void main(String[] args) {
        int result = SourceContractBoundaries.run();
        if (result != 0) throw new AssertionError("source contracts " + result);
        System.out.println(result);
    }
}
