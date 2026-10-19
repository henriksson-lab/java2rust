class SharedFieldArraysOracle {
    public static void main(String[] args) {
        if (SharedFieldArrays.aliasWrite() != 8) throw new AssertionError("alias");
        if (SharedFieldArrays.rhsRead() != 4) throw new AssertionError("RHS read");
        if (SharedFieldArrays.compoundRead() != 2) throw new AssertionError("compound read");
        if (SharedFieldArrays.rebinding() != 799) throw new AssertionError("rebound allocation");
        if (SharedFieldArrays.selfRebinding() != 99) throw new AssertionError("rebound self");
        if (!SharedFieldArrays.chainedNulls()) throw new AssertionError("chained nulls");
        if (SharedFieldArrays.staticAlias() != 8) throw new AssertionError("static alias");
        if (SharedFieldArrays.localOnly() != 2) throw new AssertionError("local");
        if (SharedFieldArrays.updates() != 133) throw new AssertionError("updates");
        if (SharedFieldArrays.division(2) != 4 || SharedFieldArrays.division(0) != 108) throw new AssertionError("division");
        if (SharedFieldArrays.clonedAllocation() != 17) throw new AssertionError("clone allocation");
        if (SharedFieldArrays.argumentMutation() != 6) throw new AssertionError("argument mutation");
        if (SharedFieldArrays.shallowClone() != 72) throw new AssertionError("shallow clone");
        int[] actual = { SharedFieldArrays.order(true, false, 0), SharedFieldArrays.order(false, true, 0), SharedFieldArrays.order(false, false, 2), SharedFieldArrays.order(false, false, 0), SharedFieldArrays.compoundOrder(true, 0), SharedFieldArrays.compoundOrder(false, 2), SharedFieldArrays.compoundOrder(false, 0) };
        int[] expected = { 11, 1231, 1232, 1230, 121, 122, 1230 };
        if (!java.util.Arrays.equals(actual, expected)) throw new AssertionError(java.util.Arrays.toString(actual));
        System.out.println("8:4:2:799:99:true:8:2:11:1231:1232:1230:121:122:1230");
    }
}
