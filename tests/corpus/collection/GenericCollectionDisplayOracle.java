class GenericCollectionDisplayOracle {
    public static void main(String[] args) {
        if (!GenericCollectionDisplay.probe().equals("([alpha, beta]);map={k=v};[alpha, beta];<[alpha, beta]>;([alpha, beta])")
                || !GenericCollectionDisplay.nullProbe().equals("(null)")) {
            throw new AssertionError("generic collection display contract");
        }
        System.out.println("generic-collection-display: ok");
    }
}
