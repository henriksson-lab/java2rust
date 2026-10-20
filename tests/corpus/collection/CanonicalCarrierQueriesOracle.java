public class CanonicalCarrierQueriesOracle {
    public static void main(String[] args) {
        int result = CanonicalCarrierQueries.run();
        if (result != 0) throw new AssertionError("canonical carrier query contract: " + result);
        System.out.println(result);
    }
}
