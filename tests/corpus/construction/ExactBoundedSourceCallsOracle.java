public class ExactBoundedSourceCallsOracle {
    public static void main(String[] args) {
        int result = ExactBoundedSourceCalls.run();
        if (result != 23) throw new AssertionError(result);
        System.out.println(result);
    }
}
