public final class RetainedPrimitiveArrayIntrinsicsOracle {
    public static void main(String[] args) {
        int result = RetainedPrimitiveArrayIntrinsics.run();
        if (result != 0) throw new AssertionError("arraycopy contract check " + result);
        System.out.println(result);
    }
}
