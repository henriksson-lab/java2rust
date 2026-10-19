class CapturedArrayIdentityOracle {
    public static void main(String[] args) {
        int[] actual = {
            CapturedArrayIdentity.parameterAlias(), CapturedArrayIdentity.returnedAlias(),
            CapturedArrayIdentity.directAlias(), CapturedArrayIdentity.reboundAlias(),
            CapturedArrayIdentity.referenceElements(), CapturedArrayIdentity.nullElement(),
            CapturedArrayIdentity.writes(), CapturedArrayIdentity.shadowed(),
            CapturedArrayIdentity.nested(), CapturedArrayIdentity.renamedCapture(), CapturedArrayIdentity.nullCaught(),
            CapturedArrayIdentity.negativeSize()
        };
        int[] expected = { 8, 18, 7, 16, 3, -6, 5, 7, 8, 9, -4, -5 };
        if (!java.util.Arrays.equals(actual, expected)) throw new AssertionError(java.util.Arrays.toString(actual));
        System.out.println(java.util.Arrays.toString(actual));
    }
}
