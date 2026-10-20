public class NestedMapContractsOracle {
    public static void main(String[] args) {
        int result = NestedMapContracts.run();
        if (result != 0) throw new AssertionError("nested map contracts: " + result);
        System.out.println(result);
    }
}
