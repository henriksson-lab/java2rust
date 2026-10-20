class NullableListProbeOracle {
    public static void main(String[] args) {
        int result = NullableListProbe.run();
        if (result != 215) throw new AssertionError("null-probe indices, order, or exceptions");
        System.out.println(result);
    }
}
