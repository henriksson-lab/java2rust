class ObjectStringIdentityOracle {
    public static void main(String[] args) {
        if (!ObjectStringIdentity.sourceResult()) throw new AssertionError("source result identity");
        if (!ObjectStringIdentity.nativeResult()) throw new AssertionError("native result identity");
        if (ObjectStringIdentity.nullableOperandOrder() != 12) throw new AssertionError("left nullable evaluation order");
        if (ObjectStringIdentity.reverseNullableOperandOrder() != 21) throw new AssertionError("right nullable evaluation order");
        if (!ObjectStringIdentity.twoNullableStrings()) throw new AssertionError("nullable reference identity");
        if (!ObjectStringIdentity.snapshotNullableOperands()) throw new AssertionError("snapshot operands before assignment");
        System.out.println("true:true:12:21:true");
    }
}
