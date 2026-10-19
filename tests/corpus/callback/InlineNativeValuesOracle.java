class InlineNativeValuesOracle {
    public static void main(String[] args) {
        if (InlineNativeValues.shared() != 345
                || InlineNativeValues.twoInputs() != 42
                || InlineNativeValues.caught() != 7
                || InlineNativeValues.ignoredNull() != 9
                || InlineNativeValues.caughtNull() != 8
                || InlineNativeValues.evaluatedOnce() != 19) {
            throw new AssertionError("inline callback value contract");
        }
        System.out.println("inline-native-values: ok");
    }
}
