public class GenericReferenceIdentityOracle {
    public static void main(String[] args) {
        int result = GenericReferenceIdentity.run();
        if (result != 0) throw new AssertionError("identity control " + result);
        System.out.println(result);
    }
}
