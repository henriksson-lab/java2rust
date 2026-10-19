public class RetainedNestedCollectionsOracle {
    public static void main(String[] args) {
        int[] actual = { RetainedNestedCollections.mapAlias(), RetainedNestedCollections.listAlias(),
            RetainedNestedCollections.factoryAlias(), RetainedNestedCollections.fieldAlias(),
            RetainedNestedCollections.plainAlias(), RetainedNestedCollections.extractedAlias(),
            RetainedNestedCollections.crossOwnerAlias(), RetainedNestedCollections.deepAlias(),
            RetainedNestedCollections.nullableRow(), RetainedNestedCollections.callBoundary() };
        int[] expected = { 63, 63, 4, 63, 33, 22, 33, 22, -1, 22 };
        for (int i = 0; i < expected.length; i++) {
            if (actual[i] != expected[i]) { throw new AssertionError(i + ": " + actual[i]); }
            System.out.println(actual[i]);
        }
    }
}
