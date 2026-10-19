class QualifiedFunctionalValuesOracle {
    public static void main(String[] args) {
        if (QualifiedFunctionalValues.run() != 457
                || QualifiedFunctionalValues.overflow() != -2147483647) {
            throw new AssertionError("qualified native functional carrier");
        }
        System.out.println("qualified-functional-values: ok");
    }
}
