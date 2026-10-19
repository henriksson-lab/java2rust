public class RetainedStreamCallbackContractsOracle {
    public static void main(String[] args) {
        int result = RetainedStreamCallbackContracts.run();
        if (result != 22) { throw new AssertionError("callback contracts: " + result); }
        System.out.println(result);
    }
}
