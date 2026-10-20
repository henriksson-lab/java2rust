public class ScopedReferenceContractsOracle {
    public static void main(String[] args) {
        int result = ScopedReferenceContracts.run();
        if (result != 0) throw new AssertionError("scoped contract " + result);
        System.out.println(result);
    }
}
