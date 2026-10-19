class NullableStringJoinsOracle {
    public static void main(String[] args) {
        if (!NullableStringJoins.neighborNullResult().equals("[null]")) throw new AssertionError("nullable neighbor");
        if (!NullableStringJoins.neighborAbsent().equals("[missing]")) throw new AssertionError("absent neighbor");
        if (!NullableStringJoins.parentNullResult().equals("[null]")) throw new AssertionError("nullable parent");
        System.out.println("[null]:[missing]:[null]");
    }
}
