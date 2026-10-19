class CallableFamilyIdentityOracle {
    public static void main(String[] args) {
        if (CallableFamilyIdentity.probe() != 8) {
            throw new AssertionError("callable family identity");
        }
        System.out.println("callable-family-identity: ok");
    }
}
