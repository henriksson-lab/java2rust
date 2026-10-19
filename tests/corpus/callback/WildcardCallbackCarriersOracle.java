class WildcardCallbackCarriersOracle {
    public static void main(String[] args) {
        if (WildcardCallbackCarriers.probe() != 4) {
            throw new AssertionError("wildcard callback carrier contract");
        }
        System.out.println("wildcard-callback-carriers: ok");
    }
}
