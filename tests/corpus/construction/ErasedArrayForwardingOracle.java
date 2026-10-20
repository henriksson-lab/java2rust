class ErasedArrayForwardingOracle {
    public static void main(String[] args) {
        if (!UnseededErasedForward.run() || !SeededErasedForward.run() || !ErasedArrayForwarding.nulls()) throw new AssertionError("null forwarding");
        if (ErasedArrayForwarding.aliasMutation() != 7) throw new AssertionError("alias");
        if (ErasedArrayForwarding.localAndConstructor() != 1) throw new AssertionError("constructor");
        if (ErasedArrayForwarding.primitiveNegative() != 3) throw new AssertionError("primitive");
        if (ErasedArrayForwarding.comparisonEffects() != 22) throw new AssertionError("effects");
        if (ErasedArrayForwarding.checkedComparisonEffects() != 1) throw new AssertionError("checked effects");
        System.out.println("true:true:true:7:1:3:22");
    }
}
