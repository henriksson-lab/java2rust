public class RetainedMapValuesOracle {
    public static void main(String[] args) {
        int result = RetainedMapValues.run();
        if (result != 0) throw new AssertionError("retained map values contract: " + result);
        System.out.println(result);
    }
}
