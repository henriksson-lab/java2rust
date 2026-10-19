class NestedNullableStringsOracle {
    public static void main(String[] args) {
        if (!NestedNullableStrings.nullNeighbor().equals("[null]")) throw new AssertionError("null neighbor");
        if (!NestedNullableStrings.presentNeighbor().equals("[present]")) throw new AssertionError("present neighbor");
        if (!NestedNullableStrings.nullDescription().equals("[null]")) throw new AssertionError("null description");
        System.out.println("[null]:[present]:[null]");
    }
}
