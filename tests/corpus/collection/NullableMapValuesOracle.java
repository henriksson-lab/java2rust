class NullableMapValuesOracle {
    public static void main(String[] args) {
        if (NullableMapValues.concrete() != 204
                || NullableMapValues.viaInterface() != 204
                || NullableMapValues.field() != 204
                || NullableMapValues.interfaceValues() != 14
                || NullableMapValues.constructorArgument() != 204) {
            throw new AssertionError("nullable map values");
        }
        System.out.println("nullable-map-values: ok");
    }
}
