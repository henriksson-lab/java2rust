class SharedNullableInterfacesOracle {
    public static void main(String[] args) {
        if (!MutableNetwork.aliases()) throw new AssertionError("alias mutation");
        MutableNetwork<String, Integer> original = new MutableNetwork<>(5);
        @SuppressWarnings("unchecked")
        MutableNetwork<String, Integer> duplicate =
            (MutableNetwork<String, Integer>) original.duplicate();
        if (duplicate == original || duplicate.size() != 5) throw new AssertionError("copy");
        original.increment();
        if (original.size() != 6 || duplicate.size() != 5) throw new AssertionError("independence");
        System.out.println("true:6:5");
    }
}
