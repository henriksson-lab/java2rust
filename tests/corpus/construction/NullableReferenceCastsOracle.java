class NullableReferenceCastsOracle {
    public static void main(String[] args) {
        if (!NullableReferenceCasts.nullResult() || !NullableReferenceCasts.localResult()
            || !NullableReferenceCasts.assignedResult() || !NullableReferenceCasts.chosenResult()
            || !NullableReferenceCasts.presentResult() || !NullableReferenceCasts.badCast()
            || !NullableReferenceCasts.arrayResult() || !NullableReferenceCasts.nestedArrayResult()
            || !NullableReferenceCasts.inferredNullResult() || !NullableReferenceCasts.inferredPresentResult()
            || !NullableReferenceCasts.nullReceiverCaught()
            || NullableReferenceCasts.nullReceiverOrder() != 1
            || NullableReferenceCasts.primitiveResult() != 3) throw new AssertionError("reference cast");
        System.out.println("null:local:assigned:choice:present:bad-cast:3");
    }
}
